//! Rapier (`bevy_rapier3d` 0.37) implementation of the E6 tests, mirroring `avian_engine.rs`.
//! Differences that matter: Rapier's CCD is opt-in per body (`Ccd::enabled()`), and its collision
//! margin equivalent is `ContactSkin`.

use crate::site::{Rng, Site, World, pct};
use crate::{Args, field_raycast, sites};
use bevy::ecs::system::RunSystemOnce;
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_rapier3d::prelude::*;
use std::time::{Duration, Instant};

const HZ: f64 = 60.0;
/// Settling time before a rest measurement starts (3 s was too short: boxes still tipped into
/// place afterwards).
const SETTLE_S: f64 = 10.0;
const G: f32 = 9.81;
const SPEED_BUCKETS: usize = 6;

#[derive(Component)]
struct Spawned;

struct Drop {
    e: Entity,
    bucket: usize,
    done: bool,
    was_over: bool,
    dipped: bool,
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule(),
    ));
    app.insert_resource(Time::<Fixed>::from_hz(HZ));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / HZ,
    )));
    app.finish();
    app.cleanup();
    app.update();
    app
}

fn set_gravity(app: &mut App, g: Vec3) {
    app.world_mut()
        .run_system_once(move |mut q: Query<&mut RapierConfiguration>| {
            for mut c in &mut q {
                c.gravity = g;
            }
        })
        .expect("gravity system");
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

fn spawn_terrain(app: &mut App, world: &World, site: &Site, margin: f32) {
    // Positive control for the fall-through detector: no terrain, so every drop must fall.
    if std::env::var_os("E6_NO_TERRAIN").is_some() {
        return;
    }
    let k = std::env::var("E6_SPAWN_K")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    for (v, t) in site.patches(world, k) {
        let collider = Collider::trimesh_with_flags(v, t, TriMeshFlags::FIX_INTERNAL_EDGES)
            .expect("terrain trimesh");
        let mut ec =
            app.world_mut()
                .spawn((Spawned, RigidBody::Fixed, collider, Transform::default()));
        if margin > 0.0 {
            ec.insert(ContactSkin(margin));
        }
    }
}

fn step(app: &mut App) -> f64 {
    let t = Instant::now();
    app.update();
    t.elapsed().as_secs_f64() * 1000.0
}

fn position(app: &App, e: Entity) -> Option<Vec3> {
    app.world().get::<Transform>(e).map(|t| t.translation)
}

pub fn drops(world: &World, a: &Args) -> String {
    let mut app = app();
    let sites = sites(world, a, a.sites);
    let per_site = 1000 / a.sites;
    let total = per_site * sites.len();
    let mut rng = Rng(42);
    let (mut fell, mut drops_in) = ([0usize; SPEED_BUCKETS], [0usize; SPEED_BUCKETS]);
    let (mut left, mut drop_index, mut recovered) = (0usize, 0usize, 0usize);
    let mut step_ms = Vec::new();
    let mut moved = 0usize;
    let mut starts: Vec<(Entity, Vec3)> = Vec::new();
    for site in &sites {
        clear(&mut app);
        set_gravity(&mut app, site.gravity(G));
        spawn_terrain(&mut app, world, site, a.margin);
        let mut bodies = Vec::new();
        for _ in 0..per_site {
            let speed = 300.0 * drop_index as f64 / (total - 1) as f64;
            drop_index += 1;
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
                Collider::capsule_y(0.5, 0.4),
                Transform::from_translation(site.body_to_phys(start)),
                Velocity::linear(v),
            ));
            if a.ccd {
                ec.insert(Ccd::enabled());
            }
            if a.solo {
                ec.insert(CollisionGroups::new(Group::GROUP_2, Group::GROUP_1));
            }
            let bucket = ((speed / 50.0) as usize).min(SPEED_BUCKETS - 1);
            drops_in[bucket] += 1;
            starts.push((ec.id(), site.body_to_phys(start)));
            bodies.push(Drop {
                e: ec.id(),
                bucket,
                done: false,
                was_over: true,
                dipped: false,
            });
        }
        for s in 0..(2.0 * HZ) as usize {
            step_ms.push(step(&mut app));
            if s == 2 {
                // Sanity: the simulation must actually move bodies (checked after 3 steps).
                for (e, p0) in &starts {
                    if let Some(p) = position(&app, *e)
                        && (p - *p0).length() > 0.5
                    {
                        moved += 1;
                    }
                }
                starts.clear();
            }
            for b in bodies.iter_mut().filter(|b| !b.done) {
                let Some(p) = position(&app, b.e) else {
                    continue;
                };
                let inner = site.over_collision(world, p, 2, 1);
                let over = site.over_collision(world, p, 2, 0);
                let height = site.height_above_ground(world, p);
                if height < -0.5 && b.was_over && !b.dipped {
                    b.dipped = true;
                } else if !over && height > -0.5 {
                    left += 1;
                    b.done = true;
                } else if b.dipped && height < -3.0 {
                    fell[b.bucket] += 1;
                    b.done = true;
                }
                b.was_over = inner;
                if b.done {
                    app.world_mut().despawn(b.e);
                }
            }
        }
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
        "{{ \"engine\": \"rapier 0.37\", \"test\": \"drops\", \"ccd\": {}, \"variant\": \"skin {} solo {}\", \"sites\": {}, \"drops\": {total}, \
         \"moved_after_3_steps\": {moved}, \"fell_through\": {}, \"deep_overlap_recovered\": {recovered}, \"left_area\": {left}, \"by_speed\": [{}], \
         \"step_ms\": {{ \"p50\": {:.3}, \"p99\": {:.3}, \"max\": {:.3} }} }}",
        a.ccd,
        a.margin,
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
    let (mut engine_hits, mut field_hits, mut engine_misses) = (0usize, 0usize, 0usize);
    for site in sites(world, a, 5) {
        clear(&mut app);
        spawn_terrain(&mut app, world, &site, a.margin);
        for _ in 0..3 {
            step(&mut app);
        }
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
            .run_system_once(move |ctx: ReadRapierContext| {
                let c = ctx.single().expect("rapier context");
                let t = Instant::now();
                let hits: Vec<Option<f32>> = phys
                    .iter()
                    .map(|(o, d)| {
                        c.cast_ray(*o, *d, 200.0, true, QueryFilter::default())
                            .map(|(_, toi)| toi)
                    })
                    .collect();
                (hits, t.elapsed().as_secs_f64() * 1000.0)
            })
            .expect("ray system");
        engine_ms.push(ms);
        let t = Instant::now();
        let field: Vec<Option<f64>> = rays
            .iter()
            .map(|(o, d)| field_raycast(world, *o, *d, 200.0).map(|(t, _)| t))
            .collect();
        field_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        for ((e, f), (o, d)) in hits.iter().zip(&field).zip(&rays) {
            let Some(f) = f else { continue };
            if !site.over_collision(world, site.body_to_phys(*o + *d * *f), 2, 0) {
                continue;
            }
            field_hits += 1;
            match e {
                Some(e) => {
                    engine_hits += 1;
                    diffs.push((*e as f64 - f).abs());
                }
                None => engine_misses += 1,
            }
        }
    }
    diffs.sort_by(f64::total_cmp);
    format!(
        "{{ \"engine\": \"rapier 0.37\", \"test\": \"rays\", \"rays_per_batch\": {N}, \"batches\": {}, \
         \"engine_ms_per_10k\": {:.2}, \"field_trace_ms_per_10k\": {:.2}, \
         \"field_hits_over_collision\": {field_hits}, \"engine_hits_there\": {engine_hits}, \"engine_misses_there\": {engine_misses}, \
         \"hit_distance_diff_m\": {{ \"p50\": {:.4}, \"p99\": {:.4}, \"max\": {:.4} }} }}",
        engine_ms.len(),
        engine_ms.iter().sum::<f64>() / engine_ms.len() as f64,
        field_ms.iter().sum::<f64>() / field_ms.len() as f64,
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
        set_gravity(&mut app, site.gravity(G));
        let flat = std::env::var("E6_FLAT")
            .ok()
            .and_then(|v| v.parse::<f32>().ok());
        let frame_rot = match site.frame {
            crate::site::Frame::Local => Quat::IDENTITY,
            crate::site::Frame::Naive => site.rot.as_quat(),
        };
        if let Some(tilt) = flat {
            let centre = site.origin - site.dir * 0.5;
            app.world_mut().spawn((
                Spawned,
                RigidBody::Fixed,
                Collider::cuboid(100.0, 0.5, 100.0),
                Transform::from_translation(site.body_to_phys(centre))
                    .with_rotation(frame_rot * Quat::from_rotation_x(tilt.to_radians())),
            ));
        } else {
            spawn_terrain(&mut app, world, &site, a.margin);
        }
        let (c, e, n) = (
            site.above_ground(world, 0.0, 0.0, 0.0),
            site.above_ground(world, 1.0, 0.0, 0.0),
            site.above_ground(world, 0.0, 1.0, 0.0),
        );
        let normal = (e - c).cross(n - c).normalize();
        let slope = normal.dot(site.dir).abs().acos().to_degrees();
        let spawn_box = |app: &mut App, east: f64, sleep: bool| {
            let p = if flat.is_some() {
                site.origin + site.rot * DVec3::new(east, 0.6, 0.0)
            } else {
                site.above_ground(world, east, 0.0, 0.6)
            };
            let mut ec = app.world_mut().spawn((
                Spawned,
                RigidBody::Dynamic,
                Collider::cuboid(0.5, 0.5, 0.5),
                Transform::from_translation(site.body_to_phys(p)).with_rotation(frame_rot),
            ));
            if !sleep {
                ec.insert(Sleeping::disabled());
            }
            ec.id()
        };
        let awake = spawn_box(&mut app, -3.0, false);
        let sleeper = spawn_box(&mut app, 3.0, true);
        // Diagnostic: E6_TRACE=1 prints the sleeping box's state every 0.25 s for 20 s.
        if std::env::var_os("E6_TRACE").is_some() {
            let p0 = site.phys_to_body(position(&app, sleeper).unwrap_or_default());
            let r0 = app
                .world()
                .get::<Transform>(sleeper)
                .map(|t| t.rotation)
                .unwrap_or_default();
            for i in 0..(20.0 * HZ) as usize {
                step(&mut app);
                if i % 15 == 0 {
                    let t = app
                        .world()
                        .get::<Transform>(sleeper)
                        .copied()
                        .unwrap_or_default();
                    let sleeping = app.world().get::<Sleeping>(sleeper).map(|s| s.sleeping);
                    let v = app
                        .world()
                        .get::<Velocity>(sleeper)
                        .map(|v| v.linear.length());
                    eprintln!(
                        "t {:5.2} s | height {:.3} m | moved {:.3} m | rotated {:5.1}° | speed {:?} | sleeping {:?}",
                        i as f64 / HZ,
                        site.height_above_ground(world, t.translation),
                        (site.phys_to_body(t.translation) - p0).length(),
                        t.rotation.angle_between(r0).to_degrees(),
                        v,
                        sleeping
                    );
                }
            }
            return String::from("{ \"trace\": true }");
        }
        for _ in 0..(SETTLE_S * HZ) as usize {
            step(&mut app);
        }
        let body_pos = |app: &App, e: Entity| position(app, e).map(|p| site.phys_to_body(p));
        let (a0, s0) = (body_pos(&app, awake), body_pos(&app, sleeper));
        let (mut max_a, mut max_s) = (0.0f64, 0.0f64);
        // When each box first moved more than 1 cm after settling (seconds), to tell a single
        // event (tipping, a slide) from steady creep.
        let (mut first_a, mut first_s) = (None::<f64>, None::<f64>);
        let t = Instant::now();
        let steps = (a.minutes * 60.0 * HZ) as usize;
        for i in 0..steps {
            step(&mut app);
            let ts = i as f64 / HZ;
            if let (Some(p0), Some(p)) = (a0, body_pos(&app, awake)) {
                let d = (p - p0).length();
                max_a = max_a.max(d);
                if d > 0.01 && first_a.is_none() {
                    first_a = Some(ts);
                }
            }
            if let (Some(p0), Some(p)) = (s0, body_pos(&app, sleeper)) {
                let d = (p - p0).length();
                max_s = max_s.max(d);
                if d > 0.01 && first_s.is_none() {
                    first_s = Some(ts);
                }
            }
        }
        let end = |e: Entity, p0: Option<DVec3>| match (p0, body_pos(&app, e)) {
            (Some(p0), Some(p)) => (p - p0).length(),
            _ => f64::NAN,
        };
        let opt = |v: Option<f64>| v.map_or("null".to_string(), |s| format!("{s:.1}"));
        out.push(format!(
            "{{ \"slope_deg\": {slope:.1}, \"awake_max_drift_m\": {max_a:.5}, \"sleeping_max_drift_m\": {max_s:.5}, \
             \"awake_end_drift_m\": {:.5}, \"sleeping_end_drift_m\": {:.5}, \
             \"awake_first_move_s\": {}, \"sleeping_first_move_s\": {}, \"wall_s\": {:.1} }}",
            end(awake, a0),
            end(sleeper, s0),
            opt(first_a),
            opt(first_s),
            t.elapsed().as_secs_f64()
        ));
    }
    format!(
        "{{ \"engine\": \"rapier 0.37\", \"test\": \"rest\", \"body_radius_m\": {}, \"frame\": \"{:?}\", \"minutes\": {}, \"flat_control_tilt_deg\": {}, \"sites\": [{}] }}",
        a.radius,
        a.frame,
        a.minutes,
        std::env::var("E6_FLAT").unwrap_or_else(|_| "null".into()),
        out.join(", ")
    )
}
