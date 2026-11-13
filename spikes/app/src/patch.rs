//! Patch meshing from the spike field, with geomorph targets. Pure; runs on worker threads.
//!
//! Every vertex carries two states: its own (this level) and its parent's (one level coarser),
//! sampled at the parent's spacing and band and interpolated with the parent's own triangulation.
//! The vertex shader blends between them by screen-space error, so a patch is geometrically
//! identical to its parent when it appears and turns into itself gradually (E3: instant LOD
//! switches popped at every threshold).
//!
//! Vertex format (I-005), 32 bytes per vertex instead of 80:
//!
//! | Data | Format | Bytes |
//! |---|---|---|
//! | own position, parent position | 2 × Unorm16x4, fractions of the patch's box | 16 |
//! | own + parent normal | Snorm16x4, two octahedral encodings | 8 |
//! | own colour, parent colour | 2 × Unorm8x4, gamma 2 (sqrt) so dark tones do not band | 8 |
//!
//! The box (over both shapes, skirts included) becomes the entity transform: translation = box
//! minimum, scale = box size. The quantisation step is ~1/65535 of the box, about 1/1000 of a
//! sample spacing at every level, because the box scales with the patch.

use crate::quadtree::{Lod, NodeId};
use spike_field::cube::dir_of;
use spike_field::{Body, Depth, Sample};

pub struct PatchMesh {
    pub id: NodeId,
    /// World position of the box's minimum corner (the entity translation).
    pub origin: [f64; 3],
    /// Box size per axis in metres (the entity scale).
    pub extent: [f32; 3],
    pub positions: Vec<[u16; 4]>,
    pub morph_positions: Vec<[u16; 4]>,
    /// xy: own normal, zw: parent normal (octahedral).
    pub normals: Vec<[i16; 4]>,
    pub colors: Vec<[u8; 4]>,
    pub morph_colors: Vec<[u8; 4]>,
    pub indices: Vec<u16>,
    pub triangles: usize,
    pub build_ms: f32,
}

/// Collision source for one patch: its own shape (not morphed), encoded as in the render mesh.
pub struct CollisionData {
    pub origin: [f64; 3],
    pub extent: [f32; 3],
    pub grid: Vec<[u16; 4]>,
    pub tris: Vec<[u16; 3]>,
}

impl CollisionData {
    /// Body-local vertex positions (f64).
    pub fn vertices(&self) -> impl Iterator<Item = [f64; 3]> + '_ {
        self.grid.iter().map(|q| {
            [0, 1, 2].map(|k| self.origin[k] + q[k] as f64 / 65535.0 * self.extent[k] as f64)
        })
    }

    /// Centre and half-diagonal of the patch box, body-local.
    pub fn bounds(&self) -> ([f64; 3], f64) {
        let e = self.extent.map(|v| v as f64);
        let c = [0, 1, 2].map(|k| self.origin[k] + 0.5 * e[k]);
        (c, 0.5 * (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt())
    }
}

/// Bytes per vertex of the encoded format and of the E3 float format, for the metrics.
pub const VERTEX_BYTES: usize = 32;
pub const FLOAT_VERTEX_BYTES: usize = 80;

impl PatchMesh {
    /// GPU bytes of this patch's vertex and index data.
    pub fn bytes(&self) -> usize {
        self.positions.len() * VERTEX_BYTES + self.indices.len() * 2
    }

    /// What the same patch took in E3's float format (f32 attributes, u32 indices).
    pub fn float_format_bytes(&self) -> usize {
        self.positions.len() * FLOAT_VERTEX_BYTES + self.indices.len() * 4
    }

    /// Body-local position of an encoded vertex, in f64. The origin stays f64: since E5, render
    /// translations are f64 differences, and collision (E6) needs exact positions on large bodies.
    pub fn world(&self, q: [u16; 4]) -> [f64; 3] {
        [0, 1, 2].map(|k| self.origin[k] + q[k] as f64 / 65535.0 * self.extent[k] as f64)
    }

    /// A compact copy of what collision needs (encoded grid vertices and triangles), kept after
    /// the render mesh is uploaded. ~85 KB per 65 × 65 patch.
    pub fn collision_data(&self, intervals: usize) -> CollisionData {
        let w = intervals + 1;
        CollisionData {
            origin: self.origin,
            extent: self.extent,
            grid: self.positions[..w * w].to_vec(),
            tris: self.indices[..intervals * intervals * 6]
                .as_chunks::<3>()
                .0
                .to_vec(),
        }
    }

    /// Body-local positions of the grid vertices (own shape, skirts excluded) and the grid
    /// triangles, for collision. `intervals` is the patch grid size minus one.
    pub fn collision_grid(&self, intervals: usize) -> (Vec<[f64; 3]>, Vec<[u32; 3]>) {
        let w = intervals + 1;
        let verts = self.positions[..w * w]
            .iter()
            .map(|q| self.world(*q))
            .collect();
        let tris = self.indices[..intervals * intervals * 6]
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| [t[0] as u32, t[1] as u32, t[2] as u32])
            .collect();
        (verts, tris)
    }
}

/// Samples on a square grid of face-space points `u0 + (i + start) · du`, with world positions,
/// colours, and central-difference normals for the interior `1..w-1`.
struct Grid {
    w: usize,
    world: Vec<[f64; 3]>,
    color: Vec<[f32; 4]>,
}

impl Grid {
    #[allow(clippy::too_many_arguments)]
    fn sample(
        body: &Body,
        face: usize,
        u0: f64,
        v0: f64,
        step: f64,
        start: i64,
        w: usize,
        spacing: f64,
        depth: Depth,
    ) -> Grid {
        let r = body.radius;
        let mut world = Vec::with_capacity(w * w);
        let mut color = Vec::with_capacity(w * w);
        let mut s = Sample::default();
        let mut s_color = Sample::default();
        for j in 0..w {
            for i in 0..w {
                let u = u0 + (i as i64 + start) as f64 * step;
                let v = v0 + (j as i64 + start) as f64 * step;
                let d = dir_of(face, u, v);
                body.sample(d, spacing, depth, &mut s);
                let h = s.height as f64;
                world.push([d[0] * (r + h), d[1] * (r + h), d[2] * (r + h)]);
                // Colour always comes from classification (L2), whatever the band evaluates for
                // height; switching from a height tint to biome colour at a band boundary was
                // itself a visible pop.
                if depth >= Depth::L2 {
                    color.push(shade(body, &s));
                } else {
                    body.sample(d, spacing, Depth::L2, &mut s_color);
                    // Water from this band's height, land colour from L2 classification.
                    color.push(shade_with_height(body, &s_color, s.height));
                }
            }
        }
        Grid { w, world, color }
    }

    fn at(&self, i: usize, j: usize) -> [f64; 3] {
        self.world[j * self.w + i]
    }

    fn normal(&self, i: usize, j: usize) -> [f64; 3] {
        let (a, b) = (self.at(i + 1, j), self.at(i - 1, j));
        let (c, d) = (self.at(i, j + 1), self.at(i, j - 1));
        let tu = sub(a, b);
        let tv = sub(c, d);
        let mut n = cross(tu, tv);
        let p = self.at(i, j);
        if dot(n, p) < 0.0 {
            n = [-n[0], -n[1], -n[2]];
        }
        normalize(n)
    }
}

/// Build the mesh for `id`: an `(n+1)²` grid, its parent-level morph targets, and a skirt.
pub fn build(body: &Body, lod: &Lod, id: NodeId) -> PatchMesh {
    let t = std::time::Instant::now();
    let n = lod.intervals as usize;
    let half = n / 2;
    let (u0, u1, v0, _) = id.uv_range();
    let du = (u1 - u0) / n as f64;
    let face = id.face as usize;
    let c = id.centre_dir();
    let r = body.radius;
    let centre = [c[0] * r, c[1] * r, c[2] * r];

    // This level: indices -1..=n+1 (one ring for normals).
    let spacing = lod.spacing(id.level);
    let own = Grid::sample(
        body,
        face,
        u0,
        v0,
        du,
        -1,
        n + 3,
        spacing,
        Depth::for_band(lod.band(id.level)),
    );

    // Parent level: every other point, indices -2..=n+2 in this patch's numbering. The parent patch
    // samples exactly these points with exactly these parameters, so the values are identical.
    let parent = if id.level > 0 {
        let pl = id.level - 1;
        Some(Grid::sample(
            body,
            face,
            u0,
            v0,
            2.0 * du,
            -1,
            half + 3,
            lod.spacing(pl),
            Depth::for_band(lod.band(pl)),
        ))
    } else {
        None
    };

    let w = n + 1;
    let mut positions = Vec::with_capacity(w * w + 4 * w);
    let mut normals = Vec::with_capacity(positions.capacity());
    let mut colors = Vec::with_capacity(positions.capacity());
    let mut morph_offsets = Vec::with_capacity(positions.capacity());
    let mut morph_normals = Vec::with_capacity(positions.capacity());
    let mut morph_colors = Vec::with_capacity(positions.capacity());
    let local = |p: [f64; 3]| {
        [
            (p[0] - centre[0]) as f32,
            (p[1] - centre[1]) as f32,
            (p[2] - centre[2]) as f32,
        ]
    };

    for j in 0..=n {
        for i in 0..=n {
            let p = own.at(i + 1, j + 1);
            let nrm = own.normal(i + 1, j + 1);
            let col = own.color[(j + 1) * own.w + i + 1];
            positions.push(local(p));
            normals.push(f32x3(nrm));
            colors.push(col);
            match &parent {
                None => {
                    morph_offsets.push([0.0; 3]);
                    morph_normals.push(f32x3(nrm));
                    morph_colors.push(col);
                }
                Some(pg) => {
                    let (pp, pn, pc) = parent_value(pg, i, j);
                    morph_offsets.push(f32x3(sub(pp, p)));
                    morph_normals.push(f32x3(normalize(pn)));
                    morph_colors.push(pc);
                }
            }
        }
    }

    let mut indices = Vec::with_capacity(n * n * 6 + 4 * n * 6);
    for j in 0..n {
        for i in 0..n {
            // Quad a=(i,j) b=(i+1,j) c=(i,j+1) d=(i+1,j+1), split along b–c. `parent_value` relies
            // on this diagonal; do not change one without the other.
            let a = (j * w + i) as u32;
            let (b, cc, d) = (a + 1, a + w as u32, a + w as u32 + 1);
            indices.extend_from_slice(&[a, b, cc, b, d, cc]);
        }
    }

    // Skirt: duplicate each edge vertex, pushed down by a few sample spacings. It morphs with its
    // edge vertex.
    let drop = (spacing * 4.0).max(2.0);
    let edge: Vec<usize> = (0..w)
        .chain((0..w).map(|j| j * w + n))
        .chain((0..w).rev().map(|i| n * w + i))
        .chain((0..w).rev().map(|j| j * w))
        .collect();
    let base = positions.len() as u32;
    for &k in &edge {
        let p = positions[k];
        let wp = [
            p[0] as f64 + centre[0],
            p[1] as f64 + centre[1],
            p[2] as f64 + centre[2],
        ];
        let dn = normalize(wp);
        positions.push([
            p[0] - (dn[0] * drop) as f32,
            p[1] - (dn[1] * drop) as f32,
            p[2] - (dn[2] * drop) as f32,
        ]);
        normals.push(normals[k]);
        colors.push(colors[k]);
        morph_offsets.push(morph_offsets[k]);
        morph_normals.push(morph_normals[k]);
        morph_colors.push(morph_colors[k]);
    }
    for e in 0..edge.len() - 1 {
        let (a, b) = (edge[e] as u32, edge[e + 1] as u32);
        let (sa, sb) = (base + e as u32, base + e as u32 + 1);
        indices.extend_from_slice(&[a, sa, b, b, sa, sb]);
    }

    fix_winding(&positions, centre, &mut indices);
    let triangles = indices.len() / 3;

    // Encode. The box covers both shapes, so any morph state stays inside it (and inside the
    // culling bounds, which are the unit box in local space).
    let both = |k: usize| {
        positions
            .iter()
            .zip(&morph_offsets)
            .flat_map(move |(p, o)| [p[k] as f64, p[k] as f64 + o[k] as f64])
    };
    let lo = [0, 1, 2].map(|k| both(k).fold(f64::MAX, f64::min));
    let hi = [0, 1, 2].map(|k| both(k).fold(f64::MIN, f64::max));
    let extent = [0, 1, 2].map(|k| ((hi[k] - lo[k]) as f32).max(1e-3));
    let quant = |v: [f64; 3]| {
        let q = |k: usize| {
            ((v[k] - lo[k]) / extent[k] as f64 * 65535.0)
                .round()
                .clamp(0.0, 65535.0) as u16
        };
        [q(0), q(1), q(2), 65535]
    };
    let f64x3 = |p: [f32; 3]| p.map(|v| v as f64);
    let encoded_positions = positions.iter().map(|p| quant(f64x3(*p))).collect();
    let encoded_morph = positions
        .iter()
        .zip(&morph_offsets)
        .map(|(p, o)| quant(add(f64x3(*p), f64x3(*o))))
        .collect();
    let encoded_normals = normals
        .iter()
        .zip(&morph_normals)
        .map(|(a, b)| {
            let (ea, eb) = (oct_encode(f64x3(*a)), oct_encode(f64x3(*b)));
            [ea[0], ea[1], eb[0], eb[1]]
        })
        .collect();
    assert!(positions.len() <= 65_536, "patch too large for u16 indices");
    PatchMesh {
        id,
        origin: add(centre, lo),
        extent,
        positions: encoded_positions,
        morph_positions: encoded_morph,
        normals: encoded_normals,
        colors: colors.iter().map(|c| color_encode(*c)).collect(),
        morph_colors: morph_colors.iter().map(|c| color_encode(*c)).collect(),
        indices: indices.iter().map(|&i| i as u16).collect(),
        triangles,
        build_ms: t.elapsed().as_secs_f32() * 1000.0,
    }
}

/// Octahedral encoding of a unit vector into two snorm16 values.
fn oct_encode(n: [f64; 3]) -> [i16; 2] {
    let l1 = n[0].abs() + n[1].abs() + n[2].abs();
    let (mut x, mut y) = (n[0] / l1, n[1] / l1);
    if n[2] < 0.0 {
        let sx = if x >= 0.0 { 1.0 } else { -1.0 };
        let sy = if y >= 0.0 { 1.0 } else { -1.0 };
        (x, y) = ((1.0 - y.abs()) * sx, (1.0 - x.abs()) * sy);
    }
    [x, y].map(|v| (v.clamp(-1.0, 1.0) * 32767.0).round() as i16)
}

/// The decoder used by `terrain.wesl`, mirrored for tests.
#[cfg(test)]
fn oct_decode(e: [i16; 2]) -> [f64; 3] {
    let (x, y) = (e[0] as f64 / 32767.0, e[1] as f64 / 32767.0);
    let mut n = [x, y, 1.0 - x.abs() - y.abs()];
    let t = (-n[2]).max(0.0);
    n[0] += if n[0] >= 0.0 { -t } else { t };
    n[1] += if n[1] >= 0.0 { -t } else { t };
    normalize(n)
}

/// Linear colour to RGBA8 with gamma 2 (the shader squares it back).
fn color_encode(c: [f32; 4]) -> [u8; 4] {
    let g = |v: f32| (v.clamp(0.0, 1.0).sqrt() * 255.0).round() as u8;
    [g(c[0]), g(c[1]), g(c[2]), 255]
}

/// The parent mesh's position, normal, and colour at this patch's vertex `(i, j)`, following the
/// parent's triangulation: even/even vertices coincide with parent vertices, edge midpoints average
/// their two ends, and cell centres lie on the parent's b–c diagonal.
fn parent_value(pg: &Grid, i: usize, j: usize) -> ([f64; 3], [f64; 3], [f32; 4]) {
    // Parent grid index k corresponds to this patch's index 2(k - 1); the ring is at k = 0.
    let (k, l) = (i / 2 + 1, j / 2 + 1);
    let pick = |a: usize, b: usize| (pg.at(a, b), pg.normal(a, b), pg.color[b * pg.w + a]);
    let avg = |x: ([f64; 3], [f64; 3], [f32; 4]), y: ([f64; 3], [f64; 3], [f32; 4])| {
        (
            scale(add(x.0, y.0), 0.5),
            scale(add(x.1, y.1), 0.5),
            [
                (x.2[0] + y.2[0]) * 0.5,
                (x.2[1] + y.2[1]) * 0.5,
                (x.2[2] + y.2[2]) * 0.5,
                1.0,
            ],
        )
    };
    match (i % 2, j % 2) {
        (0, 0) => pick(k, l),
        (1, 0) => avg(pick(k, l), pick(k + 1, l)),
        (0, 1) => avg(pick(k, l), pick(k, l + 1)),
        _ => avg(pick(k + 1, l), pick(k, l + 1)), // b–c diagonal
    }
}

/// Make every triangle wind counter-clockwise seen from outside the body (cube faces have
/// different handedness in face space). Swapping two indices does not change the diagonal.
fn fix_winding(pos: &[[f32; 3]], centre: [f64; 3], idx: &mut [u32]) {
    for t in idx.as_chunks_mut::<3>().0 {
        let (a, b, c) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let out = [
            a[0] as f64 + centre[0],
            a[1] as f64 + centre[1],
            a[2] as f64 + centre[2],
        ];
        if (n[0] as f64) * out[0] + (n[1] as f64) * out[1] + (n[2] as f64) * out[2] < 0.0 {
            t.swap(1, 2);
        }
    }
}

fn shade(body: &Body, s: &Sample) -> [f32; 4] {
    shade_with_height(body, s, s.height)
}

fn shade_with_height(body: &Body, s: &Sample, height: f32) -> [f32; 4] {
    if height < body.sea_surface {
        let depth_m = (body.sea_surface - height).min(400.0) / 400.0;
        return [0.05, 0.25 - 0.15 * depth_m, 0.45 - 0.2 * depth_m, 1.0];
    }
    [s.albedo[0], s.albedo[1], s.albedo[2], 1.0]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize(a: [f64; 3]) -> [f64; 3] {
    let l = dot(a, a).sqrt().max(1e-12);
    [a[0] / l, a[1] / l, a[2] / l]
}
fn f32x3(a: [f64; 3]) -> [f32; 3] {
    [a[0] as f32, a[1] as f32, a[2] as f32]
}

#[cfg(test)]
mod tests {
    use super::*;
    use genesis_core::Seed;

    /// The geomorph contract: a fully morphed child reproduces its parent exactly at every vertex
    /// the parent has, and lies on the parent's triangles elsewhere.
    #[test]
    fn fully_morphed_child_matches_parent() {
        let body = Body::bake_with_radius(Seed(3), 24_000.0, 8, 2, 4, 4, 64);
        let lod = Lod::new(24_000.0, 9, 8.0, 1.0);
        let parent_id = NodeId {
            face: 4,
            level: 3,
            x: 5,
            y: 2,
        };
        let parent = build(&body, &lod, parent_id);
        for child_id in parent_id.children() {
            let child = build(&body, &lod, child_id);
            let n = lod.intervals as usize;
            let half = n / 2;
            let (ox, oy) = (
                (child_id.x % 2) as usize * half,
                (child_id.y % 2) as usize * half,
            );
            // Both sides are quantised in their own boxes, and the box origins are f32 on the
            // GPU: allow one quantisation step of each box plus f32 rounding at this radius.
            let step = |m: &PatchMesh| {
                let e = m.extent.map(|v| v as f64 / 65535.0);
                dot(e, e).sqrt()
            };
            let tolerance = step(&child) + step(&parent) + 0.01;
            for j in (0..=n).step_by(2) {
                for i in (0..=n).step_by(2) {
                    let ci = j * (n + 1) + i;
                    let pi = (oy + j / 2) * (n + 1) + ox + i / 2;
                    let cw = child.world(child.morph_positions[ci]);
                    let pw = parent.world(parent.positions[pi]);
                    let err = dot(sub(cw, pw), sub(cw, pw)).sqrt();
                    assert!(
                        err < tolerance,
                        "child {child_id:?} vertex ({i},{j}) off by {err} m (tolerance {tolerance} m)"
                    );
                }
            }
        }
    }

    /// 16-bit positions are fine-grained enough at every level, coarse ones included: the step is
    /// a fixed small fraction of the sample spacing because the box scales with the patch.
    #[test]
    fn position_step_is_far_below_sample_spacing_at_every_level() {
        let body = Body::bake_with_radius(Seed(3), 24_000.0, 8, 2, 4, 4, 64);
        let lod = Lod::new(24_000.0, 65, 16.0, 1.0);
        for level in [0, 1, 4, 8, lod.max_level] {
            let m = build(
                &body,
                &lod,
                NodeId {
                    face: 2,
                    level,
                    x: 0,
                    y: 0,
                },
            );
            let step = m.extent.iter().copied().fold(0.0f32, f32::max) as f64 / 65535.0;
            let ratio = step / lod.spacing(level);
            println!(
                "level {level:>2}: step {step:.5} m, spacing {:.3} m, ratio {ratio:.5}",
                lod.spacing(level)
            );
            assert!(
                ratio < 0.01,
                "level {level}: step is {ratio} of a sample spacing"
            );
        }
    }

    #[test]
    fn octahedral_normals_round_trip_within_a_hundredth_of_a_degree() {
        let mut worst = 0.0f64;
        for i in 0..2_000 {
            // Points spread over the sphere (golden-angle spiral), plus the axes.
            let z = 1.0 - 2.0 * (i as f64 + 0.5) / 2_000.0;
            let a = i as f64 * 2.399_963;
            let r = (1.0 - z * z).sqrt();
            for n in [
                [r * a.cos(), r * a.sin(), z],
                [0.0, 0.0, -1.0],
                [1.0, 0.0, 0.0],
            ] {
                let d = oct_decode(oct_encode(n));
                worst = worst.max(dot(n, d).clamp(-1.0, 1.0).acos().to_degrees());
            }
        }
        assert!(worst < 0.01, "worst normal error {worst}°");
    }
}
