//! Physical bake phases after the sea level (P1-C1): energy and atmosphere (per-body stand-in
//! draws until stars and orbits exist), `T_base` with the ice-albedo feedback, the water state,
//! the climate summary over land, and the biosphere level (docs/03 bake steps 2–4, 8, 10;
//! docs/05).
//!
//! Each phase draws only from its own subsystem seed (docs/11): `energy` (`insolation`,
//! `albedo`, `internal_heat`), `atmosphere` (`present`, `pressure`, `greenhouse`), `biosphere`
//! (`level`, `density`); the climate field's noise uses `climate` (`moisture`, `wind`).

use crate::bake::{gaussian, uniform};
use crate::climate::{ClimateField, ClimateInputs, latitude};
use crate::cube::{self, Grid, Vec3, angle, cross, dot, normalize, scale};
use crate::descriptor::*;
use crate::params::RockyParams;
use crate::relief::Relief;
use genesis_core::Seed;
use genesis_core::math::f64 as m;
use genesis_core::rng;

pub const FREEZING: f64 = 273.15;
/// Triple-point pressure of water, Pa: below it water cannot be liquid.
pub const TRIPLE_POINT: f64 = 611.657;

pub struct PhysicalInputs {
    pub radius: f64,
    pub gravity: f64,
    /// Relief cell spacing, metres (band-limit for h0).
    pub spacing: f64,
    /// Sea surface above the reference sphere, metres; `None` on a dry body.
    pub sea_level: Option<f64>,
    pub ocean_fraction: f64,
}

pub struct Physical {
    pub energy: Energy,
    pub atmosphere: Atmosphere,
    pub state: WaterState,
    pub climate: Climate,
    pub biosphere: Biosphere,
}

/// Boiling point of water at pressure `p` (Pa), K (Clausius–Clapeyron from 373.15 K at 1 atm).
pub fn boiling_point(p: f64) -> f64 {
    1.0 / (1.0 / 373.15 - 8.314 * m::ln(p / 101_325.0) / 40_700.0)
}

/// Area fraction of the sea surface whose sea-level temperature (`T_base` + latitude term) is
/// below freezing. 0 on a dry body.
fn frozen_fraction(
    grid: &[(Vec3, f64, f64)],
    sea_level: Option<f64>,
    t_base: f64,
    a_lat: f64,
) -> f64 {
    let Some(sea) = sea_level else {
        return 0.0;
    };
    let (mut frozen, mut total) = (0.0, 0.0);
    for &(d, w, h) in grid {
        if h < sea {
            total += w;
            let s = m::sin(latitude(d));
            if t_base + 3.0 * a_lat * (1.0 / 3.0 - s * s) < FREEZING {
                frozen += w;
            }
        }
    }
    if total > 0.0 { frozen / total } else { 0.0 }
}

/// Distance to the nearest water, metres, at the nodes of an `n + 1` node grid (docs/05
/// "Distance to water"): 0 where the node itself is under the sea, otherwise the geodesic
/// distance to the nearest water cell centre of an `n × n` cell grid. `None` on a dry body. This
/// is the water-distance cache; the bake summary and the query sample the same grid.
pub fn water_distance(relief: &Relief, sea_level: Option<f64>, n: usize) -> Option<Grid> {
    let sea = sea_level?;
    let spacing = 2.0 / n as f64 * relief.radius;
    let mut water: Vec<Vec3> = Vec::new();
    for face in 0..6 {
        for j in 0..n {
            for i in 0..n {
                let (d, _) = cube::cell(face, n, i, j);
                if relief.h0(d, spacing) < sea {
                    water.push(d);
                }
            }
        }
    }
    if water.is_empty() {
        return None;
    }
    Some(Grid::build(n + 1, |d| {
        if relief.h0(d, spacing) < sea {
            return 0.0;
        }
        // Nearest by the largest dot product, then one exact angle.
        let best = water
            .iter()
            .copied()
            .fold((f64::NEG_INFINITY, [0.0; 3]), |b, w| {
                let c = dot(d, w);
                if c > b.0 { (c, w) } else { b }
            })
            .1;
        (angle(d, best) * relief.radius) as f32
    }))
}

/// Slope in degrees from `h0` differences over 20 m in two tangent directions.
fn slope(relief: &Relief, d: Vec3, h: f64, spacing: f64) -> f64 {
    let r = if d[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let e1 = normalize(cross(d, r));
    let e2 = cross(d, e1);
    let step = 20.0 / relief.radius;
    let at = |e: Vec3| relief.h0(normalize(cube::add(d, scale(e, step))), spacing);
    let (gx, gy) = ((at(e1) - h) / 20.0, (at(e2) - h) / 20.0);
    m::atan((gx * gx + gy * gy).sqrt()).to_degrees()
}

#[derive(Default)]
struct Acc {
    min: f64,
    max: f64,
    sum: f64,
    weight: f64,
}

impl Acc {
    fn new() -> Acc {
        Acc {
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            ..Acc::default()
        }
    }
    fn add(&mut self, x: f64, w: f64) {
        self.min = self.min.min(x);
        self.max = self.max.max(x);
        self.sum += x * w;
        self.weight += w;
    }
    fn stat(&self) -> Stat {
        if self.weight > 0.0 {
            Stat {
                min: self.min,
                max: self.max,
                mean: self.sum / self.weight,
            }
        } else {
            Stat {
                min: 0.0,
                max: 0.0,
                mean: 0.0,
            }
        }
    }
}

pub fn physical_phases(
    body: Seed,
    relief: &Relief,
    grid: &[(Vec3, f64, f64)],
    inp: PhysicalInputs,
    params: &RockyParams,
) -> Physical {
    let p = &params.climate;

    // ---- energy ----
    let s = body.derive("energy");
    let insolation = m::exp(p.insolation_sigma * gaussian(s, "insolation"))
        .clamp(p.insolation.lo, p.insolation.hi);
    let albedo0 = uniform(s, "albedo", p.albedo);
    let internal_heat = uniform(s, "internal_heat", p.internal_heat);

    // ---- atmosphere ----
    let s = body.derive("atmosphere");
    let present = rng::unit_f64(s, "present", 0) < p.atmosphere;
    let pressure = if present {
        m::exp(m::ln(p.pressure_median) + p.pressure_sigma * gaussian(s, "pressure"))
            .clamp(p.pressure.lo, p.pressure.hi)
    } else {
        0.0
    };
    let greenhouse = if present {
        -33.0
            + 33.0 * (pressure / 1e5).min(3.0).sqrt()
            + p.greenhouse_sigma * gaussian(s, "greenhouse")
    } else {
        -33.0
    };

    // ---- temperature with the ice-albedo feedback (docs/05: two iterations) ----
    let transport = ((pressure / 1e5).clamp(0.0, 1.0) * p.transport_scale).clamp(0.0, 1.0);
    let a_lat = p.latitude_amplitude * (1.0 - transport);
    let lapse_rate = p.lapse * inp.gravity * m::powf(pressure.max(1e3) / 1e5, 0.2);
    let t_base_at = |a: f64| {
        288.0 * m::powf(insolation, 0.25) * m::powf((1.0 - a) / 0.7, 0.25)
            + greenhouse
            + p.internal_warming * internal_heat
    };
    let mut albedo = albedo0;
    for _ in 0..2 {
        let f = frozen_fraction(grid, inp.sea_level, t_base_at(albedo), a_lat);
        albedo = albedo0 + (p.ice_albedo - albedo0) * f * inp.ocean_fraction;
    }
    let t_base = t_base_at(albedo);
    let frozen = frozen_fraction(grid, inp.sea_level, t_base, a_lat);

    // ---- water state (docs/05 "Water inventory and state") ----
    let state = if inp.sea_level.is_none() {
        WaterState::None
    } else if pressure < TRIPLE_POINT {
        if t_base < FREEZING {
            WaterState::Ice
        } else {
            WaterState::Vapour
        }
    } else if t_base >= boiling_point(pressure) {
        WaterState::Vapour
    } else if frozen > 0.9 {
        WaterState::Ice
    } else if frozen > 0.3 {
        WaterState::Mixed
    } else {
        WaterState::Liquid
    };
    // Boiled-off water leaves no sea: climate is computed as on a dry body.
    let sea_level = if state == WaterState::Vapour {
        None
    } else {
        inp.sea_level
    };

    // ---- climate summary over land ----
    let field = ClimateField {
        inputs: ClimateInputs {
            radius: inp.radius,
            sea_level,
            t_base,
            a_lat,
            lapse_rate,
            climate_seed: body.derive("climate"),
        },
        params: p,
    };
    let distance = water_distance(relief, sea_level, p.water_distance_grid);
    let (mut t, mut mo, mut alt, mut sl) = (Acc::new(), Acc::new(), Acc::new(), Acc::new());
    let (mut land, mut total) = (0.0, 0.0);
    for &(d, w, h) in grid {
        total += w;
        if sea_level.is_some_and(|sea| h < sea) {
            continue;
        }
        land += w;
        let up = relief.h0(field.upwind(d, p.wind_distance), inp.spacing);
        let dw = distance
            .as_ref()
            .map_or(f64::INFINITY, |g| f64::from(g.sample(d)));
        let temp = field.temperature(d, h, up, dw);
        t.add(temp, w);
        mo.add(field.moisture(d, h, up, dw, temp), w);
        alt.add(field.altitude(h), w);
        sl.add(slope(relief, d, h, inp.spacing), w);
    }
    let climate = Climate {
        mean_surface_temp: t_base,
        latitude_amplitude: a_lat,
        transport,
        lapse_rate,
        frozen_sea_fraction: frozen,
        ranges: ClimateRanges {
            land_fraction: land / total,
            temperature: t.stat(),
            moisture: mo.stat(),
            altitude: alt.stat(),
            slope: sl.stat(),
        },
        tidal_lock_climate: None,
    };

    // ---- biosphere: possible with liquid water in a temperature window ----
    let s = body.derive("biosphere");
    let possible = matches!(state, WaterState::Liquid | WaterState::Mixed)
        && (p.biosphere_temperature.lo..=p.biosphere_temperature.hi).contains(&t_base);
    let level = if possible {
        let wsum: f64 = p.biosphere_weights.iter().sum();
        let u = rng::unit_f64(s, "level", 0) * wsum;
        let mut cum = 0.0;
        let levels = [
            BiosphereLevel::None,
            BiosphereLevel::Microbial,
            BiosphereLevel::Complex,
            BiosphereLevel::Exotic,
        ];
        let mut out = BiosphereLevel::Exotic;
        for (l, w) in levels.iter().zip(p.biosphere_weights) {
            cum += w;
            if u < cum {
                out = *l;
                break;
            }
        }
        out
    } else {
        BiosphereLevel::None
    };
    let density = match level {
        BiosphereLevel::None => 0.0,
        BiosphereLevel::Microbial => 0.1 + 0.3 * rng::unit_f64(s, "density", 0),
        _ => 0.3 + 0.7 * rng::unit_f64(s, "density", 0),
    };

    Physical {
        energy: Energy {
            insolation,
            albedo,
            greenhouse,
            internal_heat,
            tidal_heat: 0.0,
        },
        atmosphere: Atmosphere {
            present,
            surface_pressure: pressure,
        },
        state,
        climate,
        biosphere: Biosphere { level, density },
    }
}
