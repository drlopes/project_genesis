//! L0 relief: `h0 = macroHeight + plateDelta + craterDelta` (docs/09 "L0"), in metres above the
//! reference sphere. `erosionDelta` is 0 until the climate phase exists.
//!
//! **Plates are a soft Voronoi** (docs/06 "Plates"; refines docs/09's `plateDelta`). Every quantity
//! that depends on "which plate is nearest" is blended with smooth weights
//! `w_i ∝ exp(−(d_i − d_min) / softness)`, where `d_i` is the geodesic distance to plate `i`'s seed
//! point after a domain warp. The weights are equal exactly on the Voronoi bisectors, so plates and
//! boundaries are where docs/06 puts them; but the height is continuous everywhere, including at
//! triple junctions, where a "nearest boundary" formula switches abruptly and makes a wall (the
//! B-005 lesson). Boundary features are `Σ_{i<j} amp_ij · 4 w_i w_j`: 1 on a two-plate boundary,
//! fading at junctions and away from boundaries.

use crate::cube::{Vec3, add, angle, cross, dot, length, normalize, scale, sub};
use crate::descriptor::{Crater, Plate};
use crate::params::RockyParams;
use genesis_core::Seed;
use genesis_core::math::f64 as m;
use genesis_core::noise::{Fbm, fbm3};

/// Everything L0 needs, borrowed from a baked core.
pub struct Relief<'a> {
    pub radius: f64,
    pub activity: f64,
    pub plates: &'a [Plate],
    pub craters: &'a [Crater],
    pub params: &'a RockyParams,
    macro_seed: Seed,
    warp_seeds: [Seed; 3],
    /// `cos` of each crater's influence angle (2 × rim radius), for a cheap rejection test.
    crater_cos: Vec<f64>,
}

/// The three L0 terms at one point.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Terms {
    pub macro_height: f64,
    pub plate: f64,
    pub crater: f64,
}

impl Terms {
    pub fn h0(&self) -> f64 {
        self.macro_height + self.plate + self.crater
    }
}

impl<'a> Relief<'a> {
    pub fn new(
        geology_seed: Seed,
        radius: f64,
        activity: f64,
        plates: &'a [Plate],
        craters: &'a [Crater],
        params: &'a RockyParams,
    ) -> Relief<'a> {
        let crater_cos = craters
            .iter()
            .map(|c| m::cos((2.0 * c.radius / radius).min(std::f64::consts::PI)))
            .collect();
        Relief {
            radius,
            activity,
            plates,
            craters,
            params,
            macro_seed: geology_seed.derive("macro"),
            warp_seeds: [
                geology_seed.derive_idx("warp", 0),
                geology_seed.derive_idx("warp", 1),
                geology_seed.derive_idx("warp", 2),
            ],
            crater_cos,
        }
    }

    /// The plate layout's warped direction (low frequency, not band-limited: identical at every
    /// LOD, so bake and query agree).
    fn warp(&self, d: Vec3) -> Vec3 {
        let p = scale(d, self.radius);
        let f = Fbm {
            wavelength: self.params.warp_wavelength,
            octaves: 2,
            lacunarity: 2.0,
            gain: 0.5,
        };
        let w = [0, 1, 2].map(|k| f64::from(fbm3(self.warp_seeds[k], p, &f, 0.0)));
        normalize(add(p, scale(w, self.params.warp_amplitude)))
    }

    /// Normalised soft-Voronoi weights for the given distances (metres) and softness.
    fn weights(dist: &[f64], softness: f64, out: &mut [f64]) {
        let dmin = dist.iter().copied().fold(f64::INFINITY, f64::min);
        let mut sum = 0.0;
        for (o, &d) in out.iter_mut().zip(dist) {
            let x = (d - dmin) / softness;
            *o = if x > 40.0 { 0.0 } else { m::exp(-x) };
            sum += *o;
        }
        for o in out.iter_mut() {
            *o /= sum;
        }
    }

    /// The three terms at direction `d`, band-limited to sample `spacing` (metres; 0 = none).
    pub fn terms(&self, d: Vec3, spacing: f64) -> Terms {
        Terms {
            macro_height: self.macro_height(d, spacing),
            plate: self.plate_delta(d),
            crater: self.crater_delta(d, 0..self.craters.len()),
        }
    }

    pub fn h0(&self, d: Vec3, spacing: f64) -> f64 {
        self.terms(d, spacing).h0()
    }

    /// Plate base elevation (soft blend of continental/oceanic levels) plus macro noise. This is
    /// the part the macro pyramid caches.
    pub fn macro_height(&self, d: Vec3, spacing: f64) -> f64 {
        let p = self.params;
        let q = self.warp(d);
        let n = self.plates.len();
        let mut dist = [0.0f64; 32];
        let mut w = [0.0f64; 32];
        for (i, pl) in self.plates.iter().enumerate() {
            dist[i] = angle(q, pl.centre) * self.radius;
        }
        Self::weights(&dist[..n], p.base_softness, &mut w[..n]);
        let mut base = 0.0;
        for (i, pl) in self.plates.iter().enumerate() {
            base += w[i]
                * if pl.continental {
                    p.base_continental
                } else {
                    p.base_oceanic
                };
        }
        let fbm = Fbm {
            wavelength: p.macro_wavelength,
            octaves: p.macro_octaves,
            lacunarity: 2.0,
            gain: 0.5,
        };
        let noise = f64::from(fbm3(self.macro_seed, scale(d, self.radius), &fbm, spacing));
        base + p.macro_amplitude * (0.5 + self.activity) * noise
    }

    /// Boundary features: mountains where plates converge, rifts where they diverge, nothing on
    /// transform boundaries; continuous in the convergence measure.
    pub fn plate_delta(&self, d: Vec3) -> f64 {
        let p = self.params;
        let q = self.warp(d);
        let n = self.plates.len();
        let mut dist = [0.0f64; 32];
        let mut w = [0.0f64; 32];
        for (i, pl) in self.plates.iter().enumerate() {
            dist[i] = angle(q, pl.centre) * self.radius;
        }
        Self::weights(&dist[..n], p.ridge_softness, &mut w[..n]);
        let mut sum = 0.0;
        for i in 0..n {
            if w[i] < 1e-6 {
                continue;
            }
            for j in (i + 1)..n {
                if w[j] < 1e-6 {
                    continue;
                }
                let (a, b) = (&self.plates[i], &self.plates[j]);
                // Boundary normal at q, pointing from plate i towards plate j.
                let t = sub(b.centre, a.centre);
                let t = sub(t, scale(q, dot(t, q)));
                let tl = length(t);
                if tl < 1e-12 {
                    continue;
                }
                let t = scale(t, 1.0 / tl);
                // Relative surface velocity of i with respect to j (rotation about Euler poles).
                let va = scale(cross(a.pole, q), a.speed);
                let vb = scale(cross(b.pole, q), b.speed);
                let v = sub(va, vb);
                let c = dot(v, t) / (length(v) + 1e-9); // +1 converging, −1 diverging
                let amp = if c >= 0.0 {
                    c * match (a.continental, b.continental) {
                        (true, true) => p.ridge_cc,
                        (false, false) => p.ridge_oo,
                        _ => p.ridge_mixed,
                    }
                } else {
                    c * p.rift
                };
                sum += self.activity * amp * 4.0 * w[i] * w[j];
            }
        }
        sum
    }

    /// Crater bowls and rims (continuous at the rim; B-005), summed over `range` in index order.
    pub fn crater_delta(&self, d: Vec3, range: impl IntoIterator<Item = usize>) -> f64 {
        let mut sum = 0.0;
        for k in range {
            let c = &self.craters[k];
            if dot(d, c.centre) < self.crater_cos[k] {
                continue;
            }
            sum += crater_profile(angle(d, c.centre) * self.radius, c);
        }
        sum
    }

    /// As [`Relief::crater_delta`], over an explicit list of crater indices (the crater index).
    pub fn crater_delta_listed(&self, d: Vec3, list: &[u32]) -> f64 {
        self.crater_delta(d, list.iter().map(|&k| k as usize))
    }
}

/// Height change at distance `dist` (metres) from a crater centre: a bowl inside the rim, a raised
/// rim band outside, zero at the rim and at twice the radius.
pub fn crater_profile(dist: f64, c: &Crater) -> f64 {
    let t = dist / c.radius;
    if t < 1.0 {
        -c.depth * (1.0 - t * t)
    } else if t < 2.0 {
        let u = t - 1.0;
        0.8 * c.depth * u * (1.0 - u)
    } else {
        0.0
    }
}
