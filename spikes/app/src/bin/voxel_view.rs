//! E7 look test: a voxel region at eye height, meeting the heightfield at its edge.
//!
//! ```text
//! cargo run --release -p spike_app --bin voxel_view -- --voxel 0.5
//! ```
//!
//! The region is built around a deterministic site (`Site::pick`), meshed with surface nets (or
//! the blocky baseline), and surrounded by a heightfield mesh sampled from the same field.
//! Everything is in the region's local frame: x east, y north, z up, camera at the centre.
//!
//! Keys: **1/2/3** voxel size 0.25/0.5/1 m · **B** blocky on/off · **E** scripted edits ·
//! **T** tint the two meshes (seam check) · right mouse = look · **WASD** = move · **Space/C** =
//! up/down · **Shift** = fast.
//!
//! Options: `--radius 32` (region half-extent, metres) · `--voxel 0.5` · `--blocky` · `--edits`
//! · `--tint` · `--hf 0.5` (heightfield spacing) · `--eye 2` · `--yaw 0` `--pitch -8` (degrees)
//! · `--shot out.png --frames 300` (save a screenshot and exit).

use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
use bevy::window::{PresentMode, WindowResolution};
use genesis_core::Seed;
use spike_field::Body;
use spike_voxel::{Site, Voxels, mesh};
use std::sync::Arc;
use std::time::Instant;

const SEED: u64 = 3;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let num = |flag: &str, default: f64| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    let flag = |name: &str| args.iter().any(|a| a == name);
    let get = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };

    let scene = Scene {
        body: Arc::new(Body::bake_with_radius(
            Seed(SEED),
            24_000.0,
            16,
            4,
            4,
            4,
            256,
        )),
        radius: num("--radius", 32.0),
        voxel: num("--voxel", 0.5),
        blocky: flag("--blocky"),
        edits: flag("--edits"),
        hf: num("--hf", 0.5),
        eye: num("--eye", 2.0),
        yaw: (num("--yaw", 0.0) as f32).to_radians(),
        pitch: (num("--pitch", -8.0) as f32).to_radians(),
        tint: flag("--tint"),
    };
    let shot = Shot {
        path: get("--shot"),
        frames: num("--frames", 300.0) as u32,
        ..default()
    };

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "E7 voxel region".into(),
                resolution: WindowResolution::new(1280, 720),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.42, 0.55, 0.72)))
        .insert_resource(scene)
        .insert_resource(shot)
        .init_resource::<CameraState>()
        .init_resource::<Stats>()
        .add_systems(Startup, setup)
        .add_systems(Update, (camera, tint, rebuild, screenshot, hud))
        .run();
}

#[derive(Resource)]
struct Scene {
    body: Arc<Body>,
    radius: f64,
    voxel: f64,
    blocky: bool,
    edits: bool,
    hf: f64,
    eye: f64,
    yaw: f32,
    pitch: f32,
    tint: bool,
}

#[derive(Resource, Default)]
struct Stats {
    text: String,
}

#[derive(Resource, Default)]
struct CameraState {
    yaw: f32,
    pitch: f32,
}

#[derive(Resource, Default)]
struct Shot {
    path: Option<String>,
    frames: u32,
    waited: u32,
    fired: bool,
}

#[derive(Resource)]
struct Mats {
    plain: Handle<StandardMaterial>,
    voxel: Handle<StandardMaterial>,
    hf: Handle<StandardMaterial>,
}

#[derive(Component)]
struct RegionMesh;

#[derive(Component)]
struct TerrainMesh;

#[derive(Component)]
struct Post;

#[derive(Component)]
struct Cube;

#[derive(Component)]
struct Hud;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    scene: Res<Scene>,
    mut camera: ResMut<CameraState>,
) {
    let mat = |materials: &mut Assets<StandardMaterial>, color: [f32; 3]| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(color[0], color[1], color[2]),
            perceptual_roughness: 0.95,
            ..default()
        })
    };
    let mats = Mats {
        plain: mat(&mut materials, [0.55, 0.52, 0.44]),
        voxel: mat(&mut materials, [0.36, 0.47, 0.62]),
        hf: mat(&mut materials, [0.42, 0.55, 0.38]),
    };

    camera.yaw = scene.yaw;
    camera.pitch = scene.pitch;

    // The site depends only on the field and the voxel size, so one pick gives the starting eye.
    let site = Site::pick(&scene.body, scene.voxel, scene.radius);
    let z = site.ground_z(&scene.body, 0.0, 0.0);
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 70f32.to_radians(),
            near: 0.05,
            far: 2000.0,
            ..default()
        }),
        Transform::from_translation(Vec3::new(0.0, (z + scene.eye) as f32, 0.0))
            .with_rotation(Quat::from_rotation_y(camera.yaw) * Quat::from_rotation_x(camera.pitch)),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.6, 1.0, 0.3).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        AmbientLight {
            color: Color::srgb(0.6, 0.7, 0.85),
            brightness: 450.0,
            ..default()
        },
        Transform::default(),
    ));

    // Human-scale references: a 1.8 m post and a 1 m cube.
    let reference = materials.add(StandardMaterial {
        base_color: Color::srgb(0.78, 0.16, 0.12),
        perceptual_roughness: 0.6,
        ..default()
    });
    let post = meshes.add(Cuboid::new(0.5, 1.8, 0.4));
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    commands.spawn((
        Post,
        Mesh3d(post),
        MeshMaterial3d(reference.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
    commands.spawn((
        Cube,
        Mesh3d(cube),
        MeshMaterial3d(reference),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    commands.insert_resource(mats);
    commands.spawn((
        Hud,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        },
        Text::new("building…"),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::WHITE),
    ));
}

/// Build (or rebuild) the region and its heightfield ring. The first frame always builds; keys
/// rebuild with a new configuration.
fn rebuild(
    mut first: Local<bool>,
    keys: Res<ButtonInput<KeyCode>>,
    mut scene: ResMut<Scene>,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Res<Mats>,
    mut commands: Commands,
    region: Query<Entity, With<RegionMesh>>,
    terrain: Query<Entity, With<TerrainMesh>>,
    mut post: Query<&mut Transform, (With<Post>, Without<Cube>)>,
    mut cube: Query<&mut Transform, (With<Cube>, Without<Post>)>,
    mut stats: ResMut<Stats>,
) {
    let mut run = !*first;
    if keys.just_pressed(KeyCode::Digit1) {
        scene.voxel = 0.25;
        run = true;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        scene.voxel = 0.5;
        run = true;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        scene.voxel = 1.0;
        run = true;
    }
    if keys.just_pressed(KeyCode::KeyB) {
        scene.blocky = !scene.blocky;
        run = true;
    }
    if keys.just_pressed(KeyCode::KeyE) {
        scene.edits = !scene.edits;
        run = true;
    }
    if !run {
        return;
    }
    *first = true;

    let t = Instant::now();
    let site = Site::pick(&scene.body, scene.voxel, scene.radius);
    let (mut vx, build) = Voxels::build(&scene.body, site);
    let build_ms = t.elapsed().as_secs_f64() * 1000.0;
    if scene.edits {
        apply_edits(&mut vx, &scene.body, site);
    }

    let t = Instant::now();
    let m = if scene.blocky {
        mesh::blocky(&vx, &scene.body)
    } else {
        mesh::surface_nets(&vx, &scene.body)
    };
    let mesh_ms = t.elapsed().as_secs_f64() * 1000.0;

    let t = Instant::now();
    let hf = heightfield_ring(&scene, &site);
    let hf_ms = t.elapsed().as_secs_f64() * 1000.0;

    for e in &region {
        commands.entity(e).despawn();
    }
    for e in &terrain {
        commands.entity(e).despawn();
    }
    let region_mat = if scene.tint {
        mats.voxel.clone()
    } else {
        mats.plain.clone()
    };
    let hf_mat = if scene.tint {
        mats.hf.clone()
    } else {
        mats.plain.clone()
    };
    commands.spawn((
        RegionMesh,
        Mesh3d(meshes.add(to_bevy(&m))),
        MeshMaterial3d(region_mat),
    ));
    commands.spawn((
        TerrainMesh,
        Mesh3d(meshes.add(hf.mesh)),
        MeshMaterial3d(hf_mat),
    ));

    // Stand the references on the terrain at a fixed local spot.
    let z = site.ground_z(&scene.body, 3.0, 7.0);
    if let Ok(mut t) = post.single_mut() {
        t.translation = Vec3::new(3.0, (z + 0.9) as f32, -7.0);
    }
    let z = site.ground_z(&scene.body, 7.0, 14.0);
    if let Ok(mut t) = cube.single_mut() {
        t.translation = Vec3::new(7.0, (z + 0.5) as f32, -14.0);
    }

    stats.text = format!(
        "{} · voxel {:.2} m · region ±{:.0} m ({:.0}×{:.0}×{:.0} m) · {} bricks, {:.1} MB\n\
         {} tris, {:.1} MB · build {:.0} ms · mesh {:.0} ms · heightfield {:.0} ms ({} tris)\n\
         1/2/3 voxel 0.25/0.5/1 m · B blocky · E edits ({}) · T tint\n\
         RMB look · WASD move · Space/C up/down · Shift fast",
        if scene.blocky {
            "blocky"
        } else {
            "surface nets"
        },
        site.voxel,
        site.half(),
        site.ncx as f64 * 32.0 * site.voxel,
        site.ncy as f64 * 32.0 * site.voxel,
        site.ncz as f64 * 32.0 * site.voxel,
        build.bricks,
        build.bytes as f64 / 1e6,
        m.tris.len(),
        m.bytes() as f64 / 1e6,
        build_ms,
        mesh_ms,
        hf_ms,
        hf.tris,
        if scene.edits { "on" } else { "off" },
    );
    println!(
        "voxel {:.2} m, {}: {} bricks, {:.1} MB, {} tris, build {:.0} ms (sampling {:.0} ms), mesh {:.0} ms, hf {:.0} ms",
        site.voxel,
        if scene.blocky {
            "blocky"
        } else {
            "surface nets"
        },
        build.bricks,
        build.bytes as f64 / 1e6,
        m.tris.len(),
        build_ms,
        build.ms,
        mesh_ms,
        hf_ms
    );
}

fn hud(stats: Res<Stats>, mut text: Query<&mut Text, With<Hud>>) {
    if stats.is_changed() {
        if let Ok(mut t) = text.single_mut() {
            **t = stats.text.clone();
        }
    }
}

fn camera(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
    mut state: ResMut<CameraState>,
    mut q: Query<&mut Transform, With<Camera3d>>,
) {
    let Ok(mut t) = q.single_mut() else {
        return;
    };
    if buttons.pressed(MouseButton::Right) {
        state.yaw -= motion.delta.x * 0.0025;
        state.pitch = (state.pitch - motion.delta.y * 0.0025).clamp(-1.5, 1.5);
    }
    t.rotation = Quat::from_rotation_y(state.yaw) * Quat::from_rotation_x(state.pitch);

    let mut dir = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        dir += t.rotation * Vec3::NEG_Z;
    }
    if keys.pressed(KeyCode::KeyS) {
        dir -= t.rotation * Vec3::NEG_Z;
    }
    if keys.pressed(KeyCode::KeyD) {
        dir += t.rotation * Vec3::X;
    }
    if keys.pressed(KeyCode::KeyA) {
        dir -= t.rotation * Vec3::X;
    }
    if keys.pressed(KeyCode::Space) {
        dir += Vec3::Y;
    }
    if keys.pressed(KeyCode::KeyC) {
        dir -= Vec3::Y;
    }
    let speed = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        40.0
    } else {
        6.0
    };
    if dir != Vec3::ZERO {
        t.translation += dir.normalize() * speed * time.delta_secs();
    }
}

fn tint(
    keys: Res<ButtonInput<KeyCode>>,
    mut scene: ResMut<Scene>,
    mats: Res<Mats>,
    mut region: Query<&mut MeshMaterial3d<StandardMaterial>, With<RegionMesh>>,
    mut terrain: Query<
        &mut MeshMaterial3d<StandardMaterial>,
        (With<TerrainMesh>, Without<RegionMesh>),
    >,
) {
    if !keys.just_pressed(KeyCode::KeyT) {
        return;
    }
    scene.tint = !scene.tint;
    let (r, h) = if scene.tint {
        (mats.voxel.clone(), mats.hf.clone())
    } else {
        (mats.plain.clone(), mats.plain.clone())
    };
    for mut m in &mut region {
        m.0 = r.clone();
    }
    for mut m in &mut terrain {
        m.0 = h.clone();
    }
}

fn screenshot(mut shot: ResMut<Shot>, mut commands: Commands, mut exit: MessageWriter<AppExit>) {
    let Some(path) = shot.path.clone() else {
        return;
    };
    if !shot.fired {
        if shot.frames > 0 {
            shot.frames -= 1;
            return;
        }
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        shot.fired = true;
        return;
    }
    shot.waited += 1;
    if shot.waited > 30 {
        exit.write(AppExit::Success);
    }
}

/// Local (x east, y north, z up) to Bevy (x, z, −y): a proper rotation, so winding is preserved.
fn to_bevy(m: &mesh::Mesh) -> Mesh {
    let mut out = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let pos: Vec<[f32; 3]> = m.verts.iter().map(|p| [p[0], p[2], -p[1]]).collect();
    let nrm: Vec<[f32; 3]> = m.normals.iter().map(|n| [n[0], n[2], -n[1]]).collect();
    out.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    out.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
    out.insert_indices(Indices::U32(
        m.tris.iter().flat_map(|t| [t[0], t[1], t[2]]).collect(),
    ));
    out
}

struct Ring {
    mesh: Mesh,
    tris: usize,
}

/// The heightfield around the box, at `--hf` spacing: a grid with the cells inside the region
/// skipped. Its boundary lands on the box edge when `hf` divides the half-extent (the default
/// 0.5 m does for every voxel size).
fn heightfield_ring(scene: &Scene, site: &Site) -> Ring {
    let half = site.half();
    let hf = scene.hf;
    let n_half = (half / hf).round() as i32;
    let extra = (32.0 / hf).ceil() as i32;
    let n = n_half + extra;
    let side = 2 * n;
    let outer = n as f64 * hf;
    let g = |x: f64, y: f64| site.ground_z(&scene.body, x, y);

    let mut heights = vec![0.0f32; ((side + 1) * (side + 1)) as usize];
    for j in 0..=side {
        for i in 0..=side {
            let x = -outer + i as f64 * hf;
            let y = -outer + j as f64 * hf;
            heights[(i + (side + 1) * j) as usize] = g(x, y) as f32;
        }
    }
    let at = |i: i32, j: i32| heights[(i + (side + 1) * j) as usize] as f64;
    let normal = |i: i32, j: i32| {
        let i0 = (i - 1).max(0);
        let i1 = (i + 1).min(side);
        let j0 = (j - 1).max(0);
        let j1 = (j + 1).min(side);
        let dx = (at(i1, j) - at(i0, j)) / ((i1 - i0) as f64 * hf);
        let dy = (at(i, j1) - at(i, j0)) / ((j1 - j0) as f64 * hf);
        let l = (dx * dx + dy * dy + 1.0).sqrt();
        [(-dx / l) as f32, (-dy / l) as f32, (1.0 / l) as f32]
    };

    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut nrm: Vec<[f32; 3]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let mut quads = 0usize;
    for j in 0..side {
        for i in 0..side {
            let x0 = -outer + i as f64 * hf;
            let y0 = -outer + j as f64 * hf;
            // Inside the region the voxel mesh draws the surface.
            if x0 >= -half - 1e-6
                && x0 + hf <= half + 1e-6
                && y0 >= -half - 1e-6
                && y0 + hf <= half + 1e-6
            {
                continue;
            }
            let base = pos.len() as u32;
            for (di, dj) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
                let (gi, gj) = (i + di, j + dj);
                let x = (-outer + gi as f64 * hf) as f32;
                let y = (-outer + gj as f64 * hf) as f32;
                let z = at(gi, gj) as f32;
                pos.push([x, z, -y]);
                let n = normal(gi, gj);
                nrm.push([n[0], n[2], -n[1]]);
            }
            idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            quads += 1;
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
    mesh.insert_indices(Indices::U32(idx));
    Ring {
        mesh,
        tris: quads * 2,
    }
}

/// The scripted edits from the E7 harness, at the site's centre.
fn apply_edits(vx: &mut Voxels, body: &Body, site: Site) {
    let surface = site.ground_z(body, 0.0, 0.0);
    vx.flatten(body, [0.0, 0.0, surface], 8.0, surface - 1.0);
    for i in 0..=16 {
        let x = -8.0 + i as f64;
        let z = site.ground_z(body, x, 0.0) - 4.0;
        vx.dig(body, [x, 0.0, z], 1.5);
    }
    let carve = site.ground_z(body, 24.0, 6.0) - 3.0;
    vx.dig(body, [24.0, 6.0, carve], 6.0);

    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let half = site.half();
    for _ in 0..300 {
        let x = rng.range(-half * 0.85, half * 0.85);
        let y = rng.range(-half * 0.85, half * 0.85);
        let z = site.ground_z(body, x, y) - rng.range(0.0, 1.5);
        vx.dig(body, [x, y, z], 1.0);
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}
