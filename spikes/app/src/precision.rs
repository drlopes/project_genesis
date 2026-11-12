//! E5: emulate the GPU's single-precision vertex path and measure screen-space error.
//!
//! Two frames for placing a body `system_distance` metres from the system origin:
//!
//! - **Naive:** entity and camera translations are system positions in f32 (Bevy's default use).
//! - **Relative:** positions live in f64; each frame, render translations are
//!   `(entity - camera)` computed in f64, then cast to f32. The camera sits at the origin.
//!
//! The vertex path is modelled as Bevy's: `world = model_translation + local` (f32), then
//! `view = rotation · (world - camera_translation)` (f32), then a pinhole projection. The error is
//! the distance in pixels between that and the exact f64 projection of the same point.

use bevy::math::{DQuat, DVec3, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    Naive,
    Relative,
}

/// Exact (f64) and emulated (f32) screen position of a point, in pixels from the image centre.
/// `entity` and `camera` are system positions; `local` is the vertex offset inside the entity.
pub fn project_error_px(
    frame: Frame,
    entity: DVec3,
    local: DVec3,
    camera: DVec3,
    rotation: DQuat,
    px_per_radian: f64,
) -> f64 {
    let inv = rotation.inverse();
    let exact = inv * (entity + local - camera);
    let (model, cam) = match frame {
        Frame::Naive => (entity.as_vec3(), camera.as_vec3()),
        Frame::Relative => ((entity - camera).as_vec3(), Vec3::ZERO),
    };
    let world = model + local.as_vec3();
    let view = inv.as_quat() * (world - cam);
    let p = |v: DVec3| DVec3::new(v.x / -v.z, v.y / -v.z, 0.0) * px_per_radian;
    let approx = DVec3::new(view.x as f64, view.y as f64, view.z as f64);
    (p(exact) - p(approx)).length()
}

/// Worst error over a short walk on the surface: a camera 1.7 m above the ground walking 10 m, and
/// probe points on the ground 2–2000 m ahead. Patch-sized entities (local offsets up to ~300 m).
pub fn worst_error_px(frame: Frame, system_distance: f64, body_radius: f64) -> f64 {
    let px_per_radian = 1440.0 / (2.0 * 30f64.to_radians().tan());
    let body = DVec3::new(system_distance, 0.3 * system_distance, 0.0);
    let up = DVec3::new(0.2, 0.9, 0.4).normalize();
    let east = up.cross(DVec3::Z).normalize();
    let mut worst = 0.0f64;
    for step in 0..50 {
        let surface = body + up * body_radius + east * (step as f64 * 0.2);
        let camera = surface + up * 1.7;
        // Looking along `east`, slightly down.
        let fwd = (east - up * 0.1).normalize();
        let rotation = DQuat::from_mat3(&bevy::math::DMat3::from_cols(
            fwd.cross(up).normalize(),
            up,
            -fwd,
        ));
        for &ahead in &[2.0, 10.0, 50.0, 200.0, 2000.0] {
            // The patch entity's origin a few hundred metres back; the vertex is its offset.
            let point = surface + east * ahead;
            let entity = point - east * 150.0 - up * 3.0;
            let local = point - entity;
            worst = worst.max(project_error_px(
                frame,
                entity,
                local,
                camera,
                rotation,
                px_per_radian,
            ));
        }
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints the E5 table and checks both frames behave as theory says: the naive frame fails
    /// long before 10^10 m, the relative frame is sub-pixel everywhere.
    #[test]
    fn relative_frame_is_subpixel_at_system_scale_and_naive_is_not() {
        println!("system distance | naive px | relative px");
        for d in [0.0, 1e4, 1e6, 1e8, 1e9, 1e10] {
            let n = worst_error_px(Frame::Naive, d, 24_000.0);
            let r = worst_error_px(Frame::Relative, d, 24_000.0);
            println!("{d:>15.0e} | {n:>10.3} | {r:>10.5}");
            assert!(r < 0.1, "relative frame: {r} px at {d} m");
        }
        assert!(
            worst_error_px(Frame::Naive, 1e10, 24_000.0) > 10.0,
            "control: naive should fail"
        );
    }
}
