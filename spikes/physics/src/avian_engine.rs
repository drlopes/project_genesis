//! Avian 0.8 implementation of the E6 tests (headless, fixed 60 Hz steps).

use crate::site::{Rng, Site, World, pct};
use crate::{Args, field_raycast, sites};
use avian3d::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use std::time::{Duration, Instant};

const HZ: f64 = 60.0;
const G: f32 = 9.81;
const SPEED_BUCKETS: usize = 6; // 0–50, 50–100, …, 250–300 m/s

#[derive(Component)]
struct Spawned;

struct Drop {
    e: Entity,
    bucket: usize,
    done: bool,
    speed: f64,
    tilt_deg: f64,
    was_over: bool,
    /// Went more than 0.5 m under the surface at some point.
    dipped: bool,
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::default()));
    app.insert_resource(Time::<Fixed>::from_hz(HZ));
    // Every update advances time by exactly one fixed step, so one update = one physics step.
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / HZ,
    )));
    app.finish();
    app.cleanup();
    app.update();
    app
}

fn clear(app: &mut App) {
    let w = app.world_mut();
    let es: Vec<Entity> = w
        .query_filtered::<Entity, With<Spawned>>()
        .iter(w)
        .collect();
    for e in es {
        w.despawn(e);
    }
}

fn spawn_terrain(app: &mut App, world: &World, site: &Site, margin: f32) -> usize {
    // Diagnostic: E6_SPAWN_K=3 spawns a 7×7 block while tests still evaluate the 5×5 one.
    let k = std::env::var("E6_SPAWN_K")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    let patches = site.patches(world, k);
    let n = patches.len();
    for (v, t) in patches {
        let mut ec = app.world_mut().spawn((
            Spawned,
            RigidBody::Static,
            Collider::trimesh_with_config(v, t, TrimeshFlags::FIX_INTERNAL_EDGES),
            Transform::default(),
        ));
        if margin > 0.0 {
            ec.insert(CollisionMargin(margin));
        }
    }
    n
}

/// One update (= one physics step); returns wall-clock ms.
fn step(app: &mut App) -> f64 {
    let t = Instant::now();
    app.update();
    t.elapsed().as_secs_f64() * 1000.0
}

fn position(app: &App, e: Entity) -> Option<Vec3> {
    app.world().get::<Position>(e).map(|p| p.0)
}

pub fn drops(world: &World, a: &Args) -> String {
    let mut app = app();
    if let Some(n) = a.substeps {
        app.insert_resource(SubstepCount(n));
    }
    let sites = sites(world, a, a.sites);
    let per_site = 1000 / a.sites;
    let total = per_site * sites.len();
    let mut rng = Rng(42);
    let (mut fell, mut drops_in) = ([0usize; SPEED_BUCKETS], [0usize; SPEED_BUCKETS]);
    let (mut left, mut drop_index, mut recovered) = (0usize, 0usize, 0usize);
    let mut step_ms = Vec::new();
    for site in &sites {
        clear(&mut app);
        app.insert_resource(Gravity(site.gravity(G)));
        spawn_terrain(&mut app, world, site, a.margin);
        let mut bodies = Vec::new();
        for _ in 0..per_site {
            let speed = 300.0 * drop_index as f64 / (total - 1) as f64;
            drop_index += 1;
            // Aim at a point within ±15 m of the site and start far enough back along the path
            // that impact comes ~0.1 s (6 steps) after spawning. A body spawned within one step of
            // the ground has no contact pair or previous position yet and passes straight through
            // (first harness version; see the E6 results).
            let impact = site.above_ground(world, rng.signed() * 8.0, rng.signed() * 8.0, 0.9);
            let (tilt, az) = (
                rng.unit() * 45f64.to_radians(),
                rng.unit() * std::f64::consts::TAU,
            );
            let d_local = DVec3::new(tilt.sin() * az.cos(), -tilt.cos(), tilt.sin() * az.sin());
            let start = impact - site.rot * d_local * (3.0 + speed * 0.1);
            let v = site.to_dir(site.rot * d_local) * speed as f32;
            let mut ec = app.world_mut().spawn((
                Spawned,
                RigidBody::Dynamic,
                Collider::capsule(0.4, 1.0),
                Transform::from_translation(site.body_to_phys(start)),
                LinearVelocity(v),
            ));
            // Avian sweeps fast bodies by default; the control must switch it off explicitly.
            let mut swept = SweptCcd::default();
            if !a.ccd {
                swept.filter = CcdFilter::NONE;
            }
            if a.linear_sweep {
                swept.mode = SweepMode::Linear;
            }
            if let Some(t) = a.ccd_threshold {
                swept.threshold = t;
            }
            ec.insert(swept);
            if a.solo {
                // Member of layer 1, collides only with layer 0 (the terrain's default layer).
                ec.insert(CollisionLayers::new(LayerMask(0b10), LayerMask(0b01)));
            }
            if a.speculative {
                ec.insert(SpeculativeCcd::default());
            }
            let bucket = ((speed / 50.0) as usize).min(SPEED_BUCKETS - 1);
            drops_in[bucket] += 1;
            bodies.push(Drop {
                e: ec.id(),
                bucket,
                done: false,
                speed,
                tilt_deg: tilt.to_degrees(),
                was_over: true,
                dipped: false,
            });
        }
        for s in 0..(2.0 * HZ) as usize {
            step_ms.push(step(&mut app));
            for b in bodies.iter_mut().filter(|b| !b.done) {
                let Some(p) = position(&app, b.e) else {
                    continue;
                };
                // Inner = the 3×3 patches around the site; a body below the terrain that was over
                // them a step earlier went through the terrain. A body that slid into the outer
                // ring and off the block's edge is not counted either way until it leaves.
                let inner = site.over_collision(world, p, 2, 1);
                let over = site.over_collision(world, p, 2, 0);
                let height = site.height_above_ground(world, p);
                if height < -0.5 && b.was_over && !b.dipped {
                    // Deep under the surface: either a real fall-through or a deep overlap the
                    // solver may still push back out. Keep simulating and decide at the end.
                    b.dipped = true;
                    if std::env::var_os("E6_LOG").is_some() {
                        let local = site.body_to_phys(site.phys_to_body(p));
                        let patch_edge =
                            world.lod.spacing(world.lod.max_level) * world.lod.intervals as f64;
                        let to_seam = |c: f32| {
                            let r = (c as f64).rem_euclid(patch_edge);
                            r.min(patch_edge - r)
                        };
                        eprintln!(
                            "fell: speed {:.0} m/s, tilt {:.0}°, step {s}, depth {:.2} m, local ({:.1}, {:.1}), \
                             approx. distance to a patch seam {:.1} m",
                            b.speed,
                            b.tilt_deg,
                            -height,
                            local.x,
                            local.z,
                            to_seam(local.x).min(to_seam(local.z))
                        );
                    }
                } else if !over && height > -0.5 {
                    left += 1; // slid or bounced off the collision area: not a fall-through
                    b.done = true;
                } else if b.dipped && height < -3.0 {
                    fell[b.bucket] += 1; // well under the terrain and still going: through it
                    b.done = true;
                }
                b.was_over = inner;
                if b.done {
                    app.world_mut().despawn(b.e);
                }
            }
        }
        // End of the run: a body that dipped and is still under the surface fell through; one
        // that dipped and came back up was a transient overlap.
        for b in bodies.iter().filter(|b| !b.done && b.dipped) {
            let h = position(&app, b.e)
                .map(|p| site.height_above_ground(world, p))
                .unwrap_or(f64::NAN);
            if h < -0.5 {
                fell[b.bucket] += 1;
            } else {
                recovered += 1;
            }
        }
    }
    step_ms.sort_by(f64::total_cmp);
    let buckets: Vec<String> = (0..SPEED_BUCKETS)
        .map(|i| {
            format!(
                "{{ \"speed\": \"{}-{}\", \"drops\": {}, \"fell\": {} }}",
                i * 50,
                i * 50 + 50,
                drops_in[i],
                fell[i]
            )
        })
        .collect();
    format!(
        "{{ \"engine\": \"avian 0.8\", \"test\": \"drops\", \"ccd\": {}, \"variant\": \"margin {} speculative {} linear {} threshold {:?} substeps {:?} solo {}\", \"sites\": {}, \"drops\": {total}, \
         \"fell_through\": {}, \"deep_overlap_recovered\": {recovered}, \"left_area\": {left}, \"by_speed\": [{}], \
         \"step_ms\": {{ \"p50\": {:.3}, \"p99\": {:.3}, \"max\": {:.3} }} }}",
        a.ccd,
        a.margin,
        a.speculative,
        a.linear_sweep,
        a.ccd_threshold,
        a.substeps,
        a.solo,
        sites.len(),
        fell.iter().sum::<usize>(),
        buckets.join(", "),
        pct(&step_ms, 0.5),
        pct(&step_ms, 0.99),
        step_ms.last().copied().unwrap_or(0.0),
    )
}

pub fn rays(world: &World, a: &Args) -> String {
    const N: usize = 10_000;
    let mut app = app();
    let mut rng = Rng(9);
    let (mut engine_ms, mut field_ms, mut diffs) = (Vec::new(), Vec::new(), Vec::new());
    let (mut engine_hits, mut field_hits, mut evals) = (0usize, 0usize, 0u64);
    let mut engine_misses = 0usize;
    for site in sites(world, a, 5) {
        clear(&mut app);
        spawn_terrain(&mut app, world, &site, a.margin);
        for _ in 0..3 {
            step(&mut app); // let the spatial query pipeline pick the colliders up
        }
        // Rays from 1–50 m above the ground, pointing down at 10–90° below the horizon.
        let rays: Vec<(DVec3, DVec3)> = (0..N)
            .map(|_| {
                let o = site.above_ground(
                    world,
                    rng.signed() * 20.0,
                    rng.signed() * 20.0,
                    1.0 + rng.unit() * 49.0,
                );
                let el = (10.0 + rng.unit() * 80.0f64).to_radians();
                let az = rng.unit() * std::f64::consts::TAU;
                let d = site.rot * DVec3::new(el.cos() * az.cos(), -el.sin(), el.cos() * az.sin());
                (o, d)
            })
            .collect();
        let phys: Vec<(Vec3, Vec3)> = rays
            .iter()
            .map(|(o, d)| (site.body_to_phys(*o), site.to_dir(*d).normalize()))
            .collect();
        let (hits, ms) = app
            .world_mut()
            .run_system_once(move |q: SpatialQuery| {
                let t = Instant::now();
                let filter = SpatialQueryFilter::default();
                let hits: Vec<Option<f32>> = phys
                    .iter()
                    .map(|(o, d)| {
                        q.cast_ray(*o, Dir3::new(*d).unwrap(), 200.0, true, &filter)
                            .map(|h| h.distance)
                    })
                    .collect();
                (hits, t.elapsed().as_secs_f64() * 1000.0)
            })
            .expect("ray system");
        engine_ms.push(ms);
        let t = Instant::now();
        let field: Vec<Option<f64>> = rays
            .iter()
            .map(|(o, d)| {
                field_raycast(world, *o, *d, 200.0).map(|(t, e)| {
                    evals += e as u64;
                    t
                })
            })
            .collect();
        field_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        // Compare only rays whose field hit lies over the collision block: beyond it there is no
        // mesh to hit, by construction.
        for ((e, f), (o, d)) in hits.iter().zip(&field).zip(&rays) {
            let Some(f) = f else { continue };
            let point = site.body_to_phys(*o + *d * *f);
            if !site.over_collision(world, point, 2, 0) {
                continue;
            }
            field_hits += 1;
            let inner = site.over_collision(world, point, 2, 1);
            let log = |what: &str| {
                if std::env::var_os("E6_LOG").is_some() {
                    let patch_edge =
                        world.lod.spacing(world.lod.max_level) * world.lod.intervals as f64;
                    let seam = |c: f32| {
                        let r = (c as f64).rem_euclid(patch_edge);
                        r.min(patch_edge - r)
                    };
                    eprintln!(
                        "{what}: field t {f:.2} m, engine {e:?}, hit local ({:.2}, {:.2}), inner block {inner}, \
                         elevation {:.0}°, approx. seam distance (east/north axes) {:.2} m",
                        point.x,
                        point.z,
                        (-site.to_dir(*d).y).asin().to_degrees(),
                        seam(point.x).min(seam(point.z))
                    );
                }
            };
            match e {
                Some(e) => {
                    engine_hits += 1;
                    let diff = (*e as f64 - f).abs();
                    if diff > 0.5 {
                        log("large difference");
                    }
                    diffs.push(diff);
                }
                None => {
                    engine_misses += 1;
                    log("engine miss");
                }
            }
        }
    }
    diffs.sort_by(f64::total_cmp);
    let n_rays = N * engine_ms.len();
    format!(
        "{{ \"engine\": \"avian 0.8\", \"test\": \"rays\", \"rays_per_batch\": {N}, \"batches\": {}, \
         \"engine_ms_per_10k\": {:.2}, \"field_trace_ms_per_10k\": {:.2}, \"field_samples_per_ray\": {:.1}, \
         \"field_hits_over_collision\": {field_hits}, \"engine_hits_there\": {engine_hits}, \"engine_misses_there\": {engine_misses}, \
         \"hit_distance_diff_m\": {{ \"p50\": {:.4}, \"p99\": {:.4}, \"max\": {:.4} }} }}",
        engine_ms.len(),
        engine_ms.iter().sum::<f64>() / engine_ms.len() as f64,
        field_ms.iter().sum::<f64>() / field_ms.len() as f64,
        evals as f64 / n_rays as f64,
        pct(&diffs, 0.5),
        pct(&diffs, 0.99),
        diffs.last().copied().unwrap_or(0.0),
    )
}

pub fn rest(world: &World, a: &Args) -> String {
    let mut app = app();
    let mut out = Vec::new();
    for site in sites(world, a, 4) {
        clear(&mut app);
        app.insert_resource(Gravity(site.gravity(G)));
        let flat = std::env::var_os("E6_FLAT").is_some();
        if flat {
            // Control: a flat engine-native box under the site instead of the terrain mesh, its
            // top at the site origin, tilted to the local up.
            // E6_FLAT=<degrees> tilts it about the local X axis to compare with sloped sites.
            let tilt = std::env::var("E6_FLAT")
                .ok()
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(0.0);
            let top = site.origin;
            let centre = top - site.dir * 0.5;
            let r = Quat::from_rotation_x(tilt.to_radians());
            app.world_mut().spawn((
                Spawned,
                RigidBody::Static,
                Collider::cuboid(200.0, 1.0, 200.0),
                Transform::from_translation(site.body_to_phys(centre)).with_rotation(
                    match site.frame {
                        crate::site::Frame::Local => r,
                        crate::site::Frame::Naive => site.rot.as_quat() * r,
                    },
                ),
            ));
        } else {
            spawn_terrain(&mut app, world, &site, a.margin);
        }
        // Local slope at the site, from the surface 1 m east and north.
        let (c, e, n) = (
            site.above_ground(world, 0.0, 0.0, 0.0),
            site.above_ground(world, 1.0, 0.0, 0.0),
            site.above_ground(world, 0.0, 1.0, 0.0),
        );
        let normal = (e - c).cross(n - c).normalize();
        let slope = normal.dot(site.dir).abs().acos().to_degrees();
        // Two boxes: one never sleeps (worst case), one may sleep (normal game setting).
        let spawn_box = |app: &mut App, east: f64, sleep: bool| {
            let p = if flat {
                site.origin + site.rot * DVec3::new(east, 0.6, 0.0)
            } else {
                site.above_ground(world, east, 0.0, 0.6)
            };
            let mut ec = app.world_mut().spawn((
                Spawned,
                RigidBody::Dynamic,
                Collider::cuboid(1.0, 1.0, 1.0),
                Transform::from_translation(site.body_to_phys(p)).with_rotation(match site.frame {
                    crate::site::Frame::Local => Quat::IDENTITY,
                    crate::site::Frame::Naive => site.rot.as_quat(),
                }),
            ));
            if !sleep {
                ec.insert(SleepingDisabled);
            }
            ec.id()
        };
        let awake = spawn_box(&mut app, -3.0, false);
        let sleeper = spawn_box(&mut app, 3.0, true);
        for _ in 0..(3.0 * HZ) as usize {
            step(&mut app);
        }
        let body_pos = |app: &App, e: Entity| position(app, e).map(|p| site.phys_to_body(p));
        let (a0, s0) = (body_pos(&app, awake), body_pos(&app, sleeper));
        let (mut max_a, mut max_s) = (0.0f64, 0.0f64);
        let steps = (a.minutes * 60.0 * HZ) as usize;
        let t = Instant::now();
        for _ in 0..steps {
            step(&mut app);
            if let (Some(p0), Some(p)) = (a0, body_pos(&app, awake)) {
                max_a = max_a.max((p - p0).length());
            }
            if let (Some(p0), Some(p)) = (s0, body_pos(&app, sleeper)) {
                max_s = max_s.max((p - p0).length());
            }
        }
        let below = |e: Entity| {
            position(&app, e)
                .map(|p| site.height_above_ground(world, p))
                .unwrap_or(f64::NAN)
        };
        out.push(format!(
            "{{ \"slope_deg\": {slope:.1}, \"awake_max_drift_m\": {max_a:.5}, \"sleeping_max_drift_m\": {max_s:.5}, \
             \"awake_height_above_ground_m\": {:.3}, \"sleeping_height_above_ground_m\": {:.3}, \"wall_s\": {:.1} }}",
            below(awake),
            below(sleeper),
            t.elapsed().as_secs_f64()
        ));
    }
    format!(
        "{{ \"engine\": \"avian 0.8\", \"test\": \"rest\", \"body_radius_m\": {}, \"frame\": \"{:?}\", \"minutes\": {}, \"sites\": [{}] }}",
        a.radius,
        a.frame,
        a.minutes,
        out.join(", ")
    )
}
