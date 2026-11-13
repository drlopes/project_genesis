//! Engine-agnostic E6 setup: the body, test sites, physics frames, and collision patches.

use bevy::math::{DQuat, DVec3, Vec3};
use genesis_core::Seed;
use spike_app::patch;
use spike_app::quadtree::{Lod, NodeId};
use spike_field::cube::face_uv;
use spike_field::{Body, Depth, Sample};

pub struct World {
    pub body: Body,
    pub lod: Lod,
}

impl World {
    pub fn new(radius: f64) -> World {
        let body = Body::bake_with_radius(Seed(1), radius, 16, 4, 4, 4, 256);
        // Same LOD settings as the app (E3): grid 65, SSE 16.
        let lod = Lod::new(radius, 65, 16.0, 1.0);
        World { body, lod }
    }

    /// Distance from the body centre to the terrain surface (land or sea floor; water is not
    /// solid, docs/18) along `d`, at the finest band, which is what the collision patches use.
    pub fn surface_radius(&self, d: DVec3) -> f64 {
        let level = self.lod.max_level;
        let mut s = Sample::default();
        self.body.sample(
            d.to_array(),
            self.lod.spacing(level),
            Depth::for_band(self.lod.band(level)),
            &mut s,
        );
        self.body.radius + s.height as f64
    }
}

/// Where the engine's coordinates come from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Frame {
    /// Origin on the surface at the site, +Y up; f64 positions are differenced, then cast (E5).
    Local,
    /// Control: body-local positions cast straight to f32.
    Naive,
}

pub struct Site {
    pub origin: DVec3,
    /// Local frame axes to body axes.
    pub rot: DQuat,
    pub frame: Frame,
    pub dir: DVec3,
}

impl Site {
    pub fn at(world: &World, dir: DVec3, frame: Frame) -> Site {
        let dir = dir.normalize();
        let origin = dir * world.surface_radius(dir);
        let reference = if dir.y.abs() < 0.9 {
            DVec3::Y
        } else {
            DVec3::X
        };
        let east = reference.cross(dir).normalize();
        // Local X = east, Y = up, Z = east × up (right-handed).
        let rot = DQuat::from_mat3(&bevy::math::DMat3::from_cols(east, dir, east.cross(dir)));
        Site {
            origin,
            rot,
            frame,
            dir,
        }
    }

    /// Body-local (f64) to engine coordinates (f32).
    pub fn body_to_phys(&self, p: DVec3) -> Vec3 {
        match self.frame {
            Frame::Local => (self.rot.inverse() * (p - self.origin)).as_vec3(),
            Frame::Naive => p.as_vec3(),
        }
    }

    /// Engine coordinates to body-local f64.
    pub fn phys_to_body(&self, v: Vec3) -> DVec3 {
        match self.frame {
            Frame::Local => self.origin + self.rot * v.as_dvec3(),
            Frame::Naive => v.as_dvec3(),
        }
    }

    /// A point `east`, `north` metres from the site along the surface, `up` metres above it.
    pub fn above_ground(&self, world: &World, east: f64, north: f64, up: f64) -> DVec3 {
        let flat = self.origin + self.rot * DVec3::new(east, 0.0, north);
        let d = flat.normalize();
        d * (world.surface_radius(d) + up)
    }

    /// Gravity in engine coordinates (towards the body centre).
    pub fn gravity(&self, g: f32) -> Vec3 {
        self.to_dir(-self.dir) * g
    }

    pub fn to_dir(&self, d: DVec3) -> Vec3 {
        match self.frame {
            Frame::Local => (self.rot.inverse() * d).as_vec3(),
            Frame::Naive => d.as_vec3(),
        }
    }

    /// Height of an engine-space point above the terrain, measured radially (frame-independent).
    pub fn height_above_ground(&self, world: &World, v: Vec3) -> f64 {
        let p = self.phys_to_body(v);
        p.length() - world.surface_radius(p.normalize())
    }

    /// Finest-level patch coordinates (face, x, y) under a body-local direction.
    fn cell(world: &World, d: DVec3) -> (usize, i64, i64) {
        let n = 1i64 << world.lod.max_level;
        let (face, u, v) = face_uv(d.to_array());
        let c = |t: f64| (((t + 1.0) / 2.0) * n as f64).floor() as i64;
        (face, c(u), c(v))
    }

    /// True if an engine-space point lies over the collision block spawned by `patches(k)`, with
    /// a margin of `margin_cells` patches kept clear of the block edge.
    pub fn over_collision(&self, world: &World, v: Vec3, k: i64, margin_cells: i64) -> bool {
        let (f0, x0, y0) = Self::cell(world, self.dir);
        let (f, x, y) = Self::cell(world, self.phys_to_body(v).normalize());
        let r = k - margin_cells;
        f == f0 && (x - x0).abs() <= r && (y - y0).abs() <= r
    }

    /// Slope of the terrain under local (x, z), in degrees, from the surface 1 m east and north.
    pub fn slope_deg(&self, world: &World, x: f64, z: f64) -> f64 {
        let c = self.above_ground(world, x, z, 0.0);
        let e = self.above_ground(world, x + 1.0, z, 0.0);
        let n = self.above_ground(world, x, z + 1.0, 0.0);
        let normal = (e - c).cross(n - c).normalize();
        normal.dot(c.normalize()).abs().acos().to_degrees()
    }

    /// Distance from an engine-space point to the nearest patch seam at `level`, in metres
    /// (approximate: face-space distance times the patch edge length).
    pub fn seam_distance(&self, world: &World, level: u8, v: Vec3) -> f64 {
        let n = (1u64 << level) as f64;
        let (_, u, w) = face_uv(self.phys_to_body(v).normalize().to_array());
        let frac = |t: f64| {
            let f = ((t + 1.0) / 2.0 * n).fract();
            f.min(1.0 - f)
        };
        frac(u).min(frac(w)) * world.lod.spacing(level) * world.lod.intervals as f64
    }

    /// Collision patches at the finest level: a (2k+1)² block around the site, as
    /// engine-space vertices and triangles (skirts excluded).
    pub fn patches(&self, world: &World, k: i64) -> Vec<(Vec<Vec3>, Vec<[u32; 3]>)> {
        self.patches_at(world, world.lod.max_level, k)
    }

    /// As [`Site::patches`], at any level.
    pub fn patches_at(&self, world: &World, level: u8, k: i64) -> Vec<(Vec<Vec3>, Vec<[u32; 3]>)> {
        let n = 1i64 << level;
        let (face, u, v) = face_uv(self.dir.to_array());
        let c = |t: f64| (((t + 1.0) / 2.0) * n as f64).floor() as i64;
        let (cx, cy) = (c(u), c(v));
        let mut out = Vec::new();
        for dy in -k..=k {
            for dx in -k..=k {
                let (x, y) = (cx + dx, cy + dy);
                if x < 0 || y < 0 || x >= n || y >= n {
                    continue;
                }
                let id = NodeId {
                    face: face as u8,
                    level,
                    x: x as u32,
                    y: y as u32,
                };
                let m = patch::build(&world.body, &world.lod, id);
                let (verts, tris) = m.collision_grid(world.lod.intervals as usize);
                out.push((
                    verts
                        .iter()
                        .map(|p| self.body_to_phys(DVec3::from_array(*p)))
                        .collect(),
                    tris,
                ));
            }
        }
        out
    }
}

/// Deterministic site directions, away from cube-face edges so the patch block stays on one face.
pub fn site_dirs(count: usize, seed: u64) -> Vec<DVec3> {
    let mut rng = Rng(seed);
    let mut out = Vec::new();
    while out.len() < count {
        let d = DVec3::new(rng.signed(), rng.signed(), rng.signed());
        if d.length_squared() < 1e-6 || d.length_squared() > 1.0 {
            continue;
        }
        let d = d.normalize();
        let (_, u, v) = face_uv(d.to_array());
        if u.abs() < 0.85 && v.abs() < 0.85 {
            out.push(d);
        }
    }
    out
}

/// splitmix64; test-only randomness, not the generation RNG.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Uniform in [-1, 1).
    pub fn signed(&mut self) -> f64 {
        self.unit() * 2.0 - 1.0
    }
}

pub fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}
