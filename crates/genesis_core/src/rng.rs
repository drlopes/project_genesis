//! Counter-based random values: `rand(identity, purpose, index) → value`.
//!
//! There is deliberately no stateful generator. A value is a pure function of its key, so
//! chunks can be generated in any order, on any thread, and still agree
//! (docs/11-determinism-and-sampling.md#counter-based-rng-not-streams).

use crate::seed::Seed;

/// A uniform `u64` for `(seed, purpose, index)`.
#[inline]
pub fn u64_at(seed: Seed, purpose: &str, index: u64) -> u64 {
    seed.derive_idx(purpose, index).0
}

/// A uniform `f32` in `[0, 1)`, using the top 24 bits (the full `f32` mantissa).
#[inline]
pub fn unit_f32(seed: Seed, purpose: &str, index: u64) -> f32 {
    (u64_at(seed, purpose, index) >> 40) as f32 * (1.0 / (1u64 << 24) as f32)
}

/// A uniform `f64` in `[0, 1)`, using the top 53 bits (the full `f64` mantissa).
#[inline]
pub fn unit_f64(seed: Seed, purpose: &str, index: u64) -> f64 {
    (u64_at(seed, purpose, index) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// A uniform `f32` in `[lo, hi)`.
#[inline]
pub fn range_f32(seed: Seed, purpose: &str, index: u64, lo: f32, hi: f32) -> f32 {
    lo + (hi - lo) * unit_f32(seed, purpose, index)
}

/// A uniform integer in `[0, n)`, without modulo bias (Lemire's multiply-shift).
/// Panics if `n == 0`.
#[inline]
pub fn below(seed: Seed, purpose: &str, index: u64, n: u64) -> u64 {
    assert!(n > 0, "below(): n must be positive");
    ((u64_at(seed, purpose, index) as u128 * n as u128) >> 64) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_values_are_in_range() {
        let s = Seed(1234);
        for i in 0..10_000 {
            let f = unit_f32(s, "test", i);
            assert!((0.0..1.0).contains(&f));
            let d = unit_f64(s, "test", i);
            assert!((0.0..1.0).contains(&d));
            assert!(below(s, "test", i, 7) < 7);
        }
    }

    #[test]
    fn order_independent() {
        // The defining property: a value does not depend on what was drawn before it.
        let s = Seed(5);
        let forward: Vec<u64> = (0..100).map(|i| u64_at(s, "x", i)).collect();
        let backward: Vec<u64> = (0..100).rev().map(|i| u64_at(s, "x", i)).collect();
        assert_eq!(forward, backward.into_iter().rev().collect::<Vec<_>>());
    }

    #[test]
    fn roughly_uniform() {
        // Coarse sanity check, not a statistical test suite.
        let s = Seed(77);
        let n = 100_000;
        let mut bins = [0u32; 10];
        for i in 0..n {
            bins[(unit_f64(s, "uniform", i) * 10.0) as usize] += 1;
        }
        for b in bins {
            let expected = n as f64 / 10.0;
            assert!(
                ((b as f64) - expected).abs() < expected * 0.05,
                "bins: {bins:?}"
            );
        }
    }
}
