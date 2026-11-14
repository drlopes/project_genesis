//! L1 climate: the annual-mean temperature and moisture fields (docs/05 "The temperature field",
//! "Moisture field", "Wind field"). Pure functions of the descriptor and position; the bake
//! evaluates them on the relief grid for the climate summary, and the query will use the same
//! code.
//!
//! Not yet: seasons and the diurnal swing (they never feed classification, docs/05), tidally
//! locked climates (needs orbits), and `terrainDeflection` of the wind.

use crate::cube::{Vec3, cross, dot, length, normalize, scale, sub};
use crate::params::ClimateParams;
use genesis_core::Seed;
use genesis_core::math::f64 as m;
use genesis_core::noise::{Fbm, fbm3};

/// The baked inputs the climate field needs.
#[derive(Clone, Copy, Debug)]
pub struct ClimateInputs {
    pub radius: f64,
    /// Sea surface height above the reference sphere, metres; `None` on a dry body.
    pub sea_level: Option<f64>,
    /// Global mean surface temperature, K (docs/05 `T_base`).
    pub t_base: f64,
    /// Equator-to-pole amplitude after heat transport, K (`A_lat`).
    pub a_lat: f64,
    /// K per metre.
    pub lapse_rate: f64,
    pub climate_seed: Seed,
}

pub struct ClimateField<'a> {
    pub inputs: ClimateInputs,
    pub params: &'a ClimateParams,
}

/// Latitude of a direction (+Y north), radians.
pub fn latitude(d: Vec3) -> f64 {
    m::asin(d[1].clamp(-1.0, 1.0))
}

impl ClimateField<'_> {
    /// Height above sea level of a height above the reference sphere (docs/16 "Vertical
    /// quantities"); on a dry body the reference sphere is the datum.
    pub fn altitude(&self, h: f64) -> f64 {
        h - self.inputs.sea_level.unwrap_or(0.0)
    }

    /// Prevailing wind direction at `d` (docs/05 "Wind field"), as a unit vector, or `None` where
    /// the air is calm (the vector field is continuous but its direction is not defined there).
    pub fn wind(&self, d: Vec3) -> Option<Vec3> {
        let w = self.wind_vector(d);
        let len = length(w);
        if len > 1e-12 {
            Some(normalize(w))
        } else {
            None
        }
    }

    /// The unnormalised wind: `east · (band + 0.15) + north · meridional`, magnitude 0 … ~1.15 …
    /// scaled by `cos(latitude)`. docs/05 normalises it, but normalising is wrong twice over for
    /// the upwind sample:
    ///
    /// - the vector passes through zero where the zonal band crosses −0.15 while the meridional
    ///   noise is near zero; a normalised direction flips there, and the 8 km upwind offset jumps
    ///   with it (found by the climate continuity test: 0.4 moisture within 50 m);
    /// - a continuous *unit* tangent field on a sphere must vanish somewhere (hairy-ball), so a
    ///   normalised basis cannot be smooth near the poles either. Blending in a fallback axis near
    ///   the pole only moves the turning into the blend: the probe found a 0.41 K step at 88.6°
    ///   south within 1 mm.
    ///
    /// So the basis here is the smooth field itself — `north` is +Y projected onto the tangent
    /// plane, `east` is `cross(north, d)` — and the wind shrinks to nothing in calm zones and at
    /// the poles. The upwind sample therefore moves *less* there instead of jumping, and the
    /// temperature and moisture fields stay continuous everywhere.
    pub fn wind_vector(&self, d: Vec3) -> Vec3 {
        let y = [0.0, 1.0, 0.0];
        let north = sub(y, scale(d, dot(d, y)));
        let east = cross(y, d);
        let lat = latitude(d);
        let band = m::tanh(m::sin(6.0 * lat) / 0.4);
        let f = Fbm {
            wavelength: self.params.wind_noise_wavelength,
            octaves: 2,
            lacunarity: 2.0,
            gain: 0.5,
        };
        let meridional = self.params.wind_meridional
            * f64::from(fbm3(
                self.inputs.climate_seed.derive("wind"),
                scale(d, self.inputs.radius),
                &f,
                0.0,
            ));
        [
            east[0] * (band + 0.15) + north[0] * meridional,
            east[1] * (band + 0.15) + north[1] * meridional,
            east[2] * (band + 0.15) + north[2] * meridional,
        ]
    }

    /// The upwind sample point: `distance × wind_vector` metres upwind of `d` (so up to ~1.2 ×
    /// `distance` in the strongest bands, and `d` itself where the air is calm).
    pub fn upwind(&self, d: Vec3, distance: f64) -> Vec3 {
        let w = self.wind_vector(d);
        normalize(sub(d, scale(w, distance / self.inputs.radius)))
    }

    /// Annual-mean surface temperature, K (docs/05 `T_mean`). `h0` and `h0_upwind` are pre-L3
    /// heights above the reference sphere; `d_water` metres to the nearest water (∞ if none).
    pub fn temperature(&self, d: Vec3, h0: f64, h0_upwind: f64, d_water: f64) -> f64 {
        let p = self.params;
        let i = &self.inputs;
        let s = m::sin(latitude(d));
        let t_lat = 3.0 * i.a_lat * (1.0 / 3.0 - s * s);
        let alt = self.altitude(h0).max(0.0).min(p.tropopause);
        let t_lapse = -i.lapse_rate * alt;
        let rise = (h0 - h0_upwind) / (p.wind_distance * p.slope_ref);
        let t_oro = -p.oro_cooling * rise.clamp(0.0, 1.0);
        let t_coast = if d_water.is_finite() {
            p.coastal_warming * m::exp(-d_water / p.coast_length)
        } else {
            0.0
        };
        i.t_base + t_lat + t_lapse + t_oro + t_coast
    }

    /// Annual-mean moisture in `[0, 1]` (docs/05 "Moisture field").
    pub fn moisture(&self, d: Vec3, h0: f64, h0_upwind: f64, d_water: f64, t_mean: f64) -> f64 {
        let p = self.params;
        let f = Fbm {
            wavelength: p.moisture_wavelength,
            octaves: 3,
            lacunarity: 2.0,
            gain: 0.5,
        };
        let noise = f64::from(fbm3(
            self.inputs.climate_seed.derive("moisture"),
            scale(d, self.inputs.radius),
            &f,
            0.0,
        ));
        let m_base = p.moisture_base + p.moisture_noise * noise;
        let lat = latitude(d).to_degrees();
        let g = |x: f64, c: f64, sigma: f64| m::exp(-((x - c) / sigma) * ((x - c) / sigma));
        let al = lat.abs();
        let m_lat = p.itcz * g(lat, 0.0, p.itcz_sigma)
            - p.subtropic * g(al, 30.0, p.subtropic_sigma)
            + p.midlatitude * g(al, 60.0, p.midlatitude_sigma);
        let m_water = if d_water.is_finite() {
            p.water_moisture * m::exp(-d_water / p.water_length)
        } else {
            0.0
        };
        let s_up = (h0 - h0_upwind) / p.wind_distance;
        let m_oro = p.oro_moisture * (s_up / p.slope_ref).clamp(0.0, 1.0);
        let m_shadow = p.rain_shadow * (-s_up / p.slope_ref).clamp(0.0, 1.0);
        let cold = ((t_mean - 240.0) / (273.0 - 240.0)).clamp(0.25, 1.0);
        ((m_base + m_lat + m_water + m_oro - m_shadow) * cold).clamp(0.0, 1.0)
    }
}
