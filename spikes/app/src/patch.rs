//! Patch meshing from the spike field. Pure; runs on worker threads.

use crate::quadtree::{Lod, NodeId};
use spike_field::cube::dir_of;
use spike_field::{Body, Depth, Sample};

pub struct PatchMesh {
    pub id: NodeId,
    pub centre: [f64; 3],
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub triangles: usize,
    pub build_ms: f32,
}

/// Build the mesh for `id`: a `(n+1)²` grid plus one extra ring of samples for seam-consistent
/// normals, and a skirt around the edge to hide cracks between LOD levels.
pub fn build(body: &Body, lod: &Lod, id: NodeId) -> PatchMesh {
    let t = std::time::Instant::now();
    let n = lod.intervals as usize;
    let g = n + 3; // with the normal ring
    let (u0, u1, v0, v1) = id.uv_range();
    let du = (u1 - u0) / n as f64;
    let dv = (v1 - v0) / n as f64;
    let spacing = lod.spacing(id.level);
    let band = lod.band(id.level);
    let depth = Depth::for_band(band);
    let c = id.centre_dir();
    let r = body.radius;
    let centre = [c[0] * r, c[1] * r, c[2] * r];

    let mut world = vec![[0.0f64; 3]; g * g];
    let mut color = vec![[0.0f32; 4]; g * g];
    let mut s = Sample::default();
    for j in 0..g {
        for i in 0..g {
            let u = u0 + (i as f64 - 1.0) * du;
            let v = v0 + (j as f64 - 1.0) * dv;
            let d = dir_of(id.face as usize, u, v);
            body.sample(d, spacing, depth, &mut s);
            let h = s.height as f64;
            world[j * g + i] = [d[0] * (r + h), d[1] * (r + h), d[2] * (r + h)];
            color[j * g + i] = shade(body, &s, depth);
        }
    }

    let mut positions = Vec::with_capacity((n + 1) * (n + 1) + 4 * (n + 1));
    let mut normals = Vec::with_capacity(positions.capacity());
    let mut colors = Vec::with_capacity(positions.capacity());
    let local = |p: [f64; 3]| {
        [
            (p[0] - centre[0]) as f32,
            (p[1] - centre[1]) as f32,
            (p[2] - centre[2]) as f32,
        ]
    };
    for j in 1..=n + 1 {
        for i in 1..=n + 1 {
            let p = world[j * g + i];
            let a = world[j * g + i + 1];
            let b = world[j * g + i - 1];
            let cc = world[(j + 1) * g + i];
            let dd = world[(j - 1) * g + i];
            let tu = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            let tv = [cc[0] - dd[0], cc[1] - dd[1], cc[2] - dd[2]];
            let mut nrm = [
                tu[1] * tv[2] - tu[2] * tv[1],
                tu[2] * tv[0] - tu[0] * tv[2],
                tu[0] * tv[1] - tu[1] * tv[0],
            ];
            // Face orientation varies; make the normal point outward.
            if nrm[0] * p[0] + nrm[1] * p[1] + nrm[2] * p[2] < 0.0 {
                nrm = [-nrm[0], -nrm[1], -nrm[2]];
            }
            let l = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2])
                .sqrt()
                .max(1e-12);
            positions.push(local(p));
            normals.push([
                (nrm[0] / l) as f32,
                (nrm[1] / l) as f32,
                (nrm[2] / l) as f32,
            ]);
            colors.push(color[j * g + i]);
        }
    }

    let w = n + 1;
    let mut indices = Vec::with_capacity(n * n * 6 + 4 * n * 6);
    let quad = |a: u32, b: u32, c: u32, d: u32, out: &mut Vec<u32>| {
        // Two triangles; winding is fixed up below by checking against the outward normal.
        out.extend_from_slice(&[a, b, c, b, d, c]);
    };
    for j in 0..n {
        for i in 0..n {
            let a = (j * w + i) as u32;
            quad(a, a + 1, a + w as u32, a + w as u32 + 1, &mut indices);
        }
    }

    // Skirt: duplicate each edge vertex, pushed down by a few sample spacings.
    let drop = (spacing * 4.0).max(2.0);
    let edge: Vec<usize> = (0..w)
        .map(|i| i)
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
        let len = (wp[0] * wp[0] + wp[1] * wp[1] + wp[2] * wp[2]).sqrt();
        let down = [wp[0] / len * drop, wp[1] / len * drop, wp[2] / len * drop];
        positions.push([
            p[0] - down[0] as f32,
            p[1] - down[1] as f32,
            p[2] - down[2] as f32,
        ]);
        normals.push(normals[k]);
        colors.push(colors[k]);
    }
    for e in 0..edge.len() - 1 {
        let (a, b) = (edge[e] as u32, edge[e + 1] as u32);
        let (sa, sb) = (base + e as u32, base + e as u32 + 1);
        indices.extend_from_slice(&[a, sa, b, b, sa, sb]);
    }

    fix_winding(&positions, &normals, centre, &mut indices);
    let triangles = indices.len() / 3;
    PatchMesh {
        id,
        centre,
        positions,
        normals,
        colors,
        indices,
        triangles,
        build_ms: t.elapsed().as_secs_f32() * 1000.0,
    }
}

/// Make every triangle wind counter-clockwise seen from outside the body (cube faces have
/// different handedness in face space).
fn fix_winding(pos: &[[f32; 3]], _nrm: &[[f32; 3]], centre: [f64; 3], idx: &mut [u32]) {
    for t in idx.chunks_exact_mut(3) {
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

fn shade(body: &Body, s: &Sample, depth: Depth) -> [f32; 4] {
    if s.height < body.sea_surface {
        let depth_m = (body.sea_surface - s.height).min(400.0) / 400.0;
        return [0.05, 0.25 - 0.15 * depth_m, 0.45 - 0.2 * depth_m, 1.0];
    }
    if depth >= Depth::L2 {
        return [s.albedo[0], s.albedo[1], s.albedo[2], 1.0];
    }
    // Coarse bands: height-based tint, so far LODs still read as terrain.
    let t = ((s.height + 1500.0) / 3000.0).clamp(0.0, 1.0);
    [0.30 + 0.35 * t, 0.32 + 0.25 * t, 0.25 + 0.2 * t, 1.0]
}
