//! Cube-sphere quadtree: node identity, geometry, level ↔ band mapping, and SSE selection
//! (docs/10-lod-and-streaming.md#quadtree-level-versus-lod-band).

use spike_field::BAND_SPACING;
use spike_field::cube::{Vec3, dir_of};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId {
    pub face: u8,
    pub level: u8,
    pub x: u32,
    pub y: u32,
}

impl NodeId {
    pub fn root(face: u8) -> NodeId {
        NodeId {
            face,
            level: 0,
            x: 0,
            y: 0,
        }
    }

    pub fn children(self) -> [NodeId; 4] {
        let (l, x, y) = (self.level + 1, self.x * 2, self.y * 2);
        let f = self.face;
        [
            NodeId {
                face: f,
                level: l,
                x,
                y,
            },
            NodeId {
                face: f,
                level: l,
                x: x + 1,
                y,
            },
            NodeId {
                face: f,
                level: l,
                x,
                y: y + 1,
            },
            NodeId {
                face: f,
                level: l,
                x: x + 1,
                y: y + 1,
            },
        ]
    }

    /// True if `self` is a strict ancestor of `other`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_ancestor_of(self, other: NodeId) -> bool {
        if self.face != other.face || self.level >= other.level {
            return false;
        }
        let shift = other.level - self.level;
        other.x >> shift == self.x && other.y >> shift == self.y
    }

    pub fn ancestor(self, level: u8) -> NodeId {
        let shift = self.level - level;
        NodeId {
            face: self.face,
            level,
            x: self.x >> shift,
            y: self.y >> shift,
        }
    }

    /// Face-space range `[u0, u1] × [v0, v1]` in `[-1, 1]`.
    pub fn uv_range(self) -> (f64, f64, f64, f64) {
        let n = (1u64 << self.level) as f64;
        let u0 = -1.0 + 2.0 * self.x as f64 / n;
        let v0 = -1.0 + 2.0 * self.y as f64 / n;
        (u0, u0 + 2.0 / n, v0, v0 + 2.0 / n)
    }

    pub fn centre_dir(self) -> Vec3 {
        let (u0, u1, v0, v1) = self.uv_range();
        dir_of(self.face as usize, 0.5 * (u0 + u1), 0.5 * (v0 + v1))
    }
}

/// Body-dependent LOD parameters.
#[derive(Clone, Copy, Debug)]
pub struct Lod {
    pub radius: f64,
    pub intervals: u32,
    pub max_level: u8,
    pub sse_px: f64,
    pub spacing_scale: f64,
    /// Lowest terrain height on the body (metres above the reference sphere), conservative.
    pub min_height: f64,
}

impl Lod {
    pub fn new(radius: f64, grid: u32, sse_px: f64, spacing_scale: f64) -> Lod {
        let intervals = grid - 1;
        let face_arc = std::f64::consts::FRAC_PI_2 * radius;
        // Deepen until the patch spacing reaches band 4's max spacing.
        let target = BAND_SPACING[4] * spacing_scale;
        let mut max_level = 0u8;
        while face_arc / ((1u64 << max_level) as f64 * intervals as f64) > target && max_level < 24
        {
            max_level += 1;
        }
        Lod {
            radius,
            intervals,
            max_level,
            sse_px,
            spacing_scale,
            min_height: -2_500.0,
        }
    }

    /// Use the body's real lowest terrain for horizon culling instead of the -2.5 km default.
    pub fn with_min_height(mut self, min_height: f64) -> Lod {
        self.min_height = min_height;
        self
    }

    /// Approximate sample spacing of a patch at `level` (docs/10).
    pub fn spacing(&self, level: u8) -> f64 {
        let face_arc = std::f64::consts::FRAC_PI_2 * self.radius;
        face_arc / ((1u64 << level) as f64 * self.intervals as f64)
    }

    /// The finest band whose (scaled) max spacing is >= the patch spacing.
    pub fn band(&self, level: u8) -> usize {
        let s = self.spacing(level);
        (0..5)
            .rev()
            .find(|&b| BAND_SPACING[b] * self.spacing_scale >= s)
            .unwrap_or(0)
    }

    /// Bounding radius of a patch around its centre point *on the terrain*: the horizontal
    /// half-diagonal plus a vertical margin that scales with patch size (relief within a patch is
    /// bounded by roughly half its edge, and by the body's total relief of about ±2.5 km).
    ///
    /// E3 finding: a fixed 2.5 km margin made every patch within 2.5 km look 1 m away, so the
    /// whole neighbourhood subdivided to the deepest level (~200 000 queued patches).
    pub fn bound_radius(&self, level: u8) -> f64 {
        let edge = std::f64::consts::FRAC_PI_2 * self.radius / (1u64 << level) as f64;
        0.75 * edge + (0.5 * edge).min(2_500.0)
    }
}

/// Select the leaf set (a cut through the tree) for a camera.
///
/// Splits when one sample projects to more than `sse_px` pixels; a node that was split last frame
/// stays split until the error drops below half that (hysteresis).
/// `ground(dir)` gives the terrain height (above the reference sphere) at a direction; the node's
/// distance is measured to its centre *on the terrain*.
pub fn select(
    lod: &Lod,
    cam: Vec3,
    px_per_radian: f64,
    split: &mut HashSet<NodeId>,
    ground: &dyn Fn(Vec3) -> f64,
) -> Vec<NodeId> {
    let mut leaves = Vec::new();
    let mut new_split = HashSet::new();
    let mut stack: Vec<NodeId> = (0..6).map(NodeId::root).collect();
    let cam_len = (cam[0] * cam[0] + cam[1] * cam[1] + cam[2] * cam[2]).sqrt();
    // Horizon culling against the lowest possible terrain sphere (conservative).
    let occluder = lod.radius + lod.min_height;
    let cam_horizon = (cam_len * cam_len - occluder * occluder).max(0.0).sqrt();
    while let Some(n) = stack.pop() {
        let c = n.centre_dir();
        let r = lod.radius + ground(c);
        let centre = [c[0] * r, c[1] * r, c[2] * r];
        let d = ((cam[0] - centre[0]).powi(2)
            + (cam[1] - centre[1]).powi(2)
            + (cam[2] - centre[2]).powi(2))
        .sqrt();
        let bound = lod.bound_radius(n.level);
        let patch_horizon = ((r + bound).powi(2) - occluder * occluder).max(0.0).sqrt();
        if d - bound > cam_horizon + patch_horizon {
            continue; // entirely below the horizon: neither drawn nor generated
        }
        let dist = (d - bound).max(1.0);
        let error_px = lod.spacing(n.level) / dist * px_per_radian;
        let threshold = if split.contains(&n) {
            0.5 * lod.sse_px
        } else {
            lod.sse_px
        };
        if n.level < lod.max_level && error_px > threshold {
            new_split.insert(n);
            stack.extend(n.children());
        } else {
            leaves.push(n);
        }
    }
    *split = new_split;
    leaves.sort();
    leaves
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ancestry() {
        let root = NodeId::root(2);
        let leaf = NodeId {
            face: 2,
            level: 3,
            x: 5,
            y: 6,
        };
        assert!(root.is_ancestor_of(leaf));
        assert_eq!(
            leaf.ancestor(1),
            NodeId {
                face: 2,
                level: 1,
                x: 1,
                y: 1
            }
        );
        assert!(!leaf.is_ancestor_of(root));
        assert!(!NodeId::root(3).is_ancestor_of(leaf));
    }

    /// Not a pass/fail test: prints selected-patch counts for the E3 analysis.
    /// `cargo test -p spike_app --release -- --nocapture patch_counts`
    #[test]
    fn patch_counts() {
        let px_per_radian = 1440.0 / (2.0 * (30f64.to_radians()).tan());
        let ground = |_: Vec3| 0.0;
        for grid in [33u32, 65, 129] {
            for sse in [2.0, 4.0, 8.0, 16.0] {
                // Flat test ground: lowest terrain is the reference sphere minus the delta margin.
                let lod = Lod::new(24_000.0, grid, sse, 1.0).with_min_height(-600.0);
                let mut line = format!("grid {grid:>3} sse {sse:>4}:");
                for alt in [30.0, 1_000.0, 100_000.0] {
                    let mut split = HashSet::new();
                    let cam = [0.0, 0.0, 24_000.0 + alt];
                    let mut n = 0;
                    for _ in 0..3 {
                        n = select(&lod, cam, px_per_radian, &mut split, &ground).len();
                    }
                    let tris = n as u64 * ((grid as u64 - 1).pow(2) * 2 + 8 * (grid as u64 - 1));
                    line += &format!(
                        "  alt {alt:>6} m: {n:>5} patches {:>6.1} M tris",
                        tris as f64 / 1e6
                    );
                }
                println!("{line}");
            }
        }
    }

    #[test]
    fn band_mapping_matches_docs_10() {
        // 24 km body, 65×65 grid: docs/10 table.
        let lod = Lod::new(24_000.0, 65, 2.0, 1.0);
        assert_eq!(lod.band(0), 1);
        assert_eq!(lod.band(4), 2);
        assert_eq!(lod.band(8), 3);
        assert_eq!(lod.band(11), 4);
        assert_eq!(lod.max_level, 11);
    }
}
