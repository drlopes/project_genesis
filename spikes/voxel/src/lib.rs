//! E7: an editable voxel region near the player, and the seam where it meets the heightfield.
//!
//! The region is a box in a local Cartesian frame at one site on the body: `x` east, `y` north,
//! `z` up, all in metres. Inside it, the solid is sampled as a signed distance on a voxel grid:
//!
//! ```text
//! g(p) = |p| − (body.radius + height(dir(p)))     // negative inside
//! ```
//!
//! Storage is **chunks of 32³ voxels**, allocated only where the terrain passes through the
//! chunk's column (plus a digging margin). Samples are global: a chunk owns voxels
//! `[32c, 32c + 32)` and the neighbouring chunk continues at `32(c+1)`, so no sample is stored
//! twice and an edit touches exactly one copy.
//!
//! Chunk meshing is separate ([`mesh`]): an edit dirties only the chunks it touches.

pub mod mesh;

use genesis_core::Seed;
use spike_field::{Body, Depth, Sample};
use std::collections::HashMap;

/// Voxels per chunk side.
pub const CELLS: i32 = 32;
/// How far below the lowest terrain in a column chunks are allocated, metres.
pub const DEPTH: f64 = 8.0;
/// How far above it, metres.
pub const ABOVE: f64 = 2.0;

/// A voxel region's placement, in body-local metres relative to the site centre.
#[derive(Clone, Copy, Debug)]
pub struct Site {
    /// Surface point the local frame is centred on (body-local, metres).
    pub center: [f64; 3],
    pub east: [f64; 3],
    pub north: [f64; 3],
    pub up: [f64; 3],
    pub voxel: f64,
    /// Local x of sample index 0 (also chunk 0's minimum corner).
    pub x0: f64,
    /// Local z of sample index 0.
    pub z0: f64,
    pub ncx: i32,
    pub ncy: i32,
    pub ncz: i32,
}

impl Site {
    /// How many chunks fit in `radius` metres (at least one, so the box is never empty).
    pub fn chunks_for(radius: f64, voxel: f64) -> i32 {
        let chunk = CELLS as f64 * voxel;
        (radius / chunk).ceil().max(1.0) as i32
    }

    /// Region half-extent actually used, metres (a whole number of chunks).
    pub fn half(&self) -> f64 {
        self.ncx as f64 * CELLS as f64 * self.voxel * 0.5
    }

    /// Local x coordinate of global sample `i`.
    pub fn lx(&self, i: i32) -> f64 {
        self.x0 + i as f64 * self.voxel
    }

    /// Local y coordinate of global sample `j` (the box is symmetric, so this is `lx`).
    pub fn ly(&self, j: i32) -> f64 {
        self.x0 + j as f64 * self.voxel
    }

    /// Local z coordinate of global sample `k`.
    pub fn lz(&self, k: i32) -> f64 {
        self.z0 + k as f64 * self.voxel
    }

    /// Signed distance at a local point: negative inside the solid.
    pub fn sdf(&self, body: &Body, x: f64, y: f64, z: f64) -> f32 {
        let p = add3(
            self.center,
            add3(
                add3(scale3(self.east, x), scale3(self.north, y)),
                scale3(self.up, z),
            ),
        );
        let r = len3(p);
        let d = scale3(p, 1.0 / r);
        (r - surface_radius(body, d, self.voxel)) as f32
    }

    /// Local z of the surface above local `(x, y)`, along the radial direction through it.
    pub fn ground_z(&self, body: &Body, x: f64, y: f64) -> f64 {
        let p = add3(
            self.center,
            add3(scale3(self.east, x), scale3(self.north, y)),
        );
        let r = len3(p);
        let d = scale3(p, 1.0 / r);
        let q = scale3(d, surface_radius(body, d, self.voxel));
        dot3(sub3(q, self.center), self.up)
    }

    /// Chunk coordinate containing a local point.
    pub fn chunk_of(&self, x: f64, y: f64, z: f64) -> [i32; 3] {
        let chunk = CELLS as f64 * self.voxel;
        [
            ((x - self.x0) / chunk).floor() as i32,
            ((y - self.x0) / chunk).floor() as i32,
            ((z - self.z0) / chunk).floor() as i32,
        ]
    }

    /// Pick a land site: above the sea, below the high peaks, not steep.
    pub fn pick(body: &Body, voxel: f64, radius: f64) -> Site {
        let sea = body.sea_surface as f64;
        let mut best: Option<(f64, [f64; 3])> = None;
        let mut fallback: (f64, [f64; 3]) = (f64::NEG_INFINITY, [0.0, 1.0, 0.0]);
        for d in spike_field::directions(Seed(7), 1024) {
            let d = [d[0] as f64, d[1] as f64, d[2] as f64];
            let h = body.h0(d) as f64;
            if h > fallback.0 {
                fallback = (h, d);
            }
            if h <= sea + 20.0 {
                continue;
            }
            let (east, north) = frame(d);
            let step = 20.0 / body.radius;
            let shifted = |t: [f64; 3]| normalize3(add3(d, scale3(t, step)));
            let he = body.h0(shifted(east)) as f64;
            let hn = body.h0(shifted(north)) as f64;
            let slope = (((he - h) / 20.0).powi(2) + ((hn - h) / 20.0).powi(2)).sqrt();
            if slope > 0.30 {
                continue;
            }
            let score = (h - sea).min(250.0) - 200.0 * slope;
            if best.as_ref().is_none_or(|(s, _)| score > *s) {
                best = Some((score, d));
            }
        }
        let d = best.map_or(fallback.1, |(_, d)| d);
        Site::at(body, d, voxel, radius)
    }

    /// A site centred on the surface in direction `d`.
    pub fn at(body: &Body, d: [f64; 3], voxel: f64, radius: f64) -> Site {
        let d = normalize3(d);
        let (east, north) = frame(d);
        let center = scale3(d, surface_radius(body, d, voxel));
        let chunk = CELLS as f64 * voxel;
        let k = Site::chunks_for(radius, voxel);
        let half = k as f64 * chunk;
        let x0 = -half;
        // Vertical range from a coarse scan of the surface over the square.
        let (mut zmin, mut zmax) = (f64::INFINITY, f64::NEG_INFINITY);
        for j in 0..17 {
            for i in 0..17 {
                let x = x0 + (i as f64 / 16.0) * 2.0 * half;
                let y = -half + (j as f64 / 16.0) * 2.0 * half;
                let z = Site {
                    center,
                    east,
                    north,
                    up: d,
                    voxel,
                    x0,
                    z0: 0.0,
                    ncx: 0,
                    ncy: 0,
                    ncz: 0,
                }
                .ground_z(body, x, y);
                zmin = zmin.min(z);
                zmax = zmax.max(z);
            }
        }
        let z0 = ((zmin - DEPTH) / chunk).floor() * chunk;
        let ncz = (((zmax + ABOVE - z0) / chunk).ceil() as i32).max(1);
        Site {
            center,
            east,
            north,
            up: d,
            voxel,
            x0,
            z0,
            ncx: 2 * k,
            ncy: 2 * k,
            ncz,
        }
    }

    pub fn box_metres(&self) -> [f64; 3] {
        let c = CELLS as f64 * self.voxel;
        [
            self.ncx as f64 * c,
            self.ncy as f64 * c,
            self.ncz as f64 * c,
        ]
    }
}

/// One brick of voxels: `32³` signed distances, chunk-local.
pub struct Brick {
    pub s: Vec<f32>,
}

impl Brick {
    fn filled(body: &Body, site: &Site, c: [i32; 3]) -> Brick {
        let mut s = vec![0.0f32; (CELLS * CELLS * CELLS) as usize];
        for k in 0..CELLS {
            let z = site.lz(c[2] * CELLS + k);
            for j in 0..CELLS {
                let y = site.ly(c[1] * CELLS + j);
                for i in 0..CELLS {
                    let x = site.lx(c[0] * CELLS + i);
                    s[idx(i, j, k)] = site.sdf(body, x, y, z);
                }
            }
        }
        Brick { s }
    }

    #[inline]
    pub fn at(&self, i: i32, j: i32, k: i32) -> f32 {
        self.s[idx(i, j, k)]
    }
}

#[inline]
fn idx(i: i32, j: i32, k: i32) -> usize {
    (i + CELLS * (j + CELLS * k)) as usize
}

#[derive(Default)]
pub struct BuildStats {
    pub bricks: usize,
    pub samples: usize,
    pub bytes: usize,
    pub ms: f64,
}

pub struct Voxels {
    pub site: Site,
    pub bricks: HashMap<[i32; 3], Brick>,
    pub bytes: usize,
    /// Bricks allocated after the build because an edit reached them.
    pub allocated: usize,
}

impl Voxels {
    /// Allocate and fill the chunks the terrain passes through (plus the digging margin).
    pub fn build(body: &Body, site: Site) -> (Voxels, BuildStats) {
        let t = std::time::Instant::now();
        let chunk = CELLS as f64 * site.voxel;
        let mut bricks = HashMap::new();
        let mut samples = 0usize;
        for cz in 0..site.ncz {
            for cy in 0..site.ncy {
                for cx in 0..site.ncx {
                    // The chunk's footprint in x and y, sampled at 3×3 points: does its column
                    // contain any terrain? (Chunks wholly in the air or wholly underground are
                    // not allocated.)
                    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                    for j in 0..3 {
                        for i in 0..3 {
                            let x = site.lx(cx * CELLS) + (i as f64 / 2.0) * chunk;
                            let y = site.ly(cy * CELLS) + (j as f64 / 2.0) * chunk;
                            let z = site.ground_z(body, x, y);
                            lo = lo.min(z);
                            hi = hi.max(z);
                        }
                    }
                    let zs = site.lz(cz * CELLS);
                    if zs + chunk < lo - DEPTH || zs > hi + ABOVE {
                        continue;
                    }
                    let c = [cx, cy, cz];
                    bricks.insert(c, Brick::filled(body, &site, c));
                    samples += (CELLS * CELLS * CELLS) as usize;
                }
            }
        }
        let bytes = samples * std::mem::size_of::<f32>();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let count = bricks.len();
        (
            Voxels {
                site,
                bricks,
                bytes,
                allocated: 0,
            },
            BuildStats {
                bricks: count,
                samples,
                bytes,
                ms,
            },
        )
    }

    /// The brick holding a chunk coordinate, if it is resident.
    #[inline]
    pub fn brick(&self, c: [i32; 3]) -> Option<&Brick> {
        self.bricks.get(&c)
    }

    /// Signed distance at a global sample index, falling back to the field outside the region.
    pub fn node(&self, body: &Body, g: [i32; 3]) -> f32 {
        let c = [
            g[0].div_euclid(CELLS),
            g[1].div_euclid(CELLS),
            g[2].div_euclid(CELLS),
        ];
        let l = [
            g[0].rem_euclid(CELLS),
            g[1].rem_euclid(CELLS),
            g[2].rem_euclid(CELLS),
        ];
        match self.bricks.get(&c) {
            Some(b) => b.at(l[0], l[1], l[2]),
            None => self.site.sdf(
                body,
                self.site.lx(g[0]),
                self.site.ly(g[1]),
                self.site.lz(g[2]),
            ),
        }
    }

    pub fn chunk_count(&self) -> usize {
        self.bricks.len()
    }

    /// Bricks touched by a local box, clamped to the region.
    pub fn chunks_in(&self, min: [f64; 3], max: [f64; 3]) -> Vec<[i32; 3]> {
        let a = self.site.chunk_of(min[0], min[1], min[2]);
        let b = self.site.chunk_of(max[0], max[1], max[2]);
        let mut out = Vec::new();
        for cz in a[2].max(0)..=b[2].min(self.site.ncz - 1) {
            for cy in a[1].max(0)..=b[1].min(self.site.ncy - 1) {
                for cx in a[0].max(0)..=b[0].min(self.site.ncx - 1) {
                    if self.bricks.contains_key(&[cx, cy, cz]) {
                        out.push([cx, cy, cz]);
                    }
                }
            }
        }
        out
    }

    /// Apply one edit record. Returns the chunks whose mesh must be rebuilt (empty for a no-op).
    pub fn apply(&mut self, body: &Body, e: &Edit) -> Vec<[i32; 3]> {
        match *e {
            Edit::Dig { c, r } => self.dig(body, c, r),
            Edit::Add { c, r } => self.add(body, c, r),
            Edit::Flatten { c, r, z } => self.flatten(body, c, r, z),
        }
    }

    /// Samples whose value differs from `base` (same site, unedited), counting bricks that only
    /// exist here against the field. This is the size of a changed-voxel delta.
    pub fn changed_samples(&self, base: &Voxels, body: &Body) -> usize {
        let mut n = 0;
        for (c, b) in &self.bricks {
            match base.bricks.get(c) {
                Some(o) => n += b.s.iter().zip(&o.s).filter(|(a, b)| a != b).count(),
                None => {
                    let o = Brick::filled(body, &self.site, *c);
                    n += b.s.iter().zip(&o.s).filter(|(a, b)| a != b).count();
                }
            }
        }
        n
    }

    /// Bit-identical samples in every brick of both regions.
    pub fn same_as(&self, other: &Voxels) -> bool {
        self.bricks.len() == other.bricks.len()
            && self.bricks.iter().all(|(c, b)| {
                other.bricks.get(c).is_some_and(|o| {
                    b.s.iter()
                        .zip(&o.s)
                        .all(|(x, y)| x.to_bits() == y.to_bits())
                })
            })
    }

    /// Remove a sphere (dig). Returns the chunks whose mesh must be rebuilt.
    pub fn dig(&mut self, body: &Body, c: [f64; 3], radius: f64) -> Vec<[i32; 3]> {
        self.brush(body, c, radius, |d| (radius - d) as f32, f32::max)
    }

    /// Add a sphere (build). Returns the chunks whose mesh must be rebuilt.
    pub fn add(&mut self, body: &Body, c: [f64; 3], radius: f64) -> Vec<[i32; 3]> {
        self.brush(body, c, radius, |d| (d - radius) as f32, f32::min)
    }

    /// Make the brick at `cc` resident (filled from the field) if it is inside the region.
    /// Returns false outside the region. An edit must never be dropped because the band
    /// allocation happened to stop short: a deep dig would open a hole through the region floor.
    fn ensure(&mut self, body: &Body, cc: [i32; 3]) -> bool {
        let s = self.site;
        if cc[0] < 0 || cc[1] < 0 || cc[2] < 0 || cc[0] >= s.ncx || cc[1] >= s.ncy || cc[2] >= s.ncz
        {
            return false;
        }
        if !self.bricks.contains_key(&cc) {
            self.bricks.insert(cc, Brick::filled(body, &s, cc));
            self.bytes += (CELLS * CELLS * CELLS) as usize * std::mem::size_of::<f32>();
            self.allocated += 1;
        }
        true
    }

    /// Chunks to remesh after samples in global index range `[lo, hi]` changed: a chunk meshes
    /// nodes `32c ..= 32c + 33`, so a change also dirties the chunk below on each axis when it
    /// reaches that chunk's two apron layers.
    fn dirty(&self, lo: [i32; 3], hi: [i32; 3]) -> Vec<[i32; 3]> {
        let mut out = Vec::new();
        let from = |l: i32| (l - 2).div_euclid(CELLS);
        let to = |h: i32| h.div_euclid(CELLS);
        for cz in from(lo[2])..=to(hi[2]) {
            for cy in from(lo[1])..=to(hi[1]) {
                for cx in from(lo[0])..=to(hi[0]) {
                    if self.bricks.contains_key(&[cx, cy, cz]) {
                        out.push([cx, cy, cz]);
                    }
                }
            }
        }
        out
    }

    /// Level the ground to local z = `z` inside a vertical cylinder. Returns chunks to remesh.
    pub fn flatten(&mut self, body: &Body, c: [f64; 3], radius: f64, z: f64) -> Vec<[i32; 3]> {
        let site = self.site;
        let r = radius.ceil() + 2.0;
        let a = site.chunk_of(c[0] - r, c[1] - r, z - 1.0);
        let b = site.chunk_of(c[0] + r, c[1] + r, z + 1.0);
        let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
        for cz in a[2].max(0)..=b[2].min(site.ncz - 1) {
            for cy in a[1].max(0)..=b[1].min(site.ncy - 1) {
                for cx in a[0].max(0)..=b[0].min(site.ncx - 1) {
                    let cc = [cx, cy, cz];
                    self.ensure(body, cc);
                    let brick = self.bricks.get_mut(&cc).expect("ensured");
                    for k in 0..CELLS {
                        let lz = site.lz(cz * CELLS + k);
                        for j in 0..CELLS {
                            let ly = site.ly(cy * CELLS + j);
                            for i in 0..CELLS {
                                let lx = site.lx(cx * CELLS + i);
                                let d = ((lx - c[0]).powi(2) + (ly - c[1]).powi(2)).sqrt();
                                if d > radius {
                                    continue;
                                }
                                let v = (lz - z) as f32;
                                let s = &mut brick.s[idx(i, j, k)];
                                if *s != v {
                                    *s = v;
                                    let g = [cx * CELLS + i, cy * CELLS + j, cz * CELLS + k];
                                    for a in 0..3 {
                                        lo[a] = lo[a].min(g[a]);
                                        hi[a] = hi[a].max(g[a]);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if lo[0] > hi[0] {
            return Vec::new();
        }
        self.dirty(lo, hi)
    }

    fn brush(
        &mut self,
        body: &Body,
        c: [f64; 3],
        radius: f64,
        value: impl Fn(f64) -> f32,
        op: impl Fn(f32, f32) -> f32,
    ) -> Vec<[i32; 3]> {
        let site = self.site;
        let a = site.chunk_of(c[0] - radius, c[1] - radius, c[2] - radius);
        let b = site.chunk_of(c[0] + radius, c[1] + radius, c[2] + radius);
        let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
        for cz in a[2]..=b[2] {
            for cy in a[1]..=b[1] {
                for cx in a[0]..=b[0] {
                    let cc = [cx, cy, cz];
                    if !self.ensure(body, cc) {
                        continue;
                    }
                    let brick = self.bricks.get_mut(&cc).expect("ensured");
                    for k in 0..CELLS {
                        let lz = site.lz(cz * CELLS + k);
                        for j in 0..CELLS {
                            let ly = site.ly(cy * CELLS + j);
                            for i in 0..CELLS {
                                let lx = site.lx(cx * CELLS + i);
                                let d = ((lx - c[0]).powi(2)
                                    + (ly - c[1]).powi(2)
                                    + (lz - c[2]).powi(2))
                                .sqrt();
                                if d > radius {
                                    continue;
                                }
                                let s = &mut brick.s[idx(i, j, k)];
                                let v = op(*s, value(d));
                                if v != *s {
                                    *s = v;
                                    let g = [cx * CELLS + i, cy * CELLS + j, cz * CELLS + k];
                                    for a in 0..3 {
                                        lo[a] = lo[a].min(g[a]);
                                        hi[a] = hi[a].max(g[a]);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if lo[0] > hi[0] {
            return Vec::new();
        }
        self.dirty(lo, hi)
    }
}

/// One volume edit, in the site's local frame (the spike's stand-in for docs/18 `EditRecord`'s
/// `op` + `bounds` + `payload`).
#[derive(Clone, Copy, Debug)]
pub enum Edit {
    Dig { c: [f64; 3], r: f64 },
    Add { c: [f64; 3], r: f64 },
    Flatten { c: [f64; 3], r: f64, z: f64 },
}

impl Edit {
    /// Bytes for one record in a fixed binary layout: id u64, sequence u64, createdAt f64,
    /// author u32, baseGeneratorVersion u32, domain + op u8 + u8, chunk i32×3, centre f32×3
    /// relative to the chunk, radius f32, payload f32 (flatten target; occupancy is the op).
    pub const RECORD_BYTES: usize = 8 + 8 + 8 + 4 + 4 + 2 + 12 + 12 + 4 + 4;
}

/// Radius of the solid at a direction: body radius plus the field height.
pub fn surface_radius(body: &Body, d: [f64; 3], spacing: f64) -> f64 {
    let mut s = Sample::default();
    body.sample(d, spacing, Depth::L4, &mut s);
    body.radius + s.height as f64
}

/// Local frame at a radial direction: `(east, north)`, with `east × north = up`.
pub fn frame(up: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let reference = if up[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let east = normalize3(cross3(up, reference));
    let north = cross3(up, east);
    (east, north)
}

pub fn add3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale3(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn len3(a: [f64; 3]) -> f64 {
    dot3(a, a).sqrt()
}

pub fn normalize3(a: [f64; 3]) -> [f64; 3] {
    let l = len3(a);
    if l == 0.0 {
        [0.0, 1.0, 0.0]
    } else {
        scale3(a, 1.0 / l)
    }
}

pub fn to_f32(a: [f64; 3]) -> [f32; 3] {
    [a[0] as f32, a[1] as f32, a[2] as f32]
}
