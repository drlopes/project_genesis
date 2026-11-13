//! The docs/19 classifier, implemented faithfully for cost (aerial stratum only).
//!
//! Allocation-free: fixed arrays, so the benchmark measures arithmetic, not the allocator.

use genesis_core::Seed;
use genesis_core::math::f32 as m;
use genesis_core::noise::gradient3;
use genesis_core::rng;

pub const MAXF: usize = 40;
pub const SUBSTRATES: usize = 3; // soil, sand, rock

#[derive(Clone, Copy)]
pub struct Range {
    pub lo: f32,
    pub hi: f32,
    pub falloff: f32,
}

impl Range {
    pub const ANY: Range = Range {
        lo: f32::MIN,
        hi: f32::MAX,
        falloff: 0.0,
    };

    #[inline]
    pub fn score(&self, x: f32) -> f32 {
        let d = if x < self.lo {
            self.lo - x
        } else if x > self.hi {
            x - self.hi
        } else {
            return 1.0;
        };
        1.0 - m::smoothstep(0.0, self.falloff, d)
    }
}

#[derive(Clone, Copy)]
pub struct Biome {
    pub temp: Range,
    pub moisture: Range,
    pub altitude: Range,
    pub slope: Range,
    pub substrates: [bool; SUBSTRATES],
    pub weight: f32,
    pub albedo: [f32; 3],
}

impl Biome {
    #[inline]
    pub fn suitability(&self, x: &Axes) -> f32 {
        let mut s = self.temp.score(x.temp);
        if s == 0.0 {
            return 0.0;
        }
        s *= self.moisture.score(x.moisture);
        if s == 0.0 {
            return 0.0;
        }
        s *= self.altitude.score(x.altitude) * self.slope.score(x.slope);
        if s == 0.0 {
            return 0.0;
        }
        let mut membership = 0.0;
        for (i, &on) in self.substrates.iter().enumerate() {
            if on {
                membership += x.substrate[i];
            }
        }
        s * membership
    }
}

pub struct Family {
    pub base: Biome,
    pub variant: Option<Biome>,
    pub palette_weight: f32,
    pub mosaic_seed: Seed,
    pub mosaic_strength: f32,
    pub mosaic_scale: f64,
}

pub struct Ecotone {
    pub a: usize,
    pub b: usize,
    pub strength: f32,
    pub albedo: [f32; 3],
}

pub struct Content {
    pub families: Vec<Family>,
    pub fallback: usize,
    pub ecotone: Option<Ecotone>,
    pub k: u32,
    pub budget: usize,
    pub eps: f32,
}

#[derive(Clone, Copy, Default)]
pub struct Axes {
    pub temp: f32,
    pub moisture: f32,
    pub altitude: f32,
    pub slope: f32,
    pub substrate: [f32; SUBSTRATES],
}

/// Weights per family slot (the last slot is the ecotone), split into base/variant leaves.
pub struct Weights {
    pub family: [f32; MAXF + 1],
    pub leaf: [[f32; 2]; MAXF + 1],
    pub n: usize,
}

#[inline]
fn ipow(x: f32, k: u32) -> f32 {
    let mut r = 1.0;
    for _ in 0..k {
        r *= x;
    }
    r
}

impl Content {
    /// A stub palette of `n` aerial families spread over climate space, one variant, one
    /// ecotone, and a barren fallback, as in docs/19's worked example.
    pub fn stub(seed: Seed, n: usize, k: u32, budget: usize) -> Content {
        assert!((2..=MAXF).contains(&n));
        let content_seed = seed.derive("content");
        let mut families = Vec::with_capacity(n);
        // Family 0: barren fallback (cold or very dry).
        families.push(Family {
            base: Biome {
                temp: Range {
                    lo: 150.0,
                    hi: 235.0,
                    falloff: 10.0,
                },
                moisture: Range::ANY,
                altitude: Range::ANY,
                slope: Range::ANY,
                substrates: [true, true, true],
                weight: 1.0,
                albedo: [0.5, 0.48, 0.45],
            },
            variant: None,
            palette_weight: 1.0,
            mosaic_seed: content_seed.derive_named("mosaic", "barren"),
            mosaic_strength: 0.0,
            mosaic_scale: 1500.0,
        });
        for i in 1..n {
            let t = i as f32 / n as f32;
            let tc = 240.0 + 70.0 * t;
            let mc = rng::unit_f32(content_seed, "stub_moisture", i as u64);
            let base = Biome {
                temp: Range {
                    lo: tc - 12.0,
                    hi: tc + 12.0,
                    falloff: 8.0,
                },
                moisture: Range {
                    lo: mc - 0.25,
                    hi: mc + 0.25,
                    falloff: 0.12,
                },
                altitude: Range {
                    lo: -50.0,
                    hi: 1800.0,
                    falloff: 400.0,
                },
                slope: Range {
                    lo: 0.0,
                    hi: 35.0,
                    falloff: 8.0,
                },
                substrates: [true, i % 3 == 0, i % 4 == 0],
                weight: 1.0,
                albedo: [0.2 + 0.5 * t, 0.4, 0.3 - 0.2 * t],
            };
            let variant = (i == 1).then(|| Biome {
                altitude: Range {
                    lo: 1200.0,
                    hi: 3000.0,
                    falloff: 300.0,
                },
                temp: Range {
                    lo: tc - 25.0,
                    hi: tc,
                    falloff: 4.0,
                },
                ..base
            });
            families.push(Family {
                base,
                variant,
                palette_weight: 0.5 + rng::unit_f32(content_seed, "palette", i as u64),
                mosaic_seed: content_seed.derive_idx("mosaic", i as u64),
                mosaic_strength: 0.35,
                mosaic_scale: 1500.0,
            });
        }
        Content {
            families,
            fallback: 0,
            ecotone: (n > 2).then_some(Ecotone {
                a: 1,
                b: 2,
                strength: 0.6,
                albedo: [0.3, 0.35, 0.2],
            }),
            k,
            budget,
            eps: 1e-3,
        }
    }

    /// Set the mosaic strength of every family that has one (the fallback keeps none).
    pub fn set_mosaic_strength(&mut self, strength: f32) {
        for f in &mut self.families {
            if f.mosaic_strength > 0.0 {
                f.mosaic_strength = strength;
            }
        }
    }

    pub fn classify(&self, p: [f64; 3], x: &Axes, out: &mut Weights) {
        let n = self.families.len();
        let mut v = [0.0f32; MAXF];
        let mut s_member = [[0.0f32; 2]; MAXF];

        // Steps 5–6: family envelope × prior (mosaic only evaluated where S > 0).
        for (i, f) in self.families.iter().enumerate() {
            let sb = f.base.suitability(x);
            let sv = f.variant.as_ref().map_or(0.0, |b| b.suitability(x));
            s_member[i] = [sb, sv];
            let s = sb.max(sv);
            if s > 0.0 {
                let mosaic = if f.mosaic_strength > 0.0 {
                    let q = [
                        p[0] / f.mosaic_scale,
                        p[1] / f.mosaic_scale,
                        p[2] / f.mosaic_scale,
                    ];
                    m::exp(f.mosaic_strength * gradient3(f.mosaic_seed, q))
                } else {
                    1.0
                };
                v[i] = f.base.weight * f.palette_weight * mosaic * s;
            }
        }

        // Step 7: soft budget — subtract the (K+1)-th largest value.
        let nonzero = v[..n].iter().filter(|&&x| x > 0.0).count();
        if nonzero > self.budget {
            let mut sorted = v;
            sorted[..n].sort_unstable_by(|a, b| b.total_cmp(a));
            let tau = sorted[self.budget];
            for x in &mut v[..n] {
                *x = (*x - tau).max(0.0);
            }
        }

        // Step 8: ε fallback. Step 9: power normalisation.
        v[self.fallback] += self.eps;
        let mut sum = 0.0;
        for i in 0..n {
            let p = ipow(v[i], self.k);
            out.family[i] = p;
            sum += p;
        }
        for i in 0..n {
            out.family[i] /= sum;
        }

        // Step 10: ecotone injection into the extra slot.
        out.family[MAXF] = 0.0;
        if let Some(e) = &self.ecotone {
            let inj = e.strength * 4.0 * out.family[e.a] * out.family[e.b];
            if inj > 0.0 {
                out.family[MAXF] = inj;
                let norm = 1.0 + inj;
                for i in 0..n {
                    out.family[i] /= norm;
                }
                out.family[MAXF] /= norm;
            }
        }

        // Step 11: split each family among base and variant.
        for i in 0..n {
            let w = out.family[i];
            if w == 0.0 {
                out.leaf[i] = [0.0, 0.0];
                continue;
            }
            let ub = ipow(s_member[i][0], self.k);
            let uv = ipow(s_member[i][1], self.k);
            let t = ub + uv;
            out.leaf[i] = if t > 0.0 {
                [w * ub / t, w * uv / t]
            } else {
                [w, 0.0]
            };
        }
        out.leaf[MAXF] = [out.family[MAXF], 0.0];
        out.n = n;
    }
}
