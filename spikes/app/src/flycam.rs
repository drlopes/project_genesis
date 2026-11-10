//! Free-flight camera orientation on a sphere, without a fixed reference direction.
//!
//! The first version measured heading from "north" (world +Y projected onto the horizon). That
//! frame is singular at the poles: flying over one, north swings round, the code switched to a
//! fallback axis, and the camera pivoted or flipped by itself. Here the orientation is carried
//! along as the local up changes (parallel transport) and re-levelled, so there is no special
//! point anywhere on the body.

use bevy::prelude::*;

/// Pitch limit, short of straight up/down so the horizon frame stays well defined.
pub const MAX_PITCH: f32 = 1.55;

/// Rebuild `rot` with the same forward direction and no roll relative to `up`.
pub fn level(rot: Quat, up: Vec3) -> Quat {
    let fwd = rot * Vec3::NEG_Z;
    Transform::IDENTITY.looking_to(fwd, up).rotation
}

/// Carry the orientation from the old local up to the new one (minimal rotation), then re-level.
pub fn transport(rot: Quat, old_up: Vec3, new_up: Vec3) -> Quat {
    level(Quat::from_rotation_arc(old_up, new_up) * rot, new_up)
}

/// Yaw about the local up, then pitch about the camera's right axis, clamped to `MAX_PITCH`.
pub fn look(rot: Quat, up: Vec3, d_yaw: f32, d_pitch: f32) -> Quat {
    let r = Quat::from_axis_angle(up, d_yaw) * rot;
    let pitch = (r * Vec3::NEG_Z).dot(up).clamp(-1.0, 1.0).asin();
    let target = (pitch + d_pitch).clamp(-MAX_PITCH, MAX_PITCH);
    let r = Quat::from_axis_angle(r * Vec3::X, target - pitch) * r;
    level(r, up)
}

/// Initial orientation at `pos`: looking `pitch` below the horizon, heading towards world +Y (or
/// +X if `pos` is on the Y axis).
pub fn initial(pos: Vec3, pitch: f32) -> Quat {
    let up = pos.normalize();
    let reference = if up.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
    let north = (reference - up * up.dot(reference)).normalize();
    let fwd = north * pitch.cos() + up * pitch.sin();
    Transform::IDENTITY.looking_to(fwd, up).rotation
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The original frame, kept to show the bug: heading measured from projected world +Y.
    fn old_rotation(up: Vec3, yaw: f32, pitch: f32) -> Quat {
        let north_ref = if up.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
        let north = (north_ref - up * up.dot(north_ref)).normalize();
        Transform::IDENTITY.looking_to(north, up).rotation
            * Quat::from_rotation_y(yaw)
            * Quat::from_rotation_x(pitch)
    }

    /// Hold W, looking at the horizon, starting heading north: a full lap over both poles.
    /// Returns the largest per-step change in view direction, in degrees.
    fn fly_lap(new: bool) -> f32 {
        let r = 25_000.0f32;
        let mut pos = Vec3::new(0.0, 0.0, r);
        let mut rot = initial(pos, 0.0);
        let mut up = pos.normalize();
        let mut worst = 0.0f32;
        let steps = 4_000;
        let step = std::f32::consts::TAU * r / steps as f32;
        for _ in 0..steps {
            let fwd_before = rot * Vec3::NEG_Z;
            pos = (pos + fwd_before * step).normalize() * r;
            let new_up = pos.normalize();
            rot = if new {
                transport(rot, up, new_up)
            } else {
                old_rotation(new_up, 0.0, 0.0)
            };
            up = new_up;
            let fwd_after = rot * Vec3::NEG_Z;
            assert!(fwd_after.is_finite(), "orientation became NaN");
            // Expected change per step is the arc angle; anything beyond that is the camera turning.
            let turn = fwd_before.angle_between(fwd_after).to_degrees() - (step / r).to_degrees();
            worst = worst.max(turn);
        }
        worst
    }

    #[test]
    fn crossing_the_poles_does_not_turn_the_camera() {
        let old = fly_lap(false);
        let new = fly_lap(true);
        println!("worst unintended turn per step over the poles: old {old:.1}°, new {new:.4}°");
        assert!(
            old > 45.0,
            "the old frame should reproduce the pivot (got {old}°)"
        );
        assert!(new < 0.01, "transported frame turned by itself: {new}°");
    }

    #[test]
    fn look_keeps_the_horizon_level_and_clamps_pitch() {
        let pos = Vec3::new(3.0, 17.0, -9.0).normalize() * 24_000.0;
        let up = pos.normalize();
        let mut rot = initial(pos, -0.3);
        for i in 0..500 {
            rot = look(rot, up, 0.031, if i < 250 { 0.02 } else { -0.03 });
            let right = rot * Vec3::X;
            assert!(right.dot(up).abs() < 1e-4, "camera rolled");
            let pitch = (rot * Vec3::NEG_Z).dot(up).asin();
            assert!(pitch.abs() <= MAX_PITCH + 1e-4);
        }
    }
}
