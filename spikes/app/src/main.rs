//! E3 — LOD and quadtree spike (docs/21-phase-1-spike-plan.md#e3--lod-bands-and-quadtree).
//!
//! Interactive:   cargo run --release -p spike_app
//!                (right mouse = look, WASD = move, Q/E = down/up, Shift = fast, F = start fly-in)
//! Scripted run:  cargo run --release -p spike_app -- --script --out e3.json [options]
//! Options: --radius-km 24  --grid 65  --sse 2  --spacing-scale 1  --workers 4  --seed 1
//!          --morph-s 0.5 (geomorph duration; 0 = instant LOD switches)
//! Debug keys: M morph on/off, G grey geometry, L colour by level, P freeze LOD selection

mod flycam;
mod material;
mod metrics;
mod patch;
mod quadtree;
mod workers;

use material::{
    ATTRIBUTE_QCOLOR, ATTRIBUTE_QMORPH_COLOR, ATTRIBUTE_QMORPH_POSITION, ATTRIBUTE_QNORMALS,
    ATTRIBUTE_QPOSITION, TerrainMaterial, TerrainMaterialPlugin,
};

use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bevy::window::{PresentMode, PrimaryWindow, WindowResolution};
use genesis_core::Seed;
use quadtree::{Lod, NodeId};
use spike_field::Body;
use spike_field::cube::normalize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

/// Main-thread budget for integrating finished patches per frame (docs/21).
const INTEGRATE_BUDGET_MS: f64 = 2.0;
const SCRIPT_DESCENT_S: f64 = 60.0;
const SCRIPT_SKIM_S: f64 = 30.0;
/// Morph factor quantisation: patches share one material per (level, step).
const MORPH_STEPS: f32 = 32.0;

fn sun_dir() -> Vec4 {
    Vec3::new(1.0, 0.6, 0.4).normalize().extend(0.0)
}

#[derive(Resource, Clone)]
struct Config {
    radius_km: f64,
    grid: u32,
    sse: f64,
    spacing_scale: f64,
    workers: usize,
    seed: u64,
    morph_s: f64,
    script: bool,
    out: Option<String>,
    /// Directory for screenshots at fixed points of the scripted fly-in.
    shots: Option<String>,
}

/// Script times (s) of the `--shots` screenshots: ~13 km, ~1 km, ~120 m, skim start, mid skim.
const SHOT_TIMES: [f64; 5] = [15.0, 35.0, 50.0, 62.0, 80.0];

impl Config {
    fn from_args() -> Config {
        let args: Vec<String> = std::env::args().collect();
        let get = |name: &str| {
            args.iter()
                .position(|a| a == name)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        let num =
            |name: &str, default: f64| get(name).and_then(|v| v.parse().ok()).unwrap_or(default);
        Config {
            radius_km: num("--radius-km", 24.0),
            grid: num("--grid", 65.0) as u32,
            sse: num("--sse", 2.0),
            spacing_scale: num("--spacing-scale", 1.0),
            workers: num("--workers", 4.0) as usize,
            seed: num("--seed", 1.0) as u64,
            morph_s: num("--morph-s", 0.5),
            script: args.iter().any(|a| a == "--script"),
            out: get("--out"),
            shots: get("--shots"),
        }
    }

    fn json(&self) -> String {
        format!(
            "{{ \"radius_km\": {}, \"grid\": {}, \"sse_px\": {}, \"spacing_scale\": {}, \"workers\": {}, \"seed\": {}, \"morph_s\": {} }}",
            self.radius_km,
            self.grid,
            self.sse,
            self.spacing_scale,
            self.workers,
            self.seed,
            self.morph_s
        )
    }
}

/// A patch that has been integrated (has an entity).
struct Resident {
    entity: Entity,
    triangles: usize,
    /// GPU bytes of vertex + index data, and what E3's float format would have taken.
    bytes: usize,
    float_bytes: usize,
    /// 0 = own shape, 1 = parent's shape.
    morph: f32,
    /// Morph step of the material currently assigned.
    step: u8,
    shown: bool,
}

#[derive(Resource)]
struct Terrain {
    body: Arc<Body>,
    lod: Lod,
    pool: workers::Pool,
    materials: HashMap<(u8, u8), Handle<TerrainMaterial>>,
    split: HashSet<NodeId>,
    resident: HashMap<NodeId, Resident>,
    /// Sum over resident patches (the terrain's own mesh memory, unlike whole-GPU VRAM).
    mesh_bytes: usize,
    float_format_bytes: usize,
    px_per_radian: f64,
    morph_s: f32,
    morph_on: bool,
    /// 0 shaded, 1 grey geometry, 2 colour by level.
    view_mode: u32,
    last_desired: Vec<NodeId>,
    frozen: Option<Vec<NodeId>>,
}

fn material_for(
    cache: &mut HashMap<(u8, u8), Handle<TerrainMaterial>>,
    assets: &mut Assets<TerrainMaterial>,
    level: u8,
    step: u8,
    view_mode: u32,
) -> Handle<TerrainMaterial> {
    cache
        .entry((level, step))
        .or_insert_with(|| {
            assets.add(TerrainMaterial {
                sun: sun_dir(),
                params: Vec4::new(
                    step as f32 / MORPH_STEPS,
                    level as f32,
                    view_mode as f32,
                    0.0,
                ),
            })
        })
        .clone()
}

#[derive(Resource)]
struct Run {
    start: Instant,
    last_frame: Instant,
    script_start: Option<f64>,
    recorder: metrics::Recorder,
    vram_baseline: String,
    vram_baseline_mib: Option<u64>,
    vram_peak: Arc<std::sync::atomic::AtomicU64>,
    pending_frame: metrics::Frame,
    shots_taken: usize,
}

#[derive(Component)]
struct FlyCam {
    /// Frame counter for throttling window-title updates.
    frame: u32,
}

fn main() {
    let config = Config::from_args();
    let present_mode = if config.script {
        PresentMode::AutoNoVsync
    } else {
        PresentMode::AutoVsync
    };
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Genesis — E3 LOD spike".into(),
                resolution: WindowResolution::new(2560, 1440),
                present_mode,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(TerrainMaterialPlugin)
        .insert_resource(ClearColor(Color::srgb(0.01, 0.01, 0.02)))
        .insert_resource(config)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (debug_keys, drive_camera, stream_terrain, record_frame).chain(),
        )
        .run();
}

fn debug_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut terrain: ResMut<Terrain>,
    mut assets: ResMut<Assets<TerrainMaterial>>,
) {
    let t = &mut *terrain;
    if keys.just_pressed(KeyCode::KeyM) {
        t.morph_on = !t.morph_on;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        t.frozen = match t.frozen {
            Some(_) => None,
            None => Some(t.last_desired.clone()),
        };
    }
    let mut mode = t.view_mode;
    if keys.just_pressed(KeyCode::KeyG) {
        mode = if mode == 1 { 0 } else { 1 };
    }
    if keys.just_pressed(KeyCode::KeyL) {
        mode = if mode == 2 { 0 } else { 2 };
    }
    if mode != t.view_mode {
        t.view_mode = mode;
        for h in t.materials.values() {
            if let Some(mut m) = assets.get_mut(h) {
                m.params.z = mode as f32;
            }
        }
    }
}

fn setup(mut commands: Commands, config: Res<Config>) {
    let radius = config.radius_km * 1000.0;
    let body = Arc::new(Body::bake_with_radius(
        Seed(config.seed),
        radius,
        16,
        4,
        4,
        4,
        256,
    ));
    // Lowest terrain: the pyramid minimum plus the most the analytic L0 deltas can subtract
    // (plates -200 m, craters -300 m, erosion -80 m), rounded down.
    let pyramid_min = body.pyramid.data.iter().copied().fold(f32::MAX, f32::min) as f64;
    let lod = Lod::new(radius, config.grid, config.sse, config.spacing_scale)
        .with_min_height(pyramid_min - 600.0);
    info!(
        "body radius {radius} m, max quadtree level {}, min height {:.0} m",
        lod.max_level, lod.min_height
    );
    let pool = workers::Pool::new(config.workers, body.clone(), lod);

    let start_pos = Vec3::new(0.0, 0.0, (radius * 4.0) as f32);
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 60f32.to_radians(),
            near: 0.1,
            far: 1.0e7,
            ..default()
        }),
        // Start looking (almost) straight down at the body; it fills ~30° of view from here.
        Transform::from_translation(start_pos)
            .with_rotation(flycam::initial(start_pos, -flycam::MAX_PITCH)),
        FlyCam { frame: 0 },
    ));
    // Lighting is done in the terrain shader (sun direction in the material), so no light entity.
    commands.insert_resource(Terrain {
        body,
        lod,
        pool,
        materials: HashMap::new(),
        split: HashSet::new(),
        resident: HashMap::new(),
        mesh_bytes: 0,
        float_format_bytes: 0,
        px_per_radian: 1440.0 / (2.0 * 30f64.to_radians().tan()),
        morph_s: config.morph_s as f32,
        morph_on: config.morph_s > 0.0,
        view_mode: 0,
        last_desired: Vec::new(),
        frozen: None,
    });
    let now = Instant::now();
    commands.insert_resource(Run {
        start: now,
        last_frame: now,
        script_start: config.script.then_some(0.0),
        recorder: metrics::Recorder::default(),
        vram_baseline: metrics::vram(),
        vram_baseline_mib: metrics::vram_used_mib(),
        vram_peak: metrics::start_vram_sampler(),
        pending_frame: metrics::Frame::default(),
        shots_taken: 0,
    });
}

/// Scripted pose at time `t`: exponential descent from 100 km to 30 m while drifting along the
/// surface, then a 30 m/s skim at 30 m altitude. Frame-rate independent.
fn script_pose(body: &Body, t: f64) -> (Vec3, Vec3, Vec3) {
    let r = body.radius;
    let start = normalize([0.3, 0.5, 0.8]);
    let travel = normalize([start[2], 0.0, -start[0]]); // a tangent direction
    let ground = |d: [f64; 3]| (body.h0(d) as f64).max(body.sea_surface as f64);
    let (theta, alt) = if t < SCRIPT_DESCENT_S {
        let k = t / SCRIPT_DESCENT_S;
        (0.15 * k, 100_000.0 * (30.0f64 / 100_000.0).powf(k))
    } else {
        (0.15 + 30.0 / r * (t - SCRIPT_DESCENT_S), 30.0)
    };
    let along = |th: f64| {
        let (s, c) = (th.sin(), th.cos());
        normalize([
            start[0] * c + travel[0] * s,
            start[1] * c + travel[1] * s,
            start[2] * c + travel[2] * s,
        ])
    };
    let d = along(theta);
    let pos = [
        d[0] * (r + ground(d) + alt),
        d[1] * (r + ground(d) + alt),
        d[2] * (r + ground(d) + alt),
    ];
    let ahead_th = theta + (alt * 3.0 + 60.0) / r;
    let da = along(ahead_th);
    let target = [
        da[0] * (r + ground(da)),
        da[1] * (r + ground(da)),
        da[2] * (r + ground(da)),
    ];
    let v = |p: [f64; 3]| Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32);
    (v(pos), v(target), v(d))
}

#[allow(clippy::too_many_arguments)]
fn drive_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    terrain: Res<Terrain>,
    mut run: ResMut<Run>,
    mut cam: Query<(&mut Transform, &mut FlyCam)>,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Ok((mut tf, mut fly)) = cam.single_mut() else {
        return;
    };
    let now = run.start.elapsed().as_secs_f64();
    if keys.just_pressed(KeyCode::KeyF) && run.script_start.is_none() {
        run.script_start = Some(now);
    }
    if let Some(t0) = run.script_start {
        let (pos, target, up) = script_pose(
            &terrain.body,
            (now - t0).min(SCRIPT_DESCENT_S + SCRIPT_SKIM_S),
        );
        *tf = Transform::from_translation(pos).looking_at(target, up);
        return;
    }
    // Free flight. Altitude, and so speed, is measured from the ground (coarse height or sea
    // surface), not from the reference sphere: over oceans and basins, which lie below it, the old
    // radius-relative altitude clamped to 1 m and the camera crawled at 2 m/s.
    let body = terrain.body.clone();
    let ground_radius = move |p: Vec3| {
        let d = p.normalize().as_dvec3();
        body.radius + (body.h0([d.x, d.y, d.z]) as f64).max(body.sea_surface as f64)
    };
    let up = tf.translation.normalize();
    let alt = (tf.translation.length() as f64 - ground_radius(tf.translation)) as f32;
    let speed = alt.clamp(2.0, 50_000.0)
        * if keys.pressed(KeyCode::ShiftLeft) {
            5.0
        } else {
            1.0
        };
    // Orientation has no reference direction (see flycam.rs): mouse look turns about the local up
    // and the camera's right axis, and movement carries the orientation along the sphere.
    if mouse.pressed(MouseButton::Right) {
        tf.rotation = flycam::look(
            tf.rotation,
            up,
            -motion.delta.x * 0.003,
            -motion.delta.y * 0.003,
        );
    }
    fly.frame = fly.frame.wrapping_add(1);
    if let Ok(mut win) = window.single_mut()
        && fly.frame % 15 == 1
    {
        let view = ["shaded", "grey", "by level"][terrain.view_mode as usize];
        win.title = format!(
            "Genesis E3 | alt {:.0} m | {:.0} m/s | morph {} (M) | view {} (G/L) | LOD {} (P) | RMB look · WASD · Q/E · Shift · F fly-in",
            alt,
            speed,
            if terrain.morph_on { "ON" } else { "off" },
            view,
            if terrain.frozen.is_some() {
                "FROZEN"
            } else {
                "live"
            },
        );
    }
    let mut dir = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        dir += *tf.forward();
    }
    if keys.pressed(KeyCode::KeyS) {
        dir -= *tf.forward();
    }
    if keys.pressed(KeyCode::KeyD) {
        dir += *tf.right();
    }
    if keys.pressed(KeyCode::KeyA) {
        dir -= *tf.right();
    }
    if keys.pressed(KeyCode::KeyE) {
        dir += up;
    }
    if keys.pressed(KeyCode::KeyQ) {
        dir -= up;
    }
    let mut p = tf.translation + dir.normalize_or_zero() * speed * time.delta_secs();
    // Stay at least 2 m above the coarse ground. Finer bands can still rise a little above it.
    let min_r = ground_radius(p) + 2.0;
    if (p.length() as f64) < min_r {
        p = p.normalize() * min_r as f32;
    }
    tf.translation = p;
    tf.rotation = flycam::transport(tf.rotation, up, p.normalize());
}

#[allow(clippy::too_many_arguments)]
fn stream_terrain(
    mut commands: Commands,
    mut terrain: ResMut<Terrain>,
    mut run: ResMut<Run>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut assets: ResMut<Assets<TerrainMaterial>>,
    time: Res<Time>,
    cam: Query<(&Transform, &Projection), With<Camera3d>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut vis: Query<&mut Visibility>,
) {
    let t = Instant::now();
    let Ok((tf, proj)) = cam.single() else { return };
    let Ok(win) = window.single() else { return };
    let fov = match proj {
        Projection::Perspective(p) => p.fov as f64,
        _ => 1.0,
    };
    // A minimised window reports height 0, which would collapse the selection to the six roots
    // (seen in E3). Keep the last valid scale, and flag the frame so the run can be rejected.
    let height = win.resolution.physical_height();
    let disturbed = height == 0 || !win.focused;
    if height > 0 {
        terrain.px_per_radian = height as f64 / (2.0 * (fov / 2.0).tan());
    }
    let px_per_radian = terrain.px_per_radian;
    let cam_pos = [
        tf.translation.x as f64,
        tf.translation.y as f64,
        tf.translation.z as f64,
    ];

    // 1. Select the desired cut and queue what is missing, nearest first.
    let terrain = &mut *terrain;
    let body = terrain.body.clone();
    let ground = move |d: [f64; 3]| (body.h0(d) as f64).max(body.sea_surface as f64);
    let desired = match &terrain.frozen {
        Some(f) => f.clone(),
        None => quadtree::select(
            &terrain.lod,
            cam_pos,
            px_per_radian,
            &mut terrain.split,
            &ground,
        ),
    };
    terrain.last_desired.clone_from(&desired);
    let mut wanted = HashMap::new();
    for &n in &desired {
        if !terrain.resident.contains_key(&n) {
            let c = n.centre_dir();
            let r = terrain.lod.radius;
            let d = ((cam_pos[0] - c[0] * r).powi(2)
                + (cam_pos[1] - c[1] * r).powi(2)
                + (cam_pos[2] - c[2] * r).powi(2))
            .sqrt();
            wanted.insert(n, d);
        }
    }
    let queued = terrain.pool.set_wanted(wanted);

    // 2. Integrate finished patches within the main-thread budget. Patches the camera no longer
    //    wants are discarded unopened: integrating them only to despawn them wastes the budget.
    let desired_set: HashSet<NodeId> = desired.iter().copied().collect();
    // Spawned this frame: the entities only exist after the commands apply, so these patches are
    // not displayable until next frame (showing them now would leave a one-frame hole).
    let mut fresh: HashSet<NodeId> = HashSet::new();
    while t.elapsed().as_secs_f64() * 1000.0 < INTEGRATE_BUDGET_MS {
        let Some(p) = terrain.pool.try_recv() else {
            break;
        };
        run.recorder.patch_build_ms.push(p.build_ms);
        run.recorder.patches_built += 1;
        if !desired_set.contains(&p.id) {
            run.recorder.patches_discarded += 1;
            continue;
        }
        // Positions are fractions of the patch box over both morph shapes, so the culling box is
        // the unit box in local space (Bevy cannot compute it: there is no float POSITION).
        let aabb = bevy::camera::primitives::Aabb::from_min_max(Vec3::ZERO, Vec3::ONE);
        let (bytes, float_bytes) = (p.bytes(), p.float_format_bytes());
        terrain.mesh_bytes += bytes;
        terrain.float_format_bytes += float_bytes;
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(
            ATTRIBUTE_QPOSITION,
            VertexAttributeValues::Unorm16x4(p.positions),
        );
        mesh.insert_attribute(
            ATTRIBUTE_QMORPH_POSITION,
            VertexAttributeValues::Unorm16x4(p.morph_positions),
        );
        mesh.insert_attribute(
            ATTRIBUTE_QNORMALS,
            VertexAttributeValues::Snorm16x4(p.normals),
        );
        mesh.insert_attribute(ATTRIBUTE_QCOLOR, VertexAttributeValues::Unorm8x4(p.colors));
        mesh.insert_attribute(
            ATTRIBUTE_QMORPH_COLOR,
            VertexAttributeValues::Unorm8x4(p.morph_colors),
        );
        mesh.insert_indices(Indices::U16(p.indices));
        let transform = Transform::from_translation(Vec3::new(
            p.origin[0] as f32,
            p.origin[1] as f32,
            p.origin[2] as f32,
        ))
        .with_scale(Vec3::from_array(p.extent));
        let material = material_for(
            &mut terrain.materials,
            &mut assets,
            p.id.level,
            0,
            terrain.view_mode,
        );
        let entity = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material),
                aabb,
                transform,
                Visibility::Hidden,
            ))
            .id();
        let resident = Resident {
            entity,
            triangles: p.triangles,
            bytes,
            float_bytes,
            morph: 0.0,
            step: 0,
            shown: false,
        };
        fresh.insert(p.id);
        if let Some(old) = terrain.resident.insert(p.id, resident) {
            terrain.mesh_bytes -= old.bytes;
            terrain.float_format_bytes -= old.float_bytes;
            commands.entity(old.entity).despawn();
        }
    }

    // 3. Decide what to display: never a hole, never an overlap. All lookups walk the tree
    //    (depth ≤ ~12) with hash sets, so this is O(patches × depth), not O(patches²).
    let ready = |n: &NodeId| terrain.resident.contains_key(n) && !fresh.contains(n);
    let mut covering = BTreeSet::new();
    let mut uncovered = Vec::new();
    for &n in &desired {
        if ready(&n) {
            continue;
        }
        match (0..n.level).rev().map(|l| n.ancestor(l)).find(|a| ready(a)) {
            Some(a) => {
                covering.insert(a);
            }
            None => uncovered.push(n),
        }
    }
    let has_covering_ancestor =
        |n: &NodeId| (0..n.level).any(|l| covering.contains(&n.ancestor(l)));
    let mut display: HashSet<NodeId> = covering
        .iter()
        .copied()
        .filter(|a| !has_covering_ancestor(a))
        .collect();
    for &n in &desired {
        if ready(&n) && !has_covering_ancestor(&n) {
            display.insert(n);
        }
    }
    // For desired nodes with no ready ancestor, show their top-most ready descendants.
    let mut has_resident_below: HashSet<NodeId> = HashSet::new();
    for r in terrain.resident.keys() {
        // Walk upward from the parent; once an ancestor is already recorded, so is everything
        // above it.
        for l in (0..r.level).rev() {
            if !has_resident_below.insert(r.ancestor(l)) {
                break;
            }
        }
    }
    let mut missing = 0u32;
    for &n in &uncovered {
        let mut found = false;
        let mut stack = vec![n];
        while let Some(m) = stack.pop() {
            if m != n && ready(&m) {
                display.insert(m);
                found = true;
            } else if has_resident_below.contains(&m) {
                stack.extend(m.children());
            }
        }
        if !found {
            missing += 1;
        }
    }

    // 4. Geomorph transitions between last frame's display and this frame's target.
    //    Split: the new finer patch starts in its parent's shape (morph 1) and morphs to its own;
    //    the parent stays until it has itself finished appearing.
    //    Merge: the finer patches morph back into the parent's shape first; the parent replaces
    //    them only when all have arrived.
    let prev_shown: HashSet<NodeId> = terrain
        .resident
        .iter()
        .filter(|(_, r)| r.shown)
        .map(|(id, _)| *id)
        .collect();
    let mut prev_shown_above: HashSet<NodeId> = HashSet::new();
    for s in &prev_shown {
        for l in (0..s.level).rev() {
            if !prev_shown_above.insert(s.ancestor(l)) {
                break;
            }
        }
    }
    let mut shown: HashSet<NodeId> = HashSet::new();
    let mut retiring: HashSet<NodeId> = HashSet::new();
    let mut appearing: HashSet<NodeId> = HashSet::new();
    for &n in &display {
        if prev_shown.contains(&n) {
            shown.insert(n);
            continue;
        }
        let coarser = (0..n.level)
            .rev()
            .map(|l| n.ancestor(l))
            .find(|a| prev_shown.contains(a));
        if let Some(a) = coarser {
            if terrain.resident[&a].morph > 0.0 {
                shown.insert(a); // still appearing itself: hold it
            } else {
                appearing.insert(n);
                shown.insert(n);
            }
            continue;
        }
        if prev_shown_above.contains(&n) {
            let mut below = Vec::new();
            let mut stack = n.children().to_vec();
            while let Some(m) = stack.pop() {
                if prev_shown.contains(&m) {
                    below.push(m);
                } else if prev_shown_above.contains(&m) {
                    stack.extend(m.children());
                }
            }
            if below.iter().all(|d| terrain.resident[d].morph >= 1.0) {
                shown.insert(n);
            } else {
                for d in below {
                    shown.insert(d);
                    retiring.insert(d);
                }
            }
            continue;
        }
        shown.insert(n);
    }

    // 5. Advance morphs, swap materials and visibility where they changed, and despawn what is
    //    neither shown nor desired.
    let rate = if terrain.morph_on && terrain.morph_s > 0.0 {
        time.delta_secs() / terrain.morph_s
    } else {
        f32::INFINITY
    };
    let mut triangles = 0u64;
    let mut drop = Vec::new();
    let view_mode = terrain.view_mode;
    for (id, r) in terrain.resident.iter_mut() {
        let show = shown.contains(id);
        if show {
            if appearing.contains(id) {
                r.morph = 1.0;
            }
            r.morph = if retiring.contains(id) {
                (r.morph + rate).min(1.0)
            } else {
                (r.morph - rate).max(0.0)
            };
            let step = (r.morph * MORPH_STEPS).round() as u8;
            if step != r.step {
                r.step = step;
                let m = material_for(
                    &mut terrain.materials,
                    &mut assets,
                    id.level,
                    step,
                    view_mode,
                );
                commands.entity(r.entity).insert(MeshMaterial3d(m));
            }
            triangles += r.triangles as u64;
        } else if !desired_set.contains(id) {
            drop.push(*id);
        }
        // Only record the change once applied: an entity spawned this frame does not exist yet
        // (commands are deferred), so it is retried next frame.
        if show != r.shown
            && let Ok(mut v) = vis.get_mut(r.entity)
        {
            *v = if show {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
            r.shown = show;
        }
    }
    for id in drop {
        if let Some(r) = terrain.resident.remove(&id) {
            terrain.mesh_bytes -= r.bytes;
            terrain.float_format_bytes -= r.float_bytes;
            commands.entity(r.entity).despawn();
        }
    }

    run.pending_frame = metrics::Frame {
        terrain_ms: t.elapsed().as_secs_f32() * 1000.0,
        displayed: shown.len() as u32,
        resident: terrain.resident.len() as u32,
        triangles,
        queued: queued as u32,
        in_flight: terrain.pool.in_flight() as u32,
        mesh_mb: (terrain.mesh_bytes / 1_000_000) as u32,
        float_format_mb: (terrain.float_format_bytes / 1_000_000) as u32,
        missing,
        altitude: (tf.translation.length() as f64 - terrain.lod.radius) as f32,
        disturbed,
        ..default()
    };
}

fn record_frame(
    mut commands: Commands,
    config: Res<Config>,
    mut run: ResMut<Run>,
    mut exit: MessageWriter<AppExit>,
) {
    if let (Some(dir), Some(t0)) = (&config.shots, run.script_start) {
        let ts = run.start.elapsed().as_secs_f64() - t0;
        if run.shots_taken < SHOT_TIMES.len() && ts >= SHOT_TIMES[run.shots_taken] {
            use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
            let path = format!("{dir}/shot-{:02}s.png", SHOT_TIMES[run.shots_taken] as u32);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
            run.shots_taken += 1;
        }
    }
    let now = Instant::now();
    let frame_ms = (now - run.last_frame).as_secs_f32() * 1000.0;
    run.last_frame = now;
    let t = run.start.elapsed().as_secs_f64();
    let mut f = std::mem::take(&mut run.pending_frame);
    f.t = t;
    f.frame_ms = frame_ms;
    let recording = run.script_start.is_some_and(|t0| t >= t0);
    if recording {
        run.recorder.frames.push(f);
    }
    if config.script {
        let t0 = run.script_start.unwrap_or(0.0);
        if t - t0 >= SCRIPT_DESCENT_S + SCRIPT_SKIM_S {
            let json = run.recorder.summary_json(
                &config.json(),
                metrics::ram_mb(),
                &format!(
                    "baseline {} / end {} / peak {} MiB / peak above baseline {} MiB",
                    run.vram_baseline,
                    metrics::vram(),
                    run.vram_peak.load(std::sync::atomic::Ordering::Relaxed),
                    run.vram_baseline_mib.map_or("?".to_string(), |b| {
                        run.vram_peak
                            .load(std::sync::atomic::Ordering::Relaxed)
                            .saturating_sub(b)
                            .to_string()
                    })
                ),
                t - t0,
            );
            match &config.out {
                Some(path) => {
                    std::fs::write(path, &json).expect("write results");
                    let csv = std::path::Path::new(path).with_extension("csv");
                    std::fs::write(csv, run.recorder.frames_csv()).expect("write frames");
                }
                None => println!("{json}"),
            }
            exit.write(AppExit::Success);
        }
    }
}
