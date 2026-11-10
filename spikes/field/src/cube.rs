//! Minimal cube-sphere grid: face/uv mapping and bilinear sampling (docs/10).

pub type Vec3 = [f64; 3];

pub fn normalize(v: Vec3) -> Vec3 {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / l, v[1] / l, v[2] / l]
}

/// Direction → (face, u, v) with u, v in [-1, 1].
pub fn face_uv(d: Vec3) -> (usize, f64, f64) {
    let (ax, ay, az) = (d[0].abs(), d[1].abs(), d[2].abs());
    if ax >= ay && ax >= az {
        (if d[0] > 0.0 { 0 } else { 1 }, d[1] / ax, d[2] / ax)
    } else if ay >= az {
        (if d[1] > 0.0 { 2 } else { 3 }, d[0] / ay, d[2] / ay)
    } else {
        (if d[2] > 0.0 { 4 } else { 5 }, d[0] / az, d[1] / az)
    }
}

/// (face, u, v) → unit direction.
pub fn dir_of(face: usize, u: f64, v: f64) -> Vec3 {
    let d = match face {
        0 => [1.0, u, v],
        1 => [-1.0, u, v],
        2 => [u, 1.0, v],
        3 => [u, -1.0, v],
        4 => [u, v, 1.0],
        _ => [u, v, -1.0],
    };
    normalize(d)
}

/// A scalar field on the six faces, `n × n` samples per face.
pub struct Grid {
    pub n: usize,
    pub data: Vec<f32>,
}

impl Grid {
    pub fn build(n: usize, mut f: impl FnMut(Vec3) -> f32) -> Grid {
        let mut data = Vec::with_capacity(6 * n * n);
        for face in 0..6 {
            for j in 0..n {
                for i in 0..n {
                    let u = i as f64 / (n - 1) as f64 * 2.0 - 1.0;
                    let v = j as f64 / (n - 1) as f64 * 2.0 - 1.0;
                    data.push(f(dir_of(face, u, v)));
                }
            }
        }
        Grid { n, data }
    }

    /// Bilinear sample. Clamps at face edges: fine for cost, not for seams.
    pub fn sample(&self, d: Vec3) -> f32 {
        let (face, u, v) = face_uv(d);
        let s = (self.n - 1) as f64;
        let x = ((u + 1.0) * 0.5 * s).clamp(0.0, s);
        let y = ((v + 1.0) * 0.5 * s).clamp(0.0, s);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.n - 1), (y0 + 1).min(self.n - 1));
        let (fx, fy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
        let base = face * self.n * self.n;
        let at = |i: usize, j: usize| self.data[base + j * self.n + i];
        let a = at(x0, y0) + (at(x1, y0) - at(x0, y0)) * fx;
        let b = at(x0, y1) + (at(x1, y1) - at(x0, y1)) * fx;
        a + (b - a) * fy
    }
}
