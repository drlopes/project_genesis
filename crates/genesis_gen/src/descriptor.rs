//! The descriptor core: every coarse, frozen decision about a body (docs/03-planet-descriptor.md).
//!
//! Field names serialise in camelCase as in the docs/03 schema. Blocks whose bake phase does not
//! exist yet are present and `null` (docs/13 bake test 3: absent blocks are explicit), never
//! missing. Identity is [`DescriptorCore::encode`] + [`DescriptorCore::hash`], not the JSON.

use crate::cube::Vec3;
use crate::encode::Encoder;
use serde::{Deserialize, Serialize};

/// Descriptor schema version (docs/03 "Versioning and regeneration").
pub const SCHEMA_VERSION: u32 = 1;
/// Bumped whenever bake output changes for the same seed (docs/16 "Versioning").
pub const GENERATOR_VERSION: u32 = 1;

/// The seed chain (docs/11): a galaxy seed and the indices down to the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedChain {
    pub galaxy: u64,
    pub region: u64,
    pub system: u64,
    pub body: u64,
}

impl SeedChain {
    /// A body in region 0, system 0 of `galaxy`.
    pub fn body(galaxy: u64, body: u64) -> SeedChain {
        SeedChain {
            galaxy,
            region: 0,
            system: 0,
            body,
        }
    }

    /// `bodySeed = H(H(H(galaxySeed, "region", r), "system", s), "body", b)` (docs/11).
    pub fn body_seed(&self) -> genesis_core::Seed {
        genesis_core::Seed(self.galaxy)
            .derive_idx("region", self.region)
            .derive_idx("system", self.system)
            .derive_idx("body", self.body)
    }

    /// The body ID (docs/16: IDs are not seeds; this is positional, `gal:0` is the galaxy index).
    pub fn id(&self) -> String {
        format!(
            "gal:0/reg:{}/sys:{}/body:{}",
            self.region, self.system, self.body
        )
    }
}

/// A block whose bake phase does not exist yet. Always `None`, serialised as `null`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deferred {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bulk {
    /// Reference sphere radius for elevation, metres.
    pub radius: f64,
    /// Kilograms, game scale. Derived from `gravity` until orbits exist (docs/16: gravity is
    /// authoritative; mass may later be tuned for orbits and tides).
    pub mass: f64,
    /// Surface gravity in g (9.80665 m/s²), gameplay-authoritative.
    pub gravity: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rotation {
    /// Game seconds.
    pub day_length: f64,
    /// Degrees.
    pub obliquity: f64,
    /// `null` until the orbit phase exists (tidal lock depends on the parent).
    pub tidal_lock: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaterState {
    Liquid,
    Ice,
    Mixed,
    Vapour,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaterBody {
    Marine,
    Freshwater,
    Brine,
    Hydrocarbon,
    Ammonia,
    Subsurface,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hydrosphere {
    /// Target fraction of the surface below the sea (docs/05).
    pub ocean_fraction: f64,
    /// `liquid` until the energy and atmosphere phases exist (docs/05 derives it from them).
    pub state: WaterState,
    pub water_body: WaterBody,
    /// Absolute radius of the sea surface, metres from the centre; `null` on a dry body.
    pub sea_radius: Option<f64>,
    /// `null` until depression filling exists (docs/05 "lakes at bake").
    pub lakes: Option<Deferred>,
}

/// One tectonic plate (docs/06 "Plates").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plate {
    /// Unit vector of the plate's seed point.
    pub centre: Vec3,
    /// Continental (thicker, higher) or oceanic.
    pub continental: bool,
    /// Unit vector of the Euler pole the plate rotates about.
    pub pole: Vec3,
    /// Angular speed, arbitrary units (only relative motion matters).
    pub speed: f64,
    /// Surface age, 0 (just resurfaced) to 1 (ancient): the plate is the crater-age province.
    pub age: f64,
}

/// One crater (docs/06 "Impacts and cratering"); the list is append-only within a generator version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Crater {
    /// Unit vector of the centre.
    pub centre: Vec3,
    /// Rim radius, metres.
    pub radius: f64,
    /// Bowl depth below the surrounding surface, metres.
    pub depth: f64,
}

/// The hypsometric curve: elevation quantiles of `h0` over the sphere, area-weighted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hypsometry {
    /// Cells per face edge of the coarse relief grid.
    pub grid: u32,
    /// Height above the reference sphere at 0 %, 5 %, …, 100 % of the area, metres.
    pub quantiles: Vec<f64>,
    /// Area-weighted mean height, metres.
    pub mean: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Geology {
    pub plate_count: u32,
    /// 0..1; scales relief amplitude (docs/06).
    pub tectonic_activity: f64,
    /// 0..1; stored for later phases (no L0 effect yet).
    pub volcanism: f64,
    pub plates: Vec<Plate>,
    pub craters: Vec<Crater>,
    /// `null` until the climate phase exists: erosion intensity is climate-driven × geology-driven.
    pub erosion: Option<Deferred>,
    pub hypsometry: Hypsometry,
}

/// The frozen core of a body descriptor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DescriptorCore {
    pub version: u32,
    pub generator: String,
    pub generator_version: u32,
    /// `null` until content packs exist.
    pub content_version: Option<String>,
    pub id: String,
    pub seed_chain: SeedChain,
    pub orbit: Option<Deferred>,
    pub bulk: Bulk,
    pub rotation: Rotation,
    pub energy: Option<Deferred>,
    pub atmosphere: Option<Deferred>,
    pub hydrosphere: Hydrosphere,
    pub geology: Geology,
    pub climate: Option<Deferred>,
    pub regions: Option<Deferred>,
    pub biome_palette: Option<Deferred>,
    pub landform_suites: Option<Deferred>,
    pub materials: Option<Deferred>,
    pub biosphere: Option<Deferred>,
    pub aquatic: Option<Deferred>,
    pub narrative: Option<Deferred>,
}

impl DescriptorCore {
    /// Canonical binary encoding: every field in declaration order, little-endian, floats as their
    /// IEEE bits, strings as length + UTF-8, options as a tag byte. Adding a field appends to the
    /// encoding; this is what [`DescriptorCore::hash`] covers.
    pub fn encode(&self) -> Vec<u8> {
        let mut e = Encoder::default();
        e.u32(self.version);
        e.str(&self.generator);
        e.u32(self.generator_version);
        e.opt_str(self.content_version.as_deref());
        e.str(&self.id);
        let s = &self.seed_chain;
        for v in [s.galaxy, s.region, s.system, s.body] {
            e.u64(v);
        }
        e.deferred(self.orbit);
        let b = &self.bulk;
        e.f64(b.radius);
        e.f64(b.mass);
        e.f64(b.gravity);
        let r = &self.rotation;
        e.f64(r.day_length);
        e.f64(r.obliquity);
        e.opt_bool(r.tidal_lock);
        e.deferred(self.energy);
        e.deferred(self.atmosphere);
        let h = &self.hydrosphere;
        e.f64(h.ocean_fraction);
        e.u8(h.state as u8);
        e.u8(h.water_body as u8);
        e.opt_f64(h.sea_radius);
        e.deferred(h.lakes);
        let g = &self.geology;
        e.u32(g.plate_count);
        e.f64(g.tectonic_activity);
        e.f64(g.volcanism);
        e.u32(g.plates.len() as u32);
        for p in &g.plates {
            e.vec3(p.centre);
            e.bool(p.continental);
            e.vec3(p.pole);
            e.f64(p.speed);
            e.f64(p.age);
        }
        e.u32(g.craters.len() as u32);
        for c in &g.craters {
            e.vec3(c.centre);
            e.f64(c.radius);
            e.f64(c.depth);
        }
        e.deferred(g.erosion);
        e.u32(g.hypsometry.grid);
        e.u32(g.hypsometry.quantiles.len() as u32);
        for q in &g.hypsometry.quantiles {
            e.f64(*q);
        }
        e.f64(g.hypsometry.mean);
        for d in [
            self.climate,
            self.regions,
            self.biome_palette,
            self.landform_suites,
            self.materials,
            self.biosphere,
            self.aquatic,
            self.narrative,
        ] {
            e.deferred(d);
        }
        e.into_bytes()
    }

    /// Identity hash of the core (docs/03: only the core is hashed; caches never are).
    pub fn hash(&self) -> u64 {
        crate::encode::hash(&self.encode())
    }
}
