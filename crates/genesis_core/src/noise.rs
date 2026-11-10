//! Seeded, band-limited 3D gradient noise.
//!
//! This is in-house on purpose: noise output defines terrain, so it cannot depend on a
//! third-party crate whose output might change in a minor release
//! (docs/20-tech-stack-and-workspace.md#determinism-rules, rule 2).
//!
//! Positions are body-local metres in `f64`. The lattice cell is computed in `f64` and only the
//! fractional part in `f32`, so fine wavelengths stay precise far from the body centre.

use crate::math::f32::smoothstep;
use crate::seed::{Seed, mix64};

/// Improved-Perlin-style gradient noise at `p` (already divided by the wavelength).
///
/// Output is clamped to `[-1, 1]`; it is exactly `0` at integer lattice points.
pub fn gradient3(seed: Seed, p: [f64; 3]) -> f32 {
    let cell = [p[0].floor(), p[1].floor(), p[2].floor()];
    let f = [
        (p[0] - cell[0]) as f32,
        (p[1] - cell[1]) as f32,
        (p[2] - cell[2]) as f32,
    ];
    let c = [cell[0] as i64, cell[1] as i64, cell[2] as i64];

    let u = [fade(f[0]), fade(f[1]), fade(f[2])];

    let mut corners = [0.0f32; 8];
    for (i, corner) in corners.iter_mut().enumerate() {
        let dx = (i & 1) as i64;
        let dy = ((i >> 1) & 1) as i64;
        let dz = ((i >> 2) & 1) as i64;
        let h = lattice_hash(seed, c[0] + dx, c[1] + dy, c[2] + dz);
        *corner = grad(h, f[0] - dx as f32, f[1] - dy as f32, f[2] - dz as f32);
    }

    let x00 = lerp(corners[0], corners[1], u[0]);
    let x10 = lerp(corners[2], corners[3], u[0]);
    let x01 = lerp(corners[4], corners[5], u[0]);
    let x11 = lerp(corners[6], corners[7], u[0]);
    let y0 = lerp(x00, x10, u[1]);
    let y1 = lerp(x01, x11, u[1]);
    lerp(y0, y1, u[2]).clamp(-1.0, 1.0)
}

/// Parameters for fractal (multi-octave) noise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fbm {
    /// Wavelength of the first (coarsest) octave, in metres.
    pub wavelength: f64,
    /// Number of octaves.
    pub octaves: u32,
    /// Wavelength ratio between octaves (each octave's wavelength is divided by this).
    pub lacunarity: f64,
    /// Amplitude ratio between octaves.
    pub gain: f32,
}

impl Fbm {
    /// Wavelength of octave `i`, in metres.
    pub fn octave_wavelength(&self, i: u32) -> f64 {
        let mut w = self.wavelength;
        for _ in 0..i {
            w /= self.lacunarity;
        }
        w
    }

    /// Sum of all octave amplitudes. Used for normalisation **regardless of band-limiting**, so
    /// fading out fine octaves never rescales the coarse shape (LOD independence, docs/09).
    pub fn amplitude_sum(&self) -> f32 {
        let mut a = 1.0f32;
        let mut sum = 0.0f32;
        for _ in 0..self.octaves {
            sum += a;
            a *= self.gain;
        }
        sum
    }
}

/// Fractal noise at body-local position `p` (metres), band-limited to a sample spacing.
///
/// Each octave is weighted by `smoothstep(0.5, 1.0, wavelength / (2 · spacing))`, the Nyquist fade
/// from docs/09-surface-field-stack.md#band-limiting-and-lod. Octaves that cannot be represented
/// at `spacing` contribute exactly zero and are not evaluated. Pass `spacing = 0.0` for no limit.
///
/// The result is in roughly `[-1, 1]`: normalised by the full amplitude sum.
pub fn fbm3(seed: Seed, p: [f64; 3], params: &Fbm, spacing: f64) -> f32 {
    let mut sum = 0.0f32;
    let mut amplitude = 1.0f32;
    for i in 0..params.octaves {
        let wavelength = params.octave_wavelength(i);
        let weight = nyquist_weight(wavelength, spacing);
        if weight == 0.0 {
            // Every finer octave is also below the cutoff.
            break;
        }
        let octave_seed = seed.derive_idx("octave", u64::from(i));
        let q = [p[0] / wavelength, p[1] / wavelength, p[2] / wavelength];
        sum += amplitude * weight * gradient3(octave_seed, q);
        amplitude *= params.gain;
    }
    sum / params.amplitude_sum()
}

/// Weight of a feature of `wavelength` at sample `spacing` (0 = unresolvable, 1 = fully resolved).
#[inline]
pub fn nyquist_weight(wavelength: f64, spacing: f64) -> f32 {
    if spacing <= 0.0 {
        return 1.0;
    }
    smoothstep(0.5, 1.0, (wavelength / (2.0 * spacing)) as f32)
}

#[inline]
fn fade(t: f32) -> f32 {
    // 6t⁵ − 15t⁴ + 10t³: C² continuous, so normals derived from noise are smooth.
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
fn lattice_hash(seed: Seed, x: i64, y: i64, z: i64) -> u64 {
    let mut h = seed.0;
    h = mix64(h ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    h = mix64(h ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F));
    mix64(h ^ (z as u64).wrapping_mul(0x1656_67B1_9E37_79F9))
}

/// Dot product with one of the 12 cube-edge gradients (Perlin's improved noise).
#[inline]
fn grad(h: u64, x: f32, y: f32, z: f32) -> f32 {
    match (h >> 60) % 12 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x + z,
        5 => -x + z,
        6 => x - z,
        7 => -x - z,
        8 => y + z,
        9 => -y + z,
        10 => y - z,
        _ => -y - z,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Seed = Seed(0xDEAD_BEEF);

    fn params() -> Fbm {
        Fbm {
            wavelength: 2000.0,
            octaves: 8,
            lacunarity: 2.0,
            gain: 0.5,
        }
    }

    #[test]
    fn zero_on_lattice_points() {
        for p in [[0.0, 0.0, 0.0], [3.0, -7.0, 12.0], [100_000.0, 2.0, -5.0]] {
            assert_eq!(gradient3(S, p), 0.0);
        }
    }

    #[test]
    fn deterministic_and_seed_sensitive() {
        let p = [12.3, -4.56, 7.89];
        assert_eq!(gradient3(S, p), gradient3(S, p));
        assert_ne!(gradient3(S, p), gradient3(Seed(1), p));
    }

    #[test]
    fn in_range_and_not_flat() {
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for i in 0..20_000u64 {
            let t = i as f64 * 0.137;
            let v = gradient3(S, [t, t * 0.71 + 3.1, t * 0.33 - 9.2]);
            assert!((-1.0..=1.0).contains(&v));
            min = min.min(v);
            max = max.max(v);
        }
        assert!(min < -0.5 && max > 0.5, "range too narrow: {min}..{max}");
    }

    #[test]
    fn continuous() {
        // Lipschitz-style check: a tiny step gives a tiny change, including across cell edges.
        let eps = 1e-4;
        for i in 0..5_000u64 {
            let x = i as f64 * 0.01;
            let a = gradient3(S, [x, 0.5, 0.25]);
            let b = gradient3(S, [x + eps, 0.5, 0.25]);
            assert!((a - b).abs() < 1e-2, "jump at x={x}: {a} -> {b}");
        }
    }

    #[test]
    fn precise_far_from_origin() {
        // 24 km from the centre at a 0.1 m wavelength: neighbouring samples 1 cm apart must still
        // differ smoothly, which f32 lattice coordinates (~1.6 cm resolution there) cannot do.
        let p0 = 24_000.0 / 0.1;
        let a = gradient3(S, [p0 + 0.05, 0.3, 0.7]);
        let b = gradient3(S, [p0 + 0.15, 0.3, 0.7]);
        assert_ne!(a, b);
        assert!((a - b).abs() < 0.5);
    }

    #[test]
    fn band_limit_removes_fine_octaves_and_keeps_scale() {
        let f = params();
        let p = [1234.5, 6789.0, -2468.0];
        // At 300 m spacing the 2000 m and 1000 m octaves are fully resolved
        // (ratio ≥ 1), 500 m is partly faded (ratio 0.83 → weight ≈ 0.74), finer octaves are 0.
        let coarse = fbm3(S, p, &f, 300.0);
        let manual = {
            let mut sum = 0.0;
            let mut a = 1.0;
            for i in 0..3 {
                let w = f.octave_wavelength(i);
                let q = [p[0] / w, p[1] / w, p[2] / w];
                sum += a
                    * nyquist_weight(w, 300.0)
                    * gradient3(S.derive_idx("octave", u64::from(i)), q);
                a *= f.gain;
            }
            sum / f.amplitude_sum()
        };
        assert_eq!(coarse, manual);
        // Normalisation does not depend on spacing: the coarse sum is not rescaled up.
        assert!(coarse.abs() <= 1.0);
    }

    #[test]
    fn unlimited_equals_fine_spacing() {
        let f = params();
        let p = [10.0, 20.0, 30.0];
        // A spacing far below the finest octave changes nothing.
        assert_eq!(fbm3(S, p, &f, 0.0), fbm3(S, p, &f, 1e-6));
    }

    /// Pins noise output bit-for-bit. A change here changes every world's terrain.
    #[test]
    fn known_answers() {
        let f = params();
        let got = [
            gradient3(S, [0.5, 0.25, 0.125]).to_bits(),
            gradient3(S, [-12.7, 3.3, 99.9]).to_bits(),
            fbm3(S, [1234.5, 6789.0, -2468.0], &f, 0.0).to_bits(),
            fbm3(S, [1234.5, 6789.0, -2468.0], &f, 300.0).to_bits(),
        ];
        assert_eq!(got, KNOWN);
    }

    const KNOWN: [u32; 4] = [3_110_830_592, 3_186_118_018, 1_043_219_736, 1_043_421_937];
}
