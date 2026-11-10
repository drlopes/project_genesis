//! E3 — LOD and quadtree spike (docs/21-phase-1-spike-plan.md#e3--lod-bands-and-quadtree).
//!
//! Interactive:   cargo run --release -p spike_app
//!                (right mouse = look, WASD = move, Q/E = down/up, Shift = fast, F = start fly-in)
//! Scripted run:  cargo run --release -p spike_app -- --script --out e3.json [options]
//! Options: --radius-km 24  --grid 65  --sse 2  --spacing-scale 1  --workers 4  --seed 1

mod metrics;
mod patch;
mod quadtree;
mod workers;

use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::mesh::{Indices, PrimitiveTopology};
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

#[derive(Resource, Clone)]
struct Config {
    radius_km: f64,
    grid: u32,
    sse: f64,
    spacing_scale: f64,
    workers: usize,
    seed: u64,
    script: bool,
    out: Option<String>,
}

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
            script: args.iter().any(|a| a == "--script"),
            out: get("--out"),
        }
    }

    fn json(&self) -> String {
        format!(
            "{{ \"radius_km\": {}, \"grid\": {}, \"sse_px\": {}, \"spacing_scale\": {}, \"workers\": {}, \"seed\": {} }}",
            self.radius_km, self.grid, self.sse, self.spacing_scale, self.workers, self.seed
        )
    }
}

#[derive(Resource)]
struct Terrain {
    body: Arc<Body>,
    lod: Lod,
    pool: workers::Pool,
    material: Handle<StandardMaterial>,
    split: HashSet<NodeId>,
    resident: HashMap<NodeId, (Entity, usize)>,
    px_per_radian: f64,
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
}

#[derive(Component)]
struct FlyCam {
    yaw: f32,
    pitch: f32,
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
        .insert_resource(ClearColor(Color::srgb(0.01, 0.01, 0.02)))
        .insert_resource(config)
        .add_systems(Startup, setup)
        .add_systems(Update, (drive_camera, stream_terrain, record_frame).chain())
        .run();
}

fn setup(
    mut commands: Commands,
    config: Res<Config>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
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
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        ..default()
    });

    let start_pos = Vec3::new(0.0, 0.0, (radius * 4.0) as f32);
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 60f32.to_radians(),
            near: 0.1,
            far: 1.0e7,
            ..default()
        }),
        Transform::from_translation(start_pos).looking_at(Vec3::ZERO, Vec3::Y),
        FlyCam {
            yaw: 0.0,
            pitch: 0.0,
        },
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(1.0, 0.6, 0.4).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(Terrain {
        body,
        lod,
        pool,
        material,
        split: HashSet::new(),
        resident: HashMap::new(),
        px_per_radian: 1440.0 / (2.0 * 30f64.to_radians().tan()),
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

fn drive_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    terrain: Res<Terrain>,
    mut run: ResMut<Run>,
    mut cam: Query<(&mut Transform, &mut FlyCam)>,
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
    // Free flight: speed scales with altitude so both orbit and ground are usable.
    let up = tf.translation.normalize();
    let alt = (tf.translation.length() as f64 - terrain.body.radius).max(1.0) as f32;
    let speed = alt.clamp(2.0, 50_000.0)
        * if keys.pressed(KeyCode::ShiftLeft) {
            5.0
        } else {
            1.0
        };
    if mouse.pressed(MouseButton::Right) {
        fly.yaw -= motion.delta.x * 0.003;
        fly.pitch = (fly.pitch - motion.delta.y * 0.003).clamp(-1.5, 1.5);
    }
    let base = Transform::IDENTITY.looking_to(-up.any_orthonormal_vector(), up);
    let rot = base.rotation * Quat::from_rotation_y(fly.yaw) * Quat::from_rotation_x(fly.pitch);
    tf.rotation = rot;
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
    tf.translation += dir.normalize_or_zero() * speed * time.delta_secs();
}

#[allow(clippy::too_many_arguments)]
fn stream_terrain(
    mut commands: Commands,
    mut terrain: ResMut<Terrain>,
    mut run: ResMut<Run>,
    mut meshes: ResMut<Assets<Mesh>>,
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
    let desired = quadtree::select(
        &terrain.lod,
        cam_pos,
        px_per_radian,
        &mut terrain.split,
        &ground,
    );
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
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, p.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, p.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, p.colors);
        mesh.insert_indices(Indices::U32(p.indices));
        let entity = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(terrain.material.clone()),
                Transform::from_xyz(p.centre[0] as f32, p.centre[1] as f32, p.centre[2] as f32),
                Visibility::Hidden,
            ))
            .id();
        if let Some((old, _)) = terrain.resident.insert(p.id, (entity, p.triangles)) {
            commands.entity(old).despawn();
        }
    }

    // 3. Decide what to display: never a hole, never an overlap. All lookups walk the tree
    //    (depth ≤ ~12) with hash sets, so this is O(patches × depth), not O(patches²).
    let ready = |n: &NodeId| terrain.resident.contains_key(n);
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

    // 4. Apply visibility; despawn what is neither displayed nor desired.
    let mut triangles = 0u64;
    let mut drop = Vec::new();
    for (id, (entity, tris)) in &terrain.resident {
        let show = display.contains(id);
        if let Ok(mut v) = vis.get_mut(*entity) {
            *v = if show {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
        if show {
            triangles += *tris as u64;
        } else if !desired_set.contains(id) {
            drop.push(*id);
        }
    }
    for id in drop {
        if let Some((e, _)) = terrain.resident.remove(&id) {
            commands.entity(e).despawn();
        }
    }

    run.pending_frame = metrics::Frame {
        terrain_ms: t.elapsed().as_secs_f32() * 1000.0,
        displayed: display.len() as u32,
        resident: terrain.resident.len() as u32,
        triangles,
        queued: queued as u32,
        in_flight: terrain.pool.in_flight() as u32,
        missing,
        altitude: (tf.translation.length() as f64 - terrain.lod.radius) as f32,
        disturbed,
        ..default()
    };
}

fn record_frame(config: Res<Config>, mut run: ResMut<Run>, mut exit: MessageWriter<AppExit>) {
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
