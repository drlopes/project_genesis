//! Cube-sphere grids: the coarse relief grid (cell centres) and the macro pyramid (nodes).
//!
//! Face layout (shared with the spikes): face `2a` is the `+axis a` face and `2a + 1` the `−axis`
//! face, for axes x, y, z. On faces 0–1 `(u, v) = (y, z) / |x|`, on 2–3 `(x, z) / |y|`, on 4–5
//! `(x, y) / |z|`; `u, v ∈ [−1, 1]` (gnomonic). See docs/10-lod-and-streaming.md.

/// A direction or position in body-local coordinates (docs/16: +Y north, longitude 0 at +X).
pub type Vec3 = [f64; 3];

pub fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn length(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}

pub fn normalize(a: Vec3) -> Vec3 {
    let l = length(a);
    [a[0] / l, a[1] / l, a[2] / l]
}

pub fn scale(a: Vec3, s: f64) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Angle between two unit vectors, accurate at small angles (`atan2(|a × b|, a · b)`).
pub fn angle(a: Vec3, b: Vec3) -> f64 {
    genesis_core::math::f64::atan2(length(cross(a, b)), dot(a, b))
}

/// Direction of `(face, u, v)`.
pub fn dir_of(face: usize, u: f64, v: f64) -> Vec3 {
    let s = if face.is_multiple_of(2) { 1.0 } else { -1.0 };
    let p = match face / 2 {
        0 => [s, u, v],
        1 => [u, s, v],
        _ => [u, v, s],
    };
    normalize(p)
}

/// `(face, u, v)` of a direction.
pub fn face_uv(d: Vec3) -> (usize, f64, f64) {
    let a = [d[0].abs(), d[1].abs(), d[2].abs()];
    if a[0] >= a[1] && a[0] >= a[2] {
        (usize::from(d[0] < 0.0), d[1] / a[0], d[2] / a[0])
    } else if a[1] >= a[2] {
        (2 + usize::from(d[1] < 0.0), d[0] / a[1], d[2] / a[1])
    } else {
        (4 + usize::from(d[2] < 0.0), d[0] / a[2], d[1] / a[2])
    }
}

/// Cell `(i, j)` of an `n × n` cell grid on `face`: centre direction and solid angle (steradians).
/// The solid angle of a gnomonic cell is `∫∫ (1 + u² + v²)^(−3/2) du dv`; the midpoint rule is
/// enough for area weighting (the six faces sum to 4π within 1e-4 at n ≥ 32).
pub fn cell(face: usize, n: usize, i: usize, j: usize) -> (Vec3, f64) {
    let h = 2.0 / n as f64;
    let u = -1.0 + (i as f64 + 0.5) * h;
    let v = -1.0 + (j as f64 + 0.5) * h;
    let r2 = 1.0 + u * u + v * v;
    (dir_of(face, u, v), h * h / (r2 * r2.sqrt()))
}

/// A cube-sphere grid of **nodes**, corner-aligned: node `(i, j)` of a face is at
/// `u = 2i / (n − 1) − 1`. Edge and corner nodes exist on every face that shares them and hold the
/// same value (same direction), so bilinear interpolation is continuous across face edges.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub n: usize,
    /// `face · n² + j · n + i`.
    pub data: Vec<f32>,
}

impl Grid {
    pub fn build(n: usize, mut f: impl FnMut(Vec3) -> f32) -> Grid {
        let mut data = Vec::with_capacity(6 * n * n);
        let step = 2.0 / (n - 1) as f64;
        for face in 0..6 {
            for j in 0..n {
                for i in 0..n {
                    data.push(f(dir_of(
                        face,
                        -1.0 + i as f64 * step,
                        -1.0 + j as f64 * step,
                    )));
                }
            }
        }
        Grid { n, data }
    }

    /// Bilinear sample at a direction.
    pub fn sample(&self, d: Vec3) -> f32 {
        let (face, u, v) = face_uv(d);
        let n = self.n;
        let m = (n - 1) as f64;
        let x = ((u + 1.0) * 0.5 * m).clamp(0.0, m);
        let y = ((v + 1.0) * 0.5 * m).clamp(0.0, m);
        let (i, j) = ((x as usize).min(n - 2), (y as usize).min(n - 2));
        let (tx, ty) = ((x - i as f64) as f32, (y - j as f64) as f32);
        let at = |i: usize, j: usize| self.data[face * n * n + j * n + i];
        let a = at(i, j) + (at(i + 1, j) - at(i, j)) * tx;
        let b = at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * tx;
        a + (b - a) * ty
    }

    /// Node spacing on a body of radius `r` at the face centre, metres (the finest spacing; edge
    /// nodes are closer together).
    pub fn spacing(&self, r: f64) -> f64 {
        2.0 / (self.n - 1) as f64 * r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_uv_round_trips() {
        for face in 0..6 {
            for &(u, v) in &[(0.0, 0.0), (0.3, -0.7), (-1.0, 1.0), (0.99, 0.2)] {
                let (f, u2, v2) = face_uv(dir_of(face, u, v));
                if u.abs() < 1.0 && v.abs() < 1.0 {
                    assert_eq!(f, face);
                    assert!((u - u2).abs() < 1e-12 && (v - v2).abs() < 1e-12);
                }
            }
        }
    }

    #[test]
    fn cells_cover_the_sphere() {
        let n = 32;
        let mut total = 0.0;
        for face in 0..6 {
            for j in 0..n {
                for i in 0..n {
                    total += cell(face, n, i, j).1;
                }
            }
        }
        let four_pi = 4.0 * std::f64::consts::PI;
        assert!((total - four_pi).abs() / four_pi < 1e-3, "total {total}");
    }

    #[test]
    fn bilinear_is_continuous_across_face_edges() {
        use genesis_core::math::f64::{cos, sin};
        let g = Grid::build(17, |d| (3.0 * d[0] + 5.0 * d[1] - 2.0 * d[2]) as f32);
        // Walk across every face edge in small steps; the sample must not jump.
        let ring = |t: f64| normalize([cos(t), 0.3 * sin(t), sin(t)]);
        for k in 0..2000 {
            let t = k as f64 / 2000.0 * std::f64::consts::TAU;
            assert!((g.sample(ring(t)) - g.sample(ring(t + 1e-4))).abs() < 0.01);
        }
    }
}
