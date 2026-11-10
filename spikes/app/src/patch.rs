//! Patch meshing from the spike field, with geomorph targets. Pure; runs on worker threads.
//!
//! Every vertex carries two states: its own (this level) and its parent's (one level coarser),
//! sampled at the parent's spacing and band and interpolated with the parent's own triangulation.
//! The vertex shader blends between them by screen-space error, so a patch is geometrically
//! identical to its parent when it appears and turns into itself gradually (E3: instant LOD
//! switches popped at every threshold).

use crate::quadtree::{Lod, NodeId};
use spike_field::cube::dir_of;
use spike_field::{Body, Depth, Sample};

pub struct PatchMesh {
    pub id: NodeId,
    pub centre: [f64; 3],
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    /// Parent-level position minus this level's position (local, metres).
    pub morph_offsets: Vec<[f32; 3]>,
    pub morph_normals: Vec<[f32; 3]>,
    pub morph_colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub triangles: usize,
    pub build_ms: f32,
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
    PatchMesh {
        id,
        centre,
        positions,
        normals,
        colors,
        morph_offsets,
        morph_normals,
        morph_colors,
        indices,
        triangles,
        build_ms: t.elapsed().as_secs_f32() * 1000.0,
    }
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
            for j in (0..=n).step_by(2) {
                for i in (0..=n).step_by(2) {
                    let ci = j * (n + 1) + i;
                    let pi = (oy + j / 2) * (n + 1) + ox + i / 2;
                    let cw = [
                        child.positions[ci][0] as f64
                            + child.morph_offsets[ci][0] as f64
                            + child.centre[0],
                        child.positions[ci][1] as f64
                            + child.morph_offsets[ci][1] as f64
                            + child.centre[1],
                        child.positions[ci][2] as f64
                            + child.morph_offsets[ci][2] as f64
                            + child.centre[2],
                    ];
                    let pw = [
                        parent.positions[pi][0] as f64 + parent.centre[0],
                        parent.positions[pi][1] as f64 + parent.centre[1],
                        parent.positions[pi][2] as f64 + parent.centre[2],
                    ];
                    let err = dot(sub(cw, pw), sub(cw, pw)).sqrt();
                    assert!(
                        err < 0.02,
                        "child {child_id:?} vertex ({i},{j}) off by {err} m"
                    );
                }
            }
        }
    }
}
