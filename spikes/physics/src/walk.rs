//! E6 character test: controller logic and statistics shared by both engines, so the comparison
//! measures the engines' collision and sliding, not two different controllers.
//!
//! The walker moves along local +X at a constant speed, jumps every few seconds, falls under
//! gravity when airborne, and pushes into the ground when grounded (no snapping, no auto-step, in
//! either engine).

use crate::site::{Site, World, pct};
use bevy::math::Vec3;

pub const HZ: f64 = 60.0;
pub const WALK_SPEED: f32 = 6.0;
pub const JUMP_EVERY_S: f64 = 3.0;
pub const JUMP_SPEED: f32 = 5.0;
/// Downward speed while grounded: keeps contact walking down slopes up to 45° at 6 m/s.
pub const STICK_SPEED: f32 = 6.0;
pub const CAPSULE_RADIUS: f32 = 0.3;
/// Half the capsule's segment length (total height 1.8 m).
pub const CAPSULE_HALF: f32 = 0.6;
pub const WALK_S: f64 = 16.0;
pub const START_X: f64 = -48.0;
pub const LOD_SWAP_S: f64 = 2.0;

#[derive(Default)]
pub struct Walker {
    pub vy: f32,
    pub t: f64,
    pub last_jump: f64,
}

impl Walker {
    /// Velocity to request this step (local frame: +Y up, walking along +X).
    pub fn velocity(&self) -> Vec3 {
        Vec3::new(WALK_SPEED, self.vy, 0.0)
    }

    pub fn jumping(&self) -> bool {
        self.t - self.last_jump < 0.3 && self.t > 0.3
    }

    pub fn after_step(&mut self, grounded: bool) {
        let dt = 1.0 / HZ;
        if grounded && !self.jumping() {
            if self.t - self.last_jump >= JUMP_EVERY_S {
                self.vy = JUMP_SPEED;
                self.last_jump = self.t;
            } else {
                self.vy = -STICK_SPEED;
            }
        } else {
            self.vy -= 9.81 * dt as f32;
        }
        self.t += dt;
    }
}

#[derive(Default)]
pub struct WalkStats {
    pub steps: usize,
    pub grounded_steps: usize,
    pub snag_steps: usize,
    pub snag_events: usize,
    pub snag_near_seam: usize,
    in_snag: bool,
    /// Capsule bottom minus field surface on near-flat ground (+ floats, − sinks).
    pub gap: Vec<f64>,
    pub fell: usize,
    pub swap_dy: Vec<f64>,
    pub normal_dy: Vec<f64>,
    pub step_ms: Vec<f64>,
}

pub struct StepRecord {
    pub before: Vec3,
    pub after: Vec3,
    pub grounded: bool,
    pub jumping: bool,
    pub swapped: bool,
    pub ms: f64,
    pub level: u8,
}

impl WalkStats {
    /// Returns false if the walker fell through (the walk for this site should end).
    pub fn record(&mut self, world: &World, site: &Site, r: &StepRecord) -> bool {
        self.steps += 1;
        self.step_ms.push(r.ms);
        let height = site.height_above_ground(world, r.after);
        if height < -0.5 {
            self.fell += 1;
            return false;
        }
        let (x, z) = (r.after.x as f64, r.after.z as f64);
        let slope = site.slope_deg(world, x, z);
        let dy = (r.after.y - r.before.y) as f64;
        if r.swapped {
            if r.grounded {
                self.swap_dy.push(dy.abs());
            }
        } else if r.grounded && !r.jumping {
            self.normal_dy.push(dy.abs());
        }
        if r.grounded && !r.jumping {
            self.grounded_steps += 1;
            if slope < 5.0 {
                self.gap
                    .push(height - (CAPSULE_HALF + CAPSULE_RADIUS) as f64);
            }
            let commanded = WALK_SPEED as f64 / HZ;
            let d = r.after - r.before;
            let achieved = ((d.x as f64).powi(2) + (d.z as f64).powi(2)).sqrt();
            let snag = slope < 35.0 && achieved < 0.5 * commanded;
            if snag {
                self.snag_steps += 1;
                if !self.in_snag {
                    self.snag_events += 1;
                    if site.seam_distance(world, r.level, r.after) < 1.0 {
                        self.snag_near_seam += 1;
                    }
                    if std::env::var_os("E6_LOG").is_some() {
                        eprintln!(
                            "snag: local ({x:.1}, {z:.1}), slope {slope:.0}°, achieved {achieved:.3} of {commanded:.3} m, seam {:.2} m",
                            site.seam_distance(world, r.level, r.after)
                        );
                    }
                }
            }
            self.in_snag = snag;
        } else {
            self.in_snag = false;
        }
        true
    }

    pub fn json(&mut self, engine: &str, lod_swap: bool, sites: usize) -> String {
        self.step_ms.sort_by(f64::total_cmp);
        self.gap.sort_by(f64::total_cmp);
        self.swap_dy.sort_by(f64::total_cmp);
        self.normal_dy.sort_by(f64::total_cmp);
        format!(
            "{{ \"engine\": \"{engine}\", \"test\": \"walk\", \"lod_swap\": {lod_swap}, \"sites\": {sites}, \"steps\": {}, \
             \"grounded_steps\": {}, \"fell_through\": {}, \"snag_steps\": {}, \"snag_events\": {}, \"snag_events_near_seam\": {}, \
             \"ground_gap_m\": {{ \"p1\": {:.4}, \"p50\": {:.4}, \"p99\": {:.4} }}, \
             \"grounded_step_dy_m\": {{ \"p99\": {:.4}, \"max\": {:.4} }}, \"lod_swap_step_dy_m\": {{ \"count\": {}, \"p50\": {:.4}, \"max\": {:.4} }}, \
             \"step_ms\": {{ \"p50\": {:.3}, \"p99\": {:.3}, \"max\": {:.3} }} }}",
            self.steps,
            self.grounded_steps,
            self.fell,
            self.snag_steps,
            self.snag_events,
            self.snag_near_seam,
            pct(&self.gap, 0.01),
            pct(&self.gap, 0.5),
            pct(&self.gap, 0.99),
            pct(&self.normal_dy, 0.99),
            self.normal_dy.last().copied().unwrap_or(0.0),
            self.swap_dy.len(),
            pct(&self.swap_dy, 0.5),
            self.swap_dy.last().copied().unwrap_or(0.0),
            pct(&self.step_ms, 0.5),
            pct(&self.step_ms, 0.99),
            self.step_ms.last().copied().unwrap_or(0.0),
        )
    }
}

/// Collision level for the walk: the finest level, or one coarser when toggled by a LOD swap,
/// with a block size covering the whole walk (±55 m or more).
pub fn block_for(world: &World, coarse: bool) -> (u8, i64) {
    let l = world.lod.max_level;
    if coarse { (l - 1, 2) } else { (l, 3) }
}
