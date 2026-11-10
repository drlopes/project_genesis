//! E1 synthetic body: a cost-faithful version of the docs/09 field stack.
//!
//! Faithful where cost is concerned (number of noise evaluations, pyramid fetches, the docs/05
//! climate terms, the full docs/19 classifier). Not faithful in look: the shapes are stand-ins.

pub mod classify;
pub mod cube;

use classify::{Axes, Content, MAXF, Weights};
use cube::{Grid, Vec3, normalize};
use genesis_core::math::f32 as m;
use genesis_core::noise::{Fbm, fbm3, gradient3};
use genesis_core::{Seed, rng};

/// Max sample spacing per LOD band, metres (docs/10).
pub const BAND_SPACING: [f64; 5] = [4000.0, 1000.0, 50.0, 4.0, 0.5];

pub struct Crater {
    pub centre: Vec3,
    pub radius: f64,
    pub depth: f32,
}

pub struct Suite {
    pub seed: Seed,
    pub fbm: Fbm,
    pub amplitude: f32,
    pub slope: classify::Range,
    pub families: [usize; 2],
}

pub struct Body {
    pub seed: Seed,
    pub radius: f64,
    pub sea_surface: f32,
    pub pyramid: Grid,
    pub water_distance: Grid,
    pub craters: Vec<Crater>,
    pub suites: Vec<Suite>,
    pub content: Content,
    geology: Seed,
    climate: Seed,
    material: Seed,
    micro: Seed,
}

/// What a layer-limited evaluation may include.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Depth {
    L0,
    L1,
    L2,
    L3,
    L4,
}

impl Depth {
    /// Layers evaluated at each LOD band (docs/10 table).
    pub fn for_band(band: usize) -> Depth {
        match band {
            0 => Depth::L0,
            1 => Depth::L1,
            2 | 3 => Depth::L3,
            _ => Depth::L4,
        }
    }
}

pub struct Sample {
    pub height: f32,
    pub temp: f32,
    pub moisture: f32,
    pub albedo: [f32; 3],
    pub weights: Weights,
}

impl Default for Sample {
    fn default() -> Self {
        Sample {
            height: 0.0,
            temp: 0.0,
            moisture: 0.0,
            albedo: [0.0; 3],
            weights: Weights {
                family: [0.0; MAXF + 1],
                leaf: [[0.0; 2]; MAXF + 1],
                n: 0,
            },
        }
    }
}

const MACRO: Fbm = Fbm {
    wavelength: 12_000.0,
    octaves: 6,
    lacunarity: 2.0,
    gain: 0.5,
};
const EROSION: Fbm = Fbm {
    wavelength: 20_000.0,
    octaves: 2,
    lacunarity: 2.0,
    gain: 0.5,
};
const MICRO: Fbm = Fbm {
    wavelength: 4.0,
    octaves: 3,
    lacunarity: 2.0,
    gain: 0.5,
};

impl Body {
    /// Bake: macro pyramid (6 × n × n), water-distance cache, crater list, stub content.
    pub fn bake(
        seed: Seed,
        palette: usize,
        suites: usize,
        k: u32,
        budget: usize,
        n: usize,
    ) -> Body {
        let radius = 24_000.0;
        let geology = seed.derive("geology");
        let pyramid = Grid::build(n, |d| {
            let p = [d[0] * radius, d[1] * radius, d[2] * radius];
            1500.0 * fbm3(geology.derive("macro"), p, &MACRO, 0.0)
        });
        let sea_surface = 40.0;
        // Stand-in for the baked distance field: distance grows with height above the sea.
        let water_distance = Grid::build(64, |d| {
            let h = pyramid.sample(d);
            ((h - sea_surface).max(0.0) * 12.0).min(30_000.0)
        });
        let craters = (0..48)
            .map(|i| {
                let c = |a: &str| rng::range_f32(geology, a, i, -1.0, 1.0) as f64;
                Crater {
                    centre: normalize([c("crater_x"), c("crater_y"), c("crater_z")]),
                    radius: rng::range_f32(geology, "crater_r", i, 200.0, 3000.0) as f64,
                    depth: rng::range_f32(geology, "crater_d", i, 20.0, 300.0),
                }
            })
            .collect();
        let suites = (0..suites)
            .map(|i| {
                let i64 = i as u64;
                Suite {
                    seed: seed.derive_idx("suite", i64),
                    fbm: Fbm {
                        wavelength: 120.0 * (1.0 + i as f64),
                        octaves: 4,
                        lacunarity: 2.0,
                        gain: 0.5,
                    },
                    amplitude: 8.0 + 4.0 * i as f32,
                    slope: classify::Range {
                        lo: 0.0,
                        hi: 25.0,
                        falloff: 5.0,
                    },
                    families: [1 + i % (palette - 1), 1 + (i + 1) % (palette - 1)],
                }
            })
            .collect();
        Body {
            seed,
            radius,
            sea_surface,
            pyramid,
            water_distance,
            craters,
            suites,
            content: Content::stub(seed, palette, k, budget),
            geology,
            climate: seed.derive("climate"),
            material: seed.derive("material"),
            micro: seed.derive("micro"),
        }
    }

    /// L0: macro pyramid + plate + crater + erosion deltas (docs/09 `h0`).
    pub fn h0(&self, d: Vec3) -> f32 {
        let p = [d[0] * self.radius, d[1] * self.radius, d[2] * self.radius];
        let macro_h = self.pyramid.sample(d);
        let ridge = 1.0 - gradient3(self.geology.derive("plate"), scale(p, 8000.0)).abs();
        let plate = 600.0 * ridge * ridge - 200.0;
        let mut crater = 0.0;
        for c in &self.craters {
            let dx = [d[0] - c.centre[0], d[1] - c.centre[1], d[2] - c.centre[2]];
            let dist = (dx[0] * dx[0] + dx[1] * dx[1] + dx[2] * dx[2]).sqrt() * self.radius;
            if dist < 2.0 * c.radius {
                let t = (dist / c.radius) as f32;
                crater += if t < 1.0 {
                    -c.depth * (1.0 - t * t)
                } else {
                    0.2 * c.depth * (2.0 - t)
                };
            }
        }
        let erosion = 80.0 * fbm3(self.geology.derive("erosion"), p, &EROSION, 0.0);
        macro_h + plate + crater + erosion
    }

    /// Full sample at direction `d` and sample spacing `spacing`, evaluated up to `depth`.
    pub fn sample(&self, d: Vec3, spacing: f64, depth: Depth, out: &mut Sample) {
        let p = [d[0] * self.radius, d[1] * self.radius, d[2] * self.radius];
        let h0 = self.h0(d);
        out.height = h0;
        if depth == Depth::L0 {
            return;
        }

        // ---- L1: docs/05 climate, from h0 only ----
        let altitude = h0 - self.sea_surface;
        let up = d;
        let s = up[1] as f32; // sin(latitude)
        let lat_deg = m::asin(s.clamp(-1.0, 1.0)).to_degrees();
        let t_base = 290.0;
        let t_lat = 3.0 * 20.0 * (1.0 / 3.0 - s * s);
        let t_lapse = -0.0065 * altitude.clamp(0.0, 12_000.0);
        let east = normalize([up[2], 0.0, -up[0]]); // tangent, westerlies
        let l_wind = 8000.0 / self.radius;
        let upwind = normalize([d[0] - l_wind * east[0], d[1], d[2] - l_wind * east[2]]);
        let s_up = (h0 - self.h0(upwind)) / 8000.0;
        let d_water = self.water_distance.sample(d);
        let t_coast = 1.0 * m::exp(-d_water / 6000.0);
        let temp = t_base + t_lat + t_lapse + t_coast - 2.0 * (s_up / 0.05).clamp(0.0, 1.0);
        let m_base = 0.15 + 0.25 * gradient3(self.climate.derive("moisture"), scale(p, 30_000.0));
        let gauss = |x: f32, c: f32, sigma: f32| m::exp(-((x - c) / sigma) * ((x - c) / sigma));
        let al = lat_deg.abs();
        let m_lat = 0.25 * gauss(lat_deg, 0.0, 12.0) - 0.2 * gauss(al, 30.0, 10.0)
            + 0.15 * gauss(al, 60.0, 12.0);
        let m_water = 0.3 * m::exp(-d_water / 8000.0);
        let m_oro = 0.2 * (s_up / 0.05).clamp(0.0, 1.0) - 0.2 * (-s_up / 0.05).clamp(0.0, 1.0);
        let cold = ((temp - 240.0) / 33.0).clamp(0.25, 1.0);
        let moisture = ((m_base + m_lat + m_water + m_oro) * cold).clamp(0.0, 1.0);
        out.temp = temp;
        out.moisture = moisture;
        if depth == Depth::L1 {
            return;
        }

        // ---- L2: docs/19 classification ----
        // Slope of h0 by forward differences over 20 m (two more h0 evaluations).
        let step = 20.0 / self.radius;
        let north = normalize([-up[0] * up[1], 1.0 - up[1] * up[1], -up[2] * up[1]]);
        let he = self.h0(normalize([
            d[0] + step * east[0],
            d[1],
            d[2] + step * east[2],
        ]));
        let hn = self.h0(normalize([
            d[0] + step * north[0],
            d[1] + step * north[1],
            d[2] + step * north[2],
        ]));
        let grad = (((he - h0) / 20.0).powi(2) + ((hn - h0) / 20.0).powi(2)).sqrt();
        let slope = m::atan(grad).to_degrees();
        // Value jitter (docs/19 step 2): four noise evaluations.
        let j = |tag: &str| gradient3(self.climate.derive(tag), scale(p, 2000.0));
        let mut axes = Axes {
            temp: temp + 2.0 * j("jitter_temp"),
            moisture: moisture + 0.05 * j("jitter_moisture"),
            altitude: altitude + 60.0 * j("jitter_altitude"),
            slope: slope + 2.0 * j("jitter_slope"),
            substrate: [0.0; 3],
        };
        // Soft substrate from a material region field plus the slope rule.
        let sand = m::smoothstep(-0.2, 0.2, gradient3(self.material, scale(p, 6000.0)));
        let rock = m::smoothstep(25.0, 40.0, axes.slope);
        let sum = (1.0 - sand) + sand + rock;
        axes.substrate = [(1.0 - sand) / sum, sand / sum, rock / sum];
        self.content.classify(p, &axes, &mut out.weights);

        // Material resolve (cheap blend over leaves).
        let mut albedo = [0.0f32; 3];
        for (i, f) in self.content.families.iter().enumerate() {
            let [wb, wv] = out.weights.leaf[i];
            for c in 0..3 {
                albedo[c] +=
                    wb * f.base.albedo[c] + wv * f.variant.as_ref().map_or(0.0, |v| v.albedo[c]);
            }
        }
        if let Some(e) = &self.content.ecotone {
            for c in 0..3 {
                albedo[c] += out.weights.family[MAXF] * e.albedo[c];
            }
        }
        out.albedo = albedo;
        if depth == Depth::L2 {
            return;
        }

        // ---- L3: suites, soft context, band-limited ----
        let mut ctx = [0.0f32; 16];
        let mut ctx_sum = 0.0;
        for (i, s) in self.suites.iter().enumerate() {
            let w = s.slope.score(slope)
                * (out.weights.family[s.families[0]] + out.weights.family[s.families[1]]);
            ctx[i] = w;
            ctx_sum += w;
        }
        let mut h = h0;
        if ctx_sum > 0.0 {
            for (i, s) in self.suites.iter().enumerate() {
                if ctx[i] > 0.0 {
                    h += ctx[i] / ctx_sum * s.amplitude * fbm3(s.seed, p, &s.fbm, spacing);
                }
            }
        }

        // ---- L4: micro detail, near field only ----
        if depth == Depth::L4 {
            h += 0.3 * fbm3(self.micro, p, &MICRO, spacing);
        }

        // Water and shoreline (cheap arithmetic).
        let shore = h - self.sea_surface;
        if shore.abs() < 3.0 {
            h = self.sea_surface + shore * 0.5;
        }
        out.height = h;
    }
}

#[inline]
fn scale(p: [f64; 3], wavelength: f64) -> [f64; 3] {
    [p[0] / wavelength, p[1] / wavelength, p[2] / wavelength]
}

/// Uniform random directions on the sphere, deterministic.
pub fn directions(seed: Seed, n: usize) -> Vec<Vec3> {
    (0..n as u64)
        .map(|i| {
            let z = rng::range_f32(seed, "dir_z", i, -1.0, 1.0) as f64;
            let a = rng::range_f32(seed, "dir_a", i, 0.0, std::f32::consts::TAU) as f64;
            let r = (1.0 - z * z).sqrt();
            [
                r * genesis_core::math::f64::cos(a),
                z,
                r * genesis_core::math::f64::sin(a),
            ]
        })
        .collect()
}
