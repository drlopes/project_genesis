//! Meshing a voxel region: surface nets per chunk, a blocky baseline, and the seam with the
//! heightfield.
//!
//! Surface nets: one vertex per sign-changing cell (the average of its edge crossings), then a
//! quad around every sign-changing lattice edge.
//!
//! Ownership, so neighbouring chunks tile exactly with no gap and no triangle twice: a chunk owns
//! the lattice edges whose start node is local `0..32` along the edge's axis and `1..=32` on the
//! two other axes. Each such edge's quad joins the four cells around it, which lie in local
//! `0..=32`, so a chunk computes vertices for `33³` cells (one apron layer, duplicated with the
//! neighbour's first layer) from `34³` nodes. The apron nodes come from the neighbouring brick, or
//! the field if that brick is not resident. (The first version owned only quads whose cells were
//! all local `0..32`, which left a crack one cell wide along every chunk boundary.)

use crate::{CELLS, Voxels, cross3, dot3};
use spike_field::Body;
use std::collections::HashMap;

/// Nodes per side loaded for one chunk.
const W: i32 = CELLS + 2;
/// Cells per side with a vertex slot (own cells plus one apron layer).
const V: i32 = CELLS + 1;

#[inline]
fn ni(i: i32, j: i32, k: i32) -> usize {
    (i + W * (j + W * k)) as usize
}

#[inline]
fn ci(i: i32, j: i32, k: i32) -> usize {
    (i + V * (j + V * k)) as usize
}

#[derive(Default, Clone)]
pub struct Mesh {
    pub verts: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub tris: Vec<[u32; 3]>,
}

impl Mesh {
    pub fn append(&mut self, other: &Mesh) {
        let base = self.verts.len() as u32;
        self.verts.extend_from_slice(&other.verts);
        self.normals.extend_from_slice(&other.normals);
        self.tris.extend(
            other
                .tris
                .iter()
                .map(|t| [t[0] + base, t[1] + base, t[2] + base]),
        );
    }

    pub fn triangles(&self) -> usize {
        self.tris.len()
    }

    pub fn bytes(&self) -> usize {
        self.verts.len() * (2 * 12 + 4 * 3)
    }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    cross3(
        [a[0] as f64, a[1] as f64, a[2] as f64],
        [b[0] as f64, b[1] as f64, b[2] as f64],
    )
    .map(|v| v as f32)
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt();
    if l < 1e-12 {
        [0.0, 0.0, 1.0]
    } else {
        [a[0] / l, a[1] / l, a[2] / l]
    }
}

/// Emit the quad around a sign-changing edge. The four cell vertices are given in the order whose
/// winding faces +axis; `inside_low` says the solid is at the edge's lower end, so outward is
/// +axis. Winding comes from the voxel signs, not from vertex normals: at a sharp dig rim the
/// averaged normals can point the other way, and the face would be culled (a see-through hole).
fn push_quad(m: &mut Mesh, q: [u32; 4], inside_low: bool) {
    let [a, b, c, d] = q;
    let mut tri = |x: u32, y: u32, z: u32| {
        if x == u32::MAX || y == u32::MAX || z == u32::MAX || x == y || y == z || x == z {
            return;
        }
        m.tris.push(if inside_low { [x, y, z] } else { [x, z, y] });
    };
    tri(a, b, c);
    tri(a, c, d);
}

fn push_tri(m: &mut Mesh, a: u32, b: u32, c: u32) {
    if a == u32::MAX || b == u32::MAX || c == u32::MAX || a == b || b == c || a == c {
        return;
    }
    let (va, vb, vc) = (
        m.verts[a as usize],
        m.verts[b as usize],
        m.verts[c as usize],
    );
    let geometric = cross(sub(vb, va), sub(vc, va));
    let n = add(
        add(m.normals[a as usize], m.normals[b as usize]),
        m.normals[c as usize],
    );
    if dot(geometric, n) < 0.0 {
        m.tris.push([a, c, b]);
    } else {
        m.tris.push([a, b, c]);
    }
}

/// Load a chunk's `33³` nodes: samples `0..32` from its brick, the apron at 32 from the neighbour.
fn load(vx: &Voxels, body: &Body, c: [i32; 3]) -> Vec<f32> {
    let mut s = vec![0.0f32; (W * W * W) as usize];
    for k in 0..W {
        for j in 0..W {
            for i in 0..W {
                s[ni(i, j, k)] =
                    vx.node(body, [c[0] * CELLS + i, c[1] * CELLS + j, c[2] * CELLS + k]);
            }
        }
    }
    s
}

/// 12 cell edges as pairs of corner offsets.
const EDGES: [(i32, i32, i32, i32, i32, i32); 12] = [
    (0, 0, 0, 1, 0, 0),
    (0, 1, 0, 1, 1, 0),
    (0, 0, 1, 1, 0, 1),
    (0, 1, 1, 1, 1, 1),
    (0, 0, 0, 0, 1, 0),
    (1, 0, 0, 1, 1, 0),
    (0, 0, 1, 0, 1, 1),
    (1, 0, 1, 1, 1, 1),
    (0, 0, 0, 0, 0, 1),
    (1, 0, 0, 1, 0, 1),
    (0, 1, 0, 0, 1, 1),
    (1, 1, 0, 1, 1, 1),
];

/// Surface nets for one chunk, in the site's local coordinates.
pub fn chunk_surface_nets(vx: &Voxels, body: &Body, c: [i32; 3]) -> Mesh {
    let s = load(vx, body, c);
    let site = vx.site;
    let mut mesh = Mesh::default();
    let mut vid = vec![u32::MAX; (V * V * V) as usize];
    let corner = |di: i32, dj: i32, dk: i32| s[ni(di, dj, dk)];

    for k in 0..V {
        for j in 0..V {
            for i in 0..V {
                let mut inside = 0;
                for dk in 0..2 {
                    for dj in 0..2 {
                        for di in 0..2 {
                            if s[ni(i + di, j + dj, k + dk)] < 0.0 {
                                inside += 1;
                            }
                        }
                    }
                }
                if inside == 0 || inside == 8 {
                    continue;
                }
                // Vertex: average of the edge crossings, in cell-local [0, 1]³.
                let (mut sx, mut sy, mut sz, mut n) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
                for &(ax, ay, az, bx, by, bz) in &EDGES {
                    let a = corner(i + ax, j + ay, k + az);
                    let b = corner(i + bx, j + by, k + bz);
                    if (a < 0.0) == (b < 0.0) {
                        continue;
                    }
                    let t = (a / (a - b)) as f64;
                    sx += ax as f64 + t * (bx - ax) as f64;
                    sy += ay as f64 + t * (by - ay) as f64;
                    sz += az as f64 + t * (bz - az) as f64;
                    n += 1.0;
                }
                let p = if n > 0.0 {
                    [sx / n, sy / n, sz / n]
                } else {
                    [0.5, 0.5, 0.5]
                };
                let x = site.lx(c[0] * CELLS + i) + p[0] * site.voxel;
                let y = site.ly(c[1] * CELLS + j) + p[1] * site.voxel;
                let z = site.lz(c[2] * CELLS + k) + p[2] * site.voxel;
                // Normal from the 8 corner differences (SDF grows outward).
                let avg = |q: [(i32, i32, i32); 4]| {
                    q.iter()
                        .map(|&(a, b, cc)| corner(i + a, j + b, k + cc))
                        .sum::<f32>()
                        / 4.0
                };
                let gx = avg([(1, 0, 0), (1, 1, 0), (1, 0, 1), (1, 1, 1)])
                    - avg([(0, 0, 0), (0, 1, 0), (0, 0, 1), (0, 1, 1)]);
                let gy = avg([(0, 1, 0), (1, 1, 0), (0, 1, 1), (1, 1, 1)])
                    - avg([(0, 0, 0), (1, 0, 0), (0, 0, 1), (1, 0, 1)]);
                let gz = avg([(0, 0, 1), (1, 0, 1), (0, 1, 1), (1, 1, 1)])
                    - avg([(0, 0, 0), (1, 0, 0), (0, 1, 0), (1, 1, 0)]);
                vid[ci(i, j, k)] = mesh.verts.len() as u32;
                mesh.verts.push([x as f32, y as f32, z as f32]);
                mesh.normals.push(unit([gx, gy, gz]));
            }
        }
    }

    let v = |i: i32, j: i32, k: i32| vid[ci(i, j, k)];
    let node = |i: i32, j: i32, k: i32| s[ni(i, j, k)];

    // +x edges: cells (i, j−1..j, k−1..k).
    for k in 1..=CELLS {
        for j in 1..=CELLS {
            for i in 0..CELLS {
                if (node(i, j, k) < 0.0) == (node(i + 1, j, k) < 0.0) {
                    continue;
                }
                let (a, b, c, d) = (
                    v(i, j - 1, k - 1),
                    v(i, j, k - 1),
                    v(i, j, k),
                    v(i, j - 1, k),
                );
                push_quad(&mut mesh, [a, b, c, d], node(i, j, k) < 0.0);
            }
        }
    }
    // +y edges: cells (i−1..i, j, k−1..k).
    for k in 1..=CELLS {
        for j in 0..CELLS {
            for i in 1..=CELLS {
                if (node(i, j, k) < 0.0) == (node(i, j + 1, k) < 0.0) {
                    continue;
                }
                let (a, b, c, d) = (
                    v(i - 1, j, k - 1),
                    v(i - 1, j, k),
                    v(i, j, k),
                    v(i, j, k - 1),
                );
                push_quad(&mut mesh, [a, b, c, d], node(i, j, k) < 0.0);
            }
        }
    }
    // +z edges: cells (i−1..i, j−1..j, k).
    for k in 0..CELLS {
        for j in 1..=CELLS {
            for i in 1..=CELLS {
                if (node(i, j, k) < 0.0) == (node(i, j, k + 1) < 0.0) {
                    continue;
                }
                let (a, b, c, d) = (
                    v(i - 1, j - 1, k),
                    v(i, j - 1, k),
                    v(i, j, k),
                    v(i - 1, j, k),
                );
                push_quad(&mut mesh, [a, b, c, d], node(i, j, k) < 0.0);
            }
        }
    }
    mesh
}

/// Surface nets for every resident brick.
pub fn surface_nets(vx: &Voxels, body: &Body) -> Mesh {
    let mut mesh = Mesh::default();
    for c in sorted_chunks(vx) {
        mesh.append(&chunk_surface_nets(vx, body, c));
    }
    mesh
}

fn sorted_chunks(vx: &Voxels) -> Vec<[i32; 3]> {
    let mut keys: Vec<[i32; 3]> = vx.bricks.keys().copied().collect();
    keys.sort_unstable();
    keys
}

/// Blocky baseline: one cube face per exposed voxel face. Each chunk emits only its +x/+y/+z
/// boundary faces and its −x/−y/−z faces away from the chunk edge, so no face is emitted twice.
pub fn blocky(vx: &Voxels, body: &Body) -> Mesh {
    let site = vx.site;
    let v = site.voxel as f32;
    let mut mesh = Mesh::default();
    for c in sorted_chunks(vx) {
        let s = load(vx, body, c);
        let node = |i: i32, j: i32, k: i32| s[ni(i, j, k)];
        for k in 0..CELLS {
            for j in 0..CELLS {
                for i in 0..CELLS {
                    let mut sum = 0.0;
                    for dk in 0..2 {
                        for dj in 0..2 {
                            for di in 0..2 {
                                sum += node(i + di, j + dj, k + dk);
                            }
                        }
                    }
                    if sum >= 0.0 {
                        continue;
                    }
                    let base = [
                        site.lx(c[0] * CELLS + i) as f32,
                        site.ly(c[1] * CELLS + j) as f32,
                        site.lz(c[2] * CELLS + k) as f32,
                    ];
                    // (axis, side, face node average, 4 corner offsets)
                    for axis in 0..3 {
                        for side in [-1i32, 1] {
                            if side < 0 && [i, j, k][axis] == 0 {
                                continue; // the previous chunk owns this plane
                            }
                            // The +1 is only on the face's own axis; the other two keep the cell.
                            let fi = i + if side > 0 && axis == 0 { 1 } else { 0 };
                            let fj = j + if side > 0 && axis == 1 { 1 } else { 0 };
                            let fk = k + if side > 0 && axis == 2 { 1 } else { 0 };
                            let mut fsum = 0.0;
                            for dk in 0..2 {
                                for dj in 0..2 {
                                    for di in 0..2 {
                                        fsum += node(
                                            fi + if axis == 0 { 0 } else { di },
                                            fj + if axis == 1 { 0 } else { dj },
                                            fk + if axis == 2 { 0 } else { dk },
                                        );
                                    }
                                }
                            }
                            if fsum <= 0.0 {
                                continue;
                            }
                            let start = mesh.verts.len() as u32;
                            let normal = match axis {
                                0 => [side as f32, 0.0, 0.0],
                                1 => [0.0, side as f32, 0.0],
                                _ => [0.0, 0.0, side as f32],
                            };
                            // The face's two free axes.
                            let (u, w) = match axis {
                                0 => (1usize, 2usize),
                                1 => (0usize, 2usize),
                                _ => (0usize, 1usize),
                            };
                            let mut pts = [[0.0f32; 3]; 4];
                            for (n, (du, dw)) in [(0, 0), (1, 0), (1, 1), (0, 1)].iter().enumerate()
                            {
                                let mut p = base;
                                p[axis] += if side > 0 { v } else { 0.0 };
                                p[u] += *du as f32 * v;
                                p[w] += *dw as f32 * v;
                                pts[n] = p;
                            }
                            for p in pts {
                                mesh.verts.push(p);
                                mesh.normals.push(normal);
                            }
                            push_tri(&mut mesh, start, start + 1, start + 2);
                            push_tri(&mut mesh, start, start + 2, start + 3);
                        }
                    }
                }
            }
        }
    }
    mesh
}

#[derive(Default, Debug)]
pub struct Seam {
    pub points: usize,
    pub hits: usize,
    /// Rays at least one voxel inside the box that hit no triangle: holes in the mesh. Must be 0.
    pub holes: usize,
    /// Local (x, y) range of the holes, for diagnosis: [xmin, xmax, ymin, ymax].
    pub hole_box: [f64; 4],
    pub edge_points: usize,
    pub step_p50: f64,
    pub step_p99: f64,
    pub step_max: f64,
    pub edge_p50: f64,
    pub edge_p99: f64,
    pub edge_max: f64,
    pub err_p50: f64,
    pub err_p99: f64,
    /// Largest absolute distance from a voxel surface point to the analytic solid, and where.
    pub err_max: f64,
    pub worst_err: [f64; 3],
    /// Local (x, y) and value of the largest absolute *vertical* step, with the three heights
    /// there (`vox`, bilinear heightfield, analytic) and how many triangles the ray tested.
    /// At a near-vertical cliff this is the wall height, which is not a meshing error.
    pub worst: [f64; 3],
    pub worst_z: [f64; 3],
    pub worst_tris: usize,
}

/// Highest point of the mesh above local `(x, y)`, from the triangle buckets.
fn cast(
    buckets: &HashMap<(i32, i32), Vec<u32>>,
    mesh: &Mesh,
    cell: f64,
    x: f64,
    y: f64,
) -> Option<f64> {
    let list = buckets.get(&((x / cell).floor() as i32, (y / cell).floor() as i32))?;
    let mut top: Option<f64> = None;
    for &t in list {
        let tri = mesh.tris[t as usize];
        let a = mesh.verts[tri[0] as usize];
        let b = mesh.verts[tri[1] as usize];
        let c = mesh.verts[tri[2] as usize];
        let denom = (b[1] - c[1]) as f64 * (a[0] - c[0]) as f64
            + (c[0] - b[0]) as f64 * (a[1] - c[1]) as f64;
        if denom.abs() < 1e-9 {
            continue;
        }
        let l0 = ((b[1] - c[1]) as f64 * (x - c[0] as f64)
            + (c[0] - b[0]) as f64 * (y - c[1] as f64))
            / denom;
        let l1 = ((c[1] - a[1]) as f64 * (x - c[0] as f64)
            + (a[0] - c[0]) as f64 * (y - c[1] as f64))
            / denom;
        let l2 = 1.0 - l0 - l1;
        if l0 < -1e-6 || l1 < -1e-6 || l2 < -1e-6 {
            continue;
        }
        let z = l0 * a[2] as f64 + l1 * b[2] as f64 + l2 * c[2] as f64;
        top = Some(top.map_or(z, |t: f64| t.max(z)));
    }
    top
}

impl Seam {
    fn note_hole(&mut self, x: f64, y: f64) {
        if self.holes == 1 {
            self.hole_box = [x, x, y, y];
        }
        let b = &mut self.hole_box;
        *b = [b[0].min(x), b[1].max(x), b[2].min(y), b[3].max(y)];
    }
}

fn pct(v: &[f64], q: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v[((v.len() - 1) as f64 * q) as usize]
}

/// Compare the voxel mesh's surface with a heightfield mesh (bilinear samples at `hf_spacing`)
/// over the same box. The seam is the difference between the two approximations, measured both
/// across the whole box and in a band along its edges, where the two meshes meet.
pub fn seam(vx: &Voxels, body: &Body, mesh: &Mesh, spacing: f64, hf_spacing: f64) -> Seam {
    let site = vx.site;
    let half = site.half();
    // Bucket triangles by (x, y) cell for the vertical rays.
    let cell = (spacing * 2.0).max(1.0);
    let mut buckets: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
    let mut proj = Vec::with_capacity(mesh.tris.len() * 6);
    for (t, tri) in mesh.tris.iter().enumerate() {
        let a = mesh.verts[tri[0] as usize];
        let b = mesh.verts[tri[1] as usize];
        let c = mesh.verts[tri[2] as usize];
        let xs = [a[0], b[0], c[0]];
        let ys = [a[1], b[1], c[1]];
        let (x0, x1) = (
            xs.iter().copied().fold(f32::MAX, f32::min),
            xs.iter().copied().fold(f32::MIN, f32::max),
        );
        let (y0, y1) = (
            ys.iter().copied().fold(f32::MAX, f32::min),
            ys.iter().copied().fold(f32::MIN, f32::max),
        );
        proj.push([x0, x1, y0, y1]);
        for cy in ((y0 as f64 / cell).floor() as i32)..=((y1 as f64 / cell).floor() as i32) {
            for cx in ((x0 as f64 / cell).floor() as i32)..=((x1 as f64 / cell).floor() as i32) {
                buckets.entry((cx, cy)).or_default().push(t as u32);
            }
        }
    }

    // Heightfield samples on a regular grid, bilinear between them.
    let n = ((2.0 * half) / hf_spacing).ceil() as i32 + 3;
    let hx0 = -half - 1.5 * hf_spacing;
    let mut hf = vec![0.0f32; (n * n) as usize];
    for j in 0..n {
        for i in 0..n {
            let x = hx0 + i as f64 * hf_spacing;
            let y = hx0 + j as f64 * hf_spacing;
            hf[(i + n * j) as usize] = site.ground_z(body, x, y) as f32;
        }
    }
    let hf_at = |x: f64, y: f64| -> f64 {
        let fx = (x - hx0) / hf_spacing;
        let fy = (y - hx0) / hf_spacing;
        let (i, j) = (fx.floor() as i32, fy.floor() as i32);
        if i < 0 || j < 0 || i + 1 >= n || j + 1 >= n {
            return f64::NAN;
        }
        let (tx, ty) = (fx - i as f64, fy - j as f64);
        let h = |i: i32, j: i32| hf[(i + n * j) as usize] as f64;
        (h(i, j) * (1.0 - tx) + h(i + 1, j) * tx) * (1.0 - ty)
            + (h(i, j + 1) * (1.0 - tx) + h(i + 1, j + 1) * tx) * ty
    };

    let mut all = Vec::new();
    let mut edge = Vec::new();
    let mut errs = Vec::new();
    let steps = ((2.0 * half) / spacing).ceil() as i32;
    let mut out = Seam::default();
    for sj in 0..=steps {
        let y = -half + sj as f64 * spacing;
        for si in 0..=steps {
            let x = -half + si as f64 * spacing;
            out.points += 1;
            // The outermost voxel ring may be uncovered (its quads need cells outside the box);
            // anywhere further in, a ray that hits nothing has found a hole.
            let interior = x.abs() < half - site.voxel && y.abs() < half - site.voxel;
            // A ray exactly through a shared edge or vertex can slip between two triangles in
            // f32; one retry 1 mm off separates that from a real hole.
            let top = match cast(&buckets, mesh, cell, x, y)
                .or_else(|| cast(&buckets, mesh, cell, x + 1.0e-3, y + 0.7e-3))
            {
                Some(z) => z,
                None => {
                    if interior {
                        out.holes += 1;
                        out.note_hole(x, y);
                    }
                    continue;
                }
            };
            let z_vox = top;
            let z_hf = hf_at(x, y);
            if !z_hf.is_finite() {
                continue;
            }
            out.hits += 1;
            let d = z_vox - z_hf;
            if d.abs() > out.worst[2].abs() {
                out.worst = [x, y, d];
                out.worst_z = [z_vox, z_hf, site.ground_z(body, x, y)];
                out.worst_tris = buckets
                    .get(&((x / cell).floor() as i32, (y / cell).floor() as i32))
                    .map_or(0, Vec::len);
            }
            // Distance from the meshed point to the analytic solid: the metric that stays
            // meaningful on a near-vertical wall, where a height step is not an error.
            let err = site.sdf(body, x, y, z_vox) as f64;
            if err.abs() > out.err_max {
                out.err_max = err.abs();
                out.worst_err = [x, y, err];
            }
            all.push(d);
            errs.push(err.abs());
            if x.abs() > half - 2.0 * spacing || y.abs() > half - 2.0 * spacing {
                edge.push(d);
                out.edge_points += 1;
            }
        }
    }
    all.sort_by(f64::total_cmp);
    edge.sort_by(f64::total_cmp);
    errs.sort_by(f64::total_cmp);
    out.step_p50 = pct(&all, 0.50);
    out.step_p99 = pct(&all, 0.99);
    out.step_max = all.last().copied().unwrap_or(f64::NAN);
    out.edge_p50 = pct(&edge, 0.50);
    out.edge_p99 = pct(&edge, 0.99);
    out.edge_max = edge.last().copied().unwrap_or(f64::NAN);
    out.err_p50 = pct(&errs, 0.50);
    out.err_p99 = pct(&errs, 0.99);
    let _ = dot3([0.0; 3], [0.0; 3]);
    out
}
