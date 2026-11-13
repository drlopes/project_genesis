//! E6: a walking character in the app (Avian `MoveAndSlide`).
//!
//! - **Physics frame** (E5): origin on the ground where walking starts, +Y up; positions are f64
//!   body-local, differenced, then cast. Gravity follows the local up as the character moves.
//! - **Collision = visual LOD** (docs/18): trimesh colliders built from the *displayed* patches
//!   within `INTERACTION_M` of the character, at most `BUILD_PER_FRAME` per frame (E6: rebuilding
//!   many trimeshes at once costs 16–39 ms). New colliders are added before old ones are removed,
//!   so a LOD change never leaves a gap.
//! - Colliders use each patch's own shape; during a 0.5 s morph the visible surface is between
//!   parent and own shape, so the two can differ by the morph offset (cm to dm) briefly.

use super::{BodyLocal, Config, FlyCam, Terrain};
use avian3d::prelude::*;
use bevy::math::{DQuat, DVec3};
use bevy::prelude::*;
use spike_app::quadtree::NodeId;
use spike_field::{Depth, Sample};
use std::time::{Duration, Instant};

pub const RADIUS: f32 = 0.3;
pub const HALF: f32 = 0.6;
/// Eye height above the capsule centre (eyes ~1.6 m above the ground).
pub const EYE: f32 = 0.7;
const INTERACTION_M: f64 = 40.0;
const BUILD_PER_FRAME: usize = 4;
const WALK: f32 = 6.0;
const RUN: f32 = 12.0;
const JUMP: f32 = 5.0;
/// Ground snap: after a grounded move, settle onto walkable ground up to this far below.
/// (The first version instead pushed down at 6 m/s while grounded; `MoveAndSlide` projected that
/// onto slopes as a slide that never stopped, even with no input.)
const SNAP_M: f32 = 0.5;
/// Walkable: surface normal within 50° of up.
const WALKABLE_COS: f32 = 0.64;

#[derive(Component)]
pub struct Character;

#[derive(Component)]
pub struct TerrainCollider;

#[derive(Resource, Default)]
pub struct Walking {
    pub active: bool,
    origin: DVec3,
    rot: DQuat,
    vy: f32,
    pub grounded: bool,
    prev: Vec3,
    cur: Vec3,
    pub colliders: usize,
    pub pending: usize,
    pub move_ms: f32,
    /// Movement waits until colliders under the character exist (and have had a physics step).
    ready_in: u32,
    pub ready: bool,
    /// `--auto-walk` statistics.
    pub auto: AutoStats,
}

#[derive(Default)]
pub struct AutoStats {
    pub started: bool,
    pub t: f64,
    pub steps: usize,
    pub grounded_steps: usize,
    pub fell: usize,
    /// Capsule bottom minus field surface while grounded (+ floats, − sinks).
    pub gaps: Vec<f64>,
    pub move_ms: Vec<f32>,
    pub distance: f64,
    pub shot: bool,
    /// Idle phases (no input): position at the start of the current idle phase, the largest
    /// sideways drift seen in any idle phase (must be zero: no movement without input), and the
    /// largest settle along the surface normal (contact tracking, not drift).
    pub idle_start: Option<DVec3>,
    pub idle_drift_max: f64,
    pub idle_settle_max: f64,
    pub idle_steps: usize,
    /// Steepest ground (degrees) an idle phase started on: the sliding case.
    pub idle_slope_max: f64,
}

impl Walking {
    fn to_frame(&self, p: DVec3) -> Vec3 {
        (self.rot.inverse() * (p - self.origin)).as_vec3()
    }

    fn to_body(&self, v: Vec3) -> DVec3 {
        self.origin + self.rot * v.as_dvec3()
    }

    /// Interpolated body-local position of the eyes; `alpha` is the fixed-step overstep.
    pub fn eye(&self, alpha: f32) -> DVec3 {
        let c = self.to_body(self.prev.lerp(self.cur, alpha));
        c + c.normalize() * EYE as f64
    }

    pub fn speed_hint(&self) -> f32 {
        (self.cur - self.prev).length() * 64.0
    }
}

fn frame_rot(up: DVec3) -> DQuat {
    let reference = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
    let east = reference.cross(up).normalize();
    DQuat::from_mat3(&bevy::math::DMat3::from_cols(east, up, east.cross(up)))
}

fn surface_radius(terrain: &Terrain, d: DVec3) -> f64 {
    let level = terrain.lod.max_level;
    let mut s = Sample::default();
    terrain.body.sample(
        d.to_array(),
        terrain.lod.spacing(level),
        Depth::for_band(terrain.lod.band(level)),
        &mut s,
    );
    terrain.body.radius + s.height as f64
}

/// Ground slope under a body-local direction, from the field 0.5 m east and north.
fn slope_deg(terrain: &Terrain, d: DVec3) -> f64 {
    let c = d * surface_radius(terrain, d);
    let reference = if d.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
    let east = reference.cross(d).normalize();
    let north = d.cross(east);
    let toward = |t: DVec3| {
        let q = (d + t * 0.5).normalize();
        q * surface_radius(terrain, q)
    };
    let normal = (toward(east) - c).cross(toward(north) - c).normalize();
    normal.dot(d).abs().acos().to_degrees()
}

/// C toggles walking: drop a character on the ground under the camera, or return to flight.
/// `--auto-walk <s>` starts walking on the first frame.
#[allow(clippy::too_many_arguments)]
pub fn toggle(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    config: Res<Config>,
    mut walking: ResMut<Walking>,
    mut terrain: ResMut<Terrain>,
    mut cam: Query<(&BodyLocal, &mut Transform), With<FlyCam>>,
    characters: Query<Entity, With<Character>>,
) {
    let auto_start = config.auto_walk > 0.0 && !walking.auto.started;
    if config.script || !(keys.just_pressed(KeyCode::KeyC) || auto_start) {
        return;
    }
    if walking.active {
        for e in &characters {
            commands.entity(e).despawn();
        }
        for r in terrain.resident.values_mut() {
            if let Some(c) = r.collider.take() {
                commands.entity(c).despawn();
            }
        }
        for c in terrain.orphan_colliders.drain(..) {
            commands.entity(c).despawn();
        }
        *walking = Walking::default();
        return;
    }
    let Ok((cam, mut cam_tf)) = cam.single_mut() else {
        return;
    };
    let d = cam.0.normalize();
    let ground = d * surface_radius(&terrain, d);
    let start = Vec3::new(0.0, HALF + RADIUS + 0.3, 0.0);
    *walking = Walking {
        active: true,
        origin: ground,
        rot: frame_rot(d),
        prev: start,
        cur: start,
        auto: AutoStats {
            started: true,
            ..default()
        },
        ..default()
    };
    // Look ahead, slightly down, instead of straight down from orbit.
    cam_tf.rotation = spike_app::flycam::initial(d.as_vec3() * 1000.0, -0.15);
    commands.spawn((
        Character,
        RigidBody::Kinematic,
        Collider::capsule(RADIUS, 2.0 * HALF),
        Transform::from_translation(start),
    ));
}

/// Keep trimesh colliders on the displayed patches near the character.
pub fn stream_colliders(
    mut commands: Commands,
    mut terrain: ResMut<Terrain>,
    mut walking: ResMut<Walking>,
) {
    if !walking.active {
        return;
    }
    let ch = walking.to_body(walking.cur);
    let terrain = &mut *terrain;
    let mut want: Vec<(f64, NodeId)> = terrain
        .resident
        .iter()
        .filter(|(_, r)| r.shown && r.collision.is_some())
        .filter_map(|(id, r)| {
            let (c, half) = r.collision.as_ref()?.bounds();
            let d = (DVec3::from_array(c) - ch).length() - half;
            (d < INTERACTION_M).then_some((d.max(0.0), *id))
        })
        .collect();
    want.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (mut built, mut pending, mut pending_near) = (0, 0, false);
    for &(d, id) in &want {
        let r = terrain
            .resident
            .get_mut(&id)
            .expect("wanted patch is resident");
        if r.collider.is_some() {
            continue;
        }
        if built >= BUILD_PER_FRAME {
            pending += 1;
            pending_near |= d < 5.0;
            continue;
        }
        let data = r.collision.as_ref().expect("filtered");
        let verts: Vec<Vec3> = data
            .vertices()
            .map(|p| walking.to_frame(DVec3::from_array(p)))
            .collect();
        let tris: Vec<[u32; 3]> = data
            .tris
            .iter()
            .map(|t| [t[0] as u32, t[1] as u32, t[2] as u32])
            .collect();
        let e = commands
            .spawn((
                TerrainCollider,
                RigidBody::Static,
                Collider::trimesh_with_config(verts, tris, TrimeshFlags::FIX_INTERNAL_EDGES),
                Transform::default(),
            ))
            .id();
        r.collider = Some(e);
        built += 1;
        pending_near |= d < 5.0; // built this frame: not in the physics world until next step
    }
    // Remove colliders only once everything wanted exists: add first, remove later.
    if pending == 0 {
        let wanted: std::collections::HashSet<NodeId> = want.iter().map(|w| w.1).collect();
        for (id, r) in terrain.resident.iter_mut() {
            if !wanted.contains(id)
                && let Some(c) = r.collider.take()
            {
                commands.entity(c).despawn();
            }
        }
        for c in terrain.orphan_colliders.drain(..) {
            commands.entity(c).despawn();
        }
    }
    walking.colliders = terrain
        .resident
        .values()
        .filter(|r| r.collider.is_some())
        .count();
    walking.pending = pending;
    if pending_near {
        walking.ready_in = 2;
    } else if walking.ready_in > 0 {
        walking.ready_in -= 1;
    }
    walking.ready = walking.ready_in == 0 && walking.colliders > 0;
}

/// Cast the capsule down along `-up` and settle it 1 cm above walkable ground. `None` if there is
/// no walkable ground within [`SNAP_M`] below (steep ground: no snap, the character falls).
fn snap_down(
    spatial: &SpatialQuery,
    collider: &Collider,
    pos: Vec3,
    rotation: Quat,
    up: Vec3,
    filter: &SpatialQueryFilter,
) -> Option<Vec3> {
    let down = Dir3::new(-up).ok()?;
    let hit = spatial.cast_shape(
        collider,
        pos,
        rotation,
        down,
        &ShapeCastConfig::from_max_distance(SNAP_M),
        filter,
    )?;
    (hit.normal1.dot(up) > WALKABLE_COS).then(|| pos - up * (hit.distance - 0.01).max(0.0))
}

/// Fixed-step movement with `MoveAndSlide`. With `--auto-walk`, walks forward and jumps every
/// 3 s, and records how the character sits on the field surface.
#[allow(clippy::too_many_arguments)]
pub fn move_character(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    config: Res<Config>,
    terrain: Res<Terrain>,
    mut walking: ResMut<Walking>,
    move_and_slide: MoveAndSlide,
    spatial: SpatialQuery,
    cam: Query<&Transform, (With<FlyCam>, Without<Character>)>,
    mut ch: Query<(Entity, &Collider, &mut Transform), With<Character>>,
) {
    if !walking.active || !walking.ready {
        return;
    }
    let t = Instant::now();
    let Ok((entity, collider, mut tf)) = ch.single_mut() else {
        return;
    };
    let Ok(cam) = cam.single() else { return };
    let dt = time.delta_secs();
    // Local up at the character (radial), in the physics frame: gravity follows the body.
    let up = (walking.rot.inverse() * walking.to_body(walking.cur).normalize()).as_vec3();
    // Camera axes are body-local; bring them into the physics frame.
    let inv = walking.rot.inverse().as_quat();
    let flat = |v: Vec3| (v - up * v.dot(up)).normalize_or_zero();
    let fwd = flat(inv * *cam.forward());
    let right = flat(inv * *cam.right());
    let auto = config.auto_walk > 0.0;
    // Auto-walk cycle (9 s): walk 6 s with a jump at 1 s and 4 s, then stand still 3 s.
    let phase = walking.auto.t % 9.0;
    let auto_idle = auto && phase >= 6.0;
    let auto_jump = auto && !auto_idle && (phase % 3.0) < dt as f64 && walking.auto.t > 1.0;
    let mut dir = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) || (auto && !auto_idle) {
        dir += fwd;
    }
    if keys.pressed(KeyCode::KeyS) {
        dir -= fwd;
    }
    if keys.pressed(KeyCode::KeyD) {
        dir += right;
    }
    if keys.pressed(KeyCode::KeyA) {
        dir -= right;
    }
    let speed = if keys.pressed(KeyCode::ShiftLeft) {
        RUN
    } else {
        WALK
    };
    let jump = walking.grounded && (keys.pressed(KeyCode::Space) || auto_jump);
    let idle = dir == Vec3::ZERO && !jump && walking.grounded;
    let filter = SpatialQueryFilter::from_excluded_entities([entity]);
    // Standing still on walkable ground: no solver call at all, so no lateral motion is possible.
    // (Skipping the solver matters: with zero velocity, its depenetration pass against the terrain
    // trimeshes still crept ~5 mm per idle phase, which reads as "the character never stops".)
    // The ground snap stays, so the character follows the surface if it changes underfoot, and it
    // moves only along `up`.
    let (pos, grounded) = if idle {
        walking.vy = 0.0;
        match snap_down(&spatial, collider, walking.cur, tf.rotation, up, &filter) {
            Some(p) => (p, true),
            None => (walking.cur, false),
        }
    } else {
        if jump {
            walking.vy = JUMP;
        } else if walking.grounded {
            walking.vy = 0.0;
        } else {
            walking.vy -= 9.81 * dt;
        }
        let vel = dir.normalize_or_zero() * speed + up * walking.vy;
        let mut landed = false;
        let out = move_and_slide.move_and_slide(
            collider,
            tf.translation,
            tf.rotation,
            vel,
            Duration::from_secs_f32(dt),
            &MoveAndSlideConfig::default(),
            &filter,
            |hit| {
                if hit.normal.dot(up) > WALKABLE_COS {
                    landed = true;
                }
                MoveAndSlideHitResponse::Accept
            },
        );
        let pos = out.position;
        let (pos, grounded) = if walking.grounded && !jump {
            // Snap down onto walkable ground (walking downhill would otherwise leave the ground).
            // On ground steeper than walkable there is no snap: the character falls or slides,
            // which is the deliberate behaviour there.
            match snap_down(&spatial, collider, pos, tf.rotation, up, &filter) {
                Some(p) => (p, true),
                None => (pos, false),
            }
        } else {
            (pos, landed && walking.vy <= 0.0)
        };
        (pos, grounded)
    };
    walking.grounded = grounded;
    walking.prev = walking.cur;
    walking.cur = pos;
    tf.translation = pos;
    let idle_now = idle;
    walking.move_ms = t.elapsed().as_secs_f32() * 1000.0;
    if auto {
        let p = walking.to_body(walking.cur);
        let gap = p.length() - surface_radius(&terrain, p.normalize()) - (HALF + RADIUS) as f64;
        let (grounded, ms) = (walking.grounded, walking.move_ms);
        let step = (walking.cur - walking.prev).length() as f64;
        let a = &mut walking.auto;
        if idle_now && auto_idle {
            if a.idle_start.is_none() {
                a.idle_slope_max = a.idle_slope_max.max(slope_deg(&terrain, p.normalize()));
            }
            let start = *a.idle_start.get_or_insert(p);
            let d = p - start;
            // Drift is measured across the ground, not along the surface normal: settling onto the
            // ground (vertical, cm at most) is contact tracking, while sideways creep is the bug.
            let n = p.normalize();
            let flat = (d - n * d.dot(n)).length();
            a.idle_drift_max = a.idle_drift_max.max(flat);
            a.idle_settle_max = a.idle_settle_max.max(d.length());
            a.idle_steps += 1;
        } else {
            a.idle_start = None;
        }
        a.t += dt as f64;
        a.steps += 1;
        a.move_ms.push(ms);
        a.distance += step;
        if gap < -0.5 {
            a.fell += 1;
        }
        if grounded {
            a.grounded_steps += 1;
            a.gaps.push(gap);
        }
    }
}

/// `--auto-walk`: a screenshot halfway, then print the summary and exit.
pub fn auto_report(
    mut commands: Commands,
    config: Res<Config>,
    mut walking: ResMut<Walking>,
    mut exit: MessageWriter<AppExit>,
) {
    if config.auto_walk <= 0.0 || !walking.active {
        return;
    }
    let a = &mut walking.auto;
    if !a.shot && a.t > config.auto_walk / 2.0 {
        a.shot = true;
        if let Some(dir) = &config.shots {
            use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!("{dir}/walk.png")));
        }
    }
    if a.t < config.auto_walk {
        return;
    }
    a.gaps.sort_by(f64::total_cmp);
    a.move_ms.sort_by(f32::total_cmp);
    let p = |v: &[f64], q: f64| {
        v.get(((v.len().max(1) - 1) as f64 * q) as usize)
            .copied()
            .unwrap_or(f64::NAN)
    };
    let pm = |v: &[f32], q: f64| {
        v.get(((v.len().max(1) - 1) as f64 * q) as usize)
            .copied()
            .unwrap_or(f32::NAN)
    };
    println!(
        "{{ \"test\": \"auto-walk\", \"seconds\": {:.1}, \"distance_m\": {:.1}, \"steps\": {}, \"grounded_steps\": {}, \"fell_through_steps\": {}, \
         \"idle_steps\": {}, \"idle_drift_max_m\": {:.6}, \"idle_settle_max_m\": {:.4}, \"idle_slope_max_deg\": {:.1}, \
         \"ground_gap_m\": {{ \"p1\": {:.3}, \"p50\": {:.3}, \"p99\": {:.3} }}, \"move_ms\": {{ \"p50\": {:.3}, \"p99\": {:.3}, \"max\": {:.3} }} }}",
        a.t,
        a.distance,
        a.steps,
        a.grounded_steps,
        a.fell,
        a.idle_steps,
        a.idle_drift_max,
        a.idle_settle_max,
        a.idle_slope_max,
        p(&a.gaps, 0.01),
        p(&a.gaps, 0.5),
        p(&a.gaps, 0.99),
        pm(&a.move_ms, 0.5),
        pm(&a.move_ms, 0.99),
        a.move_ms.last().copied().unwrap_or(f32::NAN),
    );
    exit.write(AppExit::Success);
}
