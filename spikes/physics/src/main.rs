//! E6 — collision and physics, headless (docs/21-phase-1-spike-plan.md#e6--collision-and-physics).
//!
//! Terrain collision is option (a) from the plan: per-patch triangle meshes built from the same
//! vertices as the visual patches (finest level), around each test site. Neither engine accepts a
//! custom field collider cleanly (see docs/spikes/E6-collision-and-physics.md).
//!
//!   cargo run --release -p spike_physics -- drops [--no-ccd] [--sites 50]
//!   cargo run --release -p spike_physics -- rays
//!   cargo run --release -p spike_physics -- rest [--body-radius 6.371e6] [--frame naive] [--minutes 10]
//!
//! Engine: `--features avian` (default) or `--no-default-features --features rapier`.

mod site;

#[cfg(feature = "avian")]
mod avian_engine;
#[cfg(feature = "avian")]
use avian_engine as engine;
#[cfg(all(feature = "rapier", not(feature = "avian")))]
mod rapier_engine;
#[cfg(all(feature = "rapier", not(feature = "avian")))]
use rapier_engine as engine;

use bevy::math::DVec3;
use site::{Frame, Site, World};

pub struct Args {
    pub test: String,
    pub ccd: bool,
    pub sites: usize,
    pub radius: f64,
    pub frame: Frame,
    pub minutes: f64,
    pub out: Option<String>,
    /// Engine tuning variants for the drop test.
    pub margin: f32,
    pub speculative: bool,
    pub linear_sweep: bool,
    pub ccd_threshold: Option<f32>,
    pub substeps: Option<u32>,
    /// Drop-test bodies collide only with the terrain, not with each other.
    pub solo: bool,
}

fn args() -> Args {
    let a: Vec<String> = std::env::args().collect();
    let get = |n: &str| {
        a.iter()
            .position(|x| x == n)
            .and_then(|i| a.get(i + 1))
            .cloned()
    };
    Args {
        test: a.get(1).cloned().unwrap_or_else(|| "drops".into()),
        ccd: !a.iter().any(|x| x == "--no-ccd"),
        sites: get("--sites").and_then(|v| v.parse().ok()).unwrap_or(50),
        radius: get("--body-radius")
            .and_then(|v| v.parse().ok())
            .unwrap_or(24_000.0),
        frame: if get("--frame").as_deref() == Some("naive") {
            Frame::Naive
        } else {
            Frame::Local
        },
        minutes: get("--minutes")
            .and_then(|v| v.parse().ok())
            .unwrap_or(10.0),
        out: get("--out"),
        margin: get("--margin").and_then(|v| v.parse().ok()).unwrap_or(0.0),
        speculative: a.iter().any(|x| x == "--speculative"),
        linear_sweep: a.iter().any(|x| x == "--linear"),
        ccd_threshold: get("--ccd-threshold").and_then(|v| v.parse().ok()),
        substeps: get("--substeps").and_then(|v| v.parse().ok()),
        solo: a.iter().any(|x| x == "--solo"),
    }
}

/// Sphere trace against the field (docs/18 "Raycasting a star-shaped solid"), body-local f64.
/// `g(p) = |p| − surfaceRadius(p)`: positive above ground. Returns the hit distance.
pub fn field_raycast(world: &World, origin: DVec3, dir: DVec3, max: f64) -> Option<(f64, u32)> {
    let tol = 0.02f64.max(0.5 * world.lod.spacing(world.lod.max_level));
    let slope = 2.0; // |∇h| bound: ~63°, conservative for the spike body
    let (min_step, max_step): (f64, f64) = (0.05, 20.0);
    let g = |t: f64| {
        let p = origin + dir * t;
        p.length() - world.surface_radius(p.normalize())
    };
    let (mut t, mut gt) = (0.0, g(0.0));
    let mut evals = 1u32;
    for _ in 0..512 {
        if gt <= tol {
            // Within tolerance but maybe not yet across: walk in small steps until the segment
            // brackets the crossing (g changes sign), then bisect it. (First version bisected an
            // unbracketed segment and returned hits ~tol short.)
            let mut lo = t;
            while gt > 0.0 {
                lo = t;
                t += min_step;
                if t > max {
                    return None;
                }
                gt = g(t);
                evals += 1;
            }
            let mut hi = t;
            if lo == hi {
                lo = (t - min_step).max(0.0);
            }
            for _ in 0..20 {
                let mid = 0.5 * (lo + hi);
                evals += 1;
                if g(mid) > 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Some((hi, evals));
        }
        let step = (gt / (1.0 + slope)).clamp(min_step, max_step);
        t += step;
        if t > max {
            return None;
        }
        gt = g(t);
        evals += 1;
    }
    None
}

fn main() {
    let a = args();
    let world = World::new(a.radius);
    println!(
        "body radius {} m, finest level {} (spacing {:.3} m), frame {:?}",
        a.radius,
        world.lod.max_level,
        world.lod.spacing(world.lod.max_level),
        a.frame
    );
    let json = match a.test.as_str() {
        "drops" => engine::drops(&world, &a),
        "rays" => engine::rays(&world, &a),
        "rest" => engine::rest(&world, &a),
        t => panic!("unknown test {t}"),
    };
    println!("{json}");
    if let Some(path) = &a.out {
        std::fs::write(path, &json).expect("write results");
    }
}

/// Shared by the engine modules.
pub fn sites(world: &World, a: &Args, count: usize) -> Vec<Site> {
    site::site_dirs(count, 7)
        .into_iter()
        .map(|d| Site::at(world, d, a.frame))
        .collect()
}
