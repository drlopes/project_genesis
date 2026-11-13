//! `bake_core` and `bake_caches` for the `rocky` class (docs/03 "Bake ordering", docs/13).
//!
//! Phases implemented: identity → bulk (+ rotation) → water inventory → geology → coarse relief →
//! hydrosphere completion (sea level). Every other block is explicitly `null` (see
//! [`crate::descriptor`]). Each phase draws only from its own subsystem seed (docs/11 tag rule 2):
//! `bulk`, `hydrosphere`, `geology`; plates and craters from `H(geologySeed, "plate" | "crater", i)`.
//!
//! Purposes drawn from each seed (append-only within a generator version, like the tags):
//!
//! | Seed | Purposes |
//! |---|---|
//! | `bulk` | `radius`, `gravity`, `day_length`, `obliquity` |
//! | `hydrosphere` | `ocean_fraction` |
//! | `geology` | `plate_count`, `tectonic_activity`, `volcanism`; noise seeds `macro`, `warp` (0–2) |
//! | `plate`, i | `continental`, `centre`, `pole`, `speed`, `age` |
//! | `crater`, i | `centre`, `size`, `keep` |

use crate::cube::{self, Grid, Vec3, angle, face_uv};
use crate::descriptor::*;
use crate::params::{Normal, Range, RockyParams};
use crate::relief::Relief;
use genesis_core::Seed;
use genesis_core::math::f64 as m;
use genesis_core::rng;

/// Standard gravity, m/s² (docs/16 units).
pub const G0: f64 = 9.806_65;
/// Gravitational constant, SI. Only used to derive a placeholder mass from gravity.
pub const G: f64 = 6.674_30e-11;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum BakeError {
    #[error("invalid bake parameter: {0}")]
    InvalidParams(String),
}

fn check(params: &RockyParams) -> Result<(), BakeError> {
    let bad = |what: &str| Err(BakeError::InvalidParams(what.to_string()));
    for (name, r) in [
        ("radius", params.radius),
        ("gravity", params.gravity),
        ("day_length", params.day_length),
        ("obliquity", params.obliquity),
        ("volcanism", params.volcanism),
        ("age_oceanic", params.age_oceanic),
        ("age_continental", params.age_continental),
        ("crater_radius", params.crater_radius),
    ] {
        if !(r.lo.is_finite() && r.hi.is_finite() && r.lo <= r.hi) {
            return bad(name);
        }
    }
    if params.radius.lo <= 0.0 || params.gravity.lo <= 0.0 || params.crater_radius.lo <= 0.0 {
        return bad("radius, gravity and crater radius must be positive");
    }
    let (lo, hi) = params.plates;
    if lo < 2 || hi < lo || hi > 32 {
        return bad("plates must be within 2..=32");
    }
    if params.relief_grid < 8 || params.pyramid_grid < 3 || params.crater_index_grid < 1 {
        return bad("grid sizes");
    }
    Ok(())
}

fn uniform(seed: Seed, purpose: &str, r: Range) -> f64 {
    r.lo + (r.hi - r.lo) * rng::unit_f64(seed, purpose, 0)
}

/// `N(0, 1)` by Box–Muller from two counter draws (index 0 and 1 of `purpose`).
fn gaussian(seed: Seed, purpose: &str) -> f64 {
    // u1 in (0, 1): never 0, so the logarithm is finite.
    let u1 = ((rng::u64_at(seed, purpose, 0) >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
    let u2 = rng::unit_f64(seed, purpose, 1);
    (-2.0 * m::ln(u1)).sqrt() * m::cos(std::f64::consts::TAU * u2)
}

fn normal(seed: Seed, purpose: &str, n: Normal) -> f64 {
    (n.median + n.spread * gaussian(seed, purpose)).clamp(n.lo, n.hi)
}

/// A uniformly distributed unit vector (+Y north).
fn on_sphere(seed: Seed, purpose: &str) -> Vec3 {
    let y = 2.0 * rng::unit_f64(seed, purpose, 0) - 1.0;
    let phi = std::f64::consts::TAU * rng::unit_f64(seed, purpose, 1);
    let r = (1.0 - y * y).max(0.0).sqrt();
    [r * m::cos(phi), y, r * m::sin(phi)]
}

/// Bake the frozen core of a rocky body.
pub fn bake_core(chain: SeedChain, params: &RockyParams) -> Result<DescriptorCore, BakeError> {
    check(params)?;
    let body = chain.body_seed();

    // ---- bulk and rotation ----
    let s = body.derive("bulk");
    let radius = uniform(s, "radius", params.radius);
    let gravity = uniform(s, "gravity", params.gravity);
    let bulk = Bulk {
        radius,
        mass: gravity * G0 * radius * radius / G,
        gravity,
    };
    let rotation = Rotation {
        day_length: uniform(s, "day_length", params.day_length),
        obliquity: uniform(s, "obliquity", params.obliquity),
        tidal_lock: None,
    };

    // ---- water inventory ----
    let s = body.derive("hydrosphere");
    let drawn = normal(s, "ocean_fraction", params.ocean_fraction);
    let dry = drawn < params.dry_below;
    let ocean_fraction = if dry { 0.0 } else { drawn };

    // ---- geology ----
    let geo = body.derive("geology");
    let (lo, hi) = params.plates;
    let plate_count = lo + rng::below(geo, "plate_count", 0, u64::from(hi - lo + 1)) as u32;
    let tectonic_activity = normal(geo, "tectonic_activity", params.tectonic_activity);
    let volcanism = uniform(geo, "volcanism", params.volcanism);
    let plates: Vec<Plate> = (0..u64::from(plate_count))
        .map(|i| {
            let ps = geo.derive_idx("plate", i);
            let continental = rng::unit_f64(ps, "continental", 0) < params.continental;
            Plate {
                centre: on_sphere(ps, "centre"),
                continental,
                pole: on_sphere(ps, "pole"),
                speed: uniform(ps, "speed", Range { lo: 0.5, hi: 1.0 }),
                age: uniform(
                    ps,
                    "age",
                    if continental {
                        params.age_continental
                    } else {
                        params.age_oceanic
                    },
                ),
            }
        })
        .collect();
    let craters = place_craters(geo, radius, &plates, params);

    // ---- coarse relief: h0 on the cell grid, area-weighted ----
    let relief = Relief::new(geo, radius, tectonic_activity, &plates, &craters, params);
    let n = params.relief_grid;
    let spacing = 2.0 / n as f64 * radius;
    let mut cells: Vec<(f64, f64)> = Vec::with_capacity(6 * n * n);
    for face in 0..6 {
        for j in 0..n {
            for i in 0..n {
                let (d, w) = cube::cell(face, n, i, j);
                cells.push((relief.h0(d, spacing), w));
            }
        }
    }
    // Sort by height; ties keep grid order (stable sort), so the result is fully determined.
    cells.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = cells.iter().map(|c| c.1).sum();
    let mean = cells.iter().map(|c| c.0 * c.1).sum::<f64>() / total;
    let quantile = |q: f64| -> f64 {
        let target = q * total;
        let mut cum = 0.0;
        for (k, &(h, w)) in cells.iter().enumerate() {
            cum += w;
            if cum >= target || k + 1 == cells.len() {
                return h;
            }
        }
        cells[cells.len() - 1].0
    };
    let quantiles: Vec<f64> = (0..=20).map(|k| quantile(k as f64 / 20.0)).collect();

    // ---- hydrosphere completion: the sea level that puts `ocean_fraction` of the area below ----
    let sea_radius = if dry {
        None
    } else {
        let target = ocean_fraction * total;
        let mut cum = 0.0;
        let mut level = cells[cells.len() - 1].0;
        for (k, &(h, w)) in cells.iter().enumerate() {
            cum += w;
            if cum >= target {
                // Between this cell and the next one up, so this cell is under water.
                let next = cells.get(k + 1).map_or(h, |c| c.0);
                level = 0.5 * (h + next);
                break;
            }
        }
        Some(radius + level)
    };

    Ok(DescriptorCore {
        version: SCHEMA_VERSION,
        generator: "rocky".into(),
        generator_version: GENERATOR_VERSION,
        content_version: None,
        id: chain.id(),
        seed_chain: chain,
        orbit: None,
        bulk,
        rotation,
        energy: None,
        atmosphere: None,
        hydrosphere: Hydrosphere {
            ocean_fraction,
            state: if dry {
                WaterState::None
            } else {
                WaterState::Liquid
            },
            water_body: WaterBody::Marine,
            sea_radius,
            lakes: None,
        },
        geology: Geology {
            plate_count,
            tectonic_activity,
            volcanism,
            plates,
            craters,
            erosion: None,
            hypsometry: Hypsometry {
                grid: n as u32,
                quantiles,
                mean,
            },
        },
        climate: None,
        regions: None,
        biome_palette: None,
        landform_suites: None,
        materials: None,
        biosphere: None,
        aquatic: None,
        narrative: None,
    })
}

/// Crater accumulation (docs/06): attempts at uniform positions with power-law sizes, kept with
/// probability equal to the surface age of the plate they land on, and rejected if too close to an
/// earlier crater of similar size. Attempt `i` always uses `H(geologySeed, "crater", i)`.
fn place_craters(geo: Seed, radius: f64, plates: &[Plate], p: &RockyParams) -> Vec<Crater> {
    let area_km2 = 4.0 * std::f64::consts::PI * radius * radius / 1e6;
    let attempts = (area_km2 * p.crater_density).round() as u64;
    let r_min = p.crater_radius.lo;
    let r_max = p
        .crater_radius
        .hi
        .min(radius * p.crater_max_fraction)
        .max(r_min);
    let s = p.crater_slope;
    let mut out: Vec<Crater> = Vec::new();
    for i in 0..attempts {
        let cs = geo.derive_idx("crater", i);
        let centre = on_sphere(cs, "centre");
        // Inverse CDF of N(> r) ∝ r^−s on [r_min, r_max].
        let u = rng::unit_f64(cs, "size", 0);
        let r = r_min * m::powf(1.0 - u * (1.0 - m::powf(r_min / r_max, s)), -1.0 / s);
        let nearest = plates
            .iter()
            .enumerate()
            .map(|(k, pl)| (angle(centre, pl.centre), k))
            .fold((f64::INFINITY, 0), |a, b| if b.0 < a.0 { b } else { a })
            .1;
        if rng::unit_f64(cs, "keep", 0) >= plates[nearest].age {
            continue;
        }
        let crowded = out.iter().any(|c| {
            let (small, large) = if c.radius < r {
                (c.radius, r)
            } else {
                (r, c.radius)
            };
            small / large > p.crater_similar
                && angle(centre, c.centre) * radius < p.crater_spacing * (c.radius + r)
        });
        if crowded {
            continue;
        }
        out.push(Crater {
            centre,
            radius: r,
            depth: p.crater_depth * r,
        });
    }
    out
}

/// Caches rebuilt from the core: never identity (docs/03 "Core versus caches").
pub struct Caches {
    /// `macroHeight` at the pyramid nodes, band-limited to the node spacing.
    pub pyramid: Grid,
    pub crater_index: CraterIndex,
}

/// Cube-sphere cells listing, in ascending order, the craters whose influence (2 × rim radius)
/// can reach the cell (docs/06, `DECIDED` from E1).
pub struct CraterIndex {
    pub n: usize,
    pub cells: Vec<Vec<u32>>,
}

impl CraterIndex {
    pub fn build(radius: f64, craters: &[Crater], n: usize) -> CraterIndex {
        let mut cells = Vec::with_capacity(6 * n * n);
        let h = 2.0 / n as f64;
        for face in 0..6 {
            for j in 0..n {
                for i in 0..n {
                    let (c, _) = cube::cell(face, n, i, j);
                    // Angular radius of the cell: the farthest corner from its centre.
                    let mut reach: f64 = 0.0;
                    for (di, dj) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let u = -1.0 + (i + di) as f64 * h;
                        let v = -1.0 + (j + dj) as f64 * h;
                        reach = reach.max(angle(c, cube::dir_of(face, u, v)));
                    }
                    let list = craters
                        .iter()
                        .enumerate()
                        .filter(|(_, k)| angle(c, k.centre) <= reach + 2.0 * k.radius / radius)
                        .map(|(idx, _)| idx as u32)
                        .collect();
                    cells.push(list);
                }
            }
        }
        CraterIndex { n, cells }
    }

    pub fn at(&self, d: Vec3) -> &[u32] {
        let (face, u, v) = face_uv(d);
        let n = self.n;
        let i = (((u + 1.0) * 0.5 * n as f64) as usize).min(n - 1);
        let j = (((v + 1.0) * 0.5 * n as f64) as usize).min(n - 1);
        &self.cells[face * n * n + j * n + i]
    }

    pub fn entries(&self) -> usize {
        self.cells.iter().map(Vec::len).sum()
    }
}

/// The relief of a baked core (the same field the bake evaluated).
pub fn relief<'a>(core: &'a DescriptorCore, params: &'a RockyParams) -> Relief<'a> {
    Relief::new(
        core.seed_chain.body_seed().derive("geology"),
        core.bulk.radius,
        core.geology.tectonic_activity,
        &core.geology.plates,
        &core.geology.craters,
        params,
    )
}

pub fn bake_caches(core: &DescriptorCore, params: &RockyParams) -> Caches {
    let relief = relief(core, params);
    let radius = core.bulk.radius;
    let n = params.pyramid_grid;
    let spacing = 2.0 / (n - 1) as f64 * radius;
    let pyramid = Grid::build(n, |d| relief.macro_height(d, spacing) as f32);
    Caches {
        pyramid,
        crater_index: CraterIndex::build(radius, &core.geology.craters, params.crater_index_grid),
    }
}

/// `h0` from the caches, as the query will compute it: pyramid + plate boundaries + indexed craters.
pub fn h0_cached(relief: &Relief, caches: &Caches, d: Vec3) -> f64 {
    f64::from(caches.pyramid.sample(d))
        + relief.plate_delta(d)
        + relief.crater_delta_listed(d, caches.crater_index.at(d))
}

/// Fraction of the sphere's area where `h0 < level` (metres above the reference sphere), measured
/// on an independent `n × n` cell grid. A check for the sea-level solve, not used by the bake.
pub fn area_below(relief: &Relief, level: f64, n: usize) -> f64 {
    let spacing = 2.0 / n as f64 * relief.radius;
    let (mut below, mut total) = (0.0, 0.0);
    for face in 0..6 {
        for j in 0..n {
            for i in 0..n {
                let (d, w) = cube::cell(face, n, i, j);
                total += w;
                if relief.h0(d, spacing) < level {
                    below += w;
                }
            }
        }
    }
    below / total
}
