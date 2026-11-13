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

/// A uniform integer in `[0, n)`, exactly unbiased: Lemire's multiply-shift with its rejection
/// step (Lemire 2019, "Fast Random Integer Generation in an Interval").
///
/// The high 64 bits of `x · n` are the result. Values of `x` whose low 64 bits fall below
/// `2^64 mod n` would make some results more likely than others, so they are rejected and redrawn.
/// A redraw comes from a key derived from the rejected value and the attempt number, so the result
/// is still a pure function of `(seed, purpose, index)`. Rejection happens with probability below
/// `n / 2^64`: for any practical `n` the first draw is used. Panics if `n == 0`.
#[inline]
pub fn below(seed: Seed, purpose: &str, index: u64, n: u64) -> u64 {
    assert!(n > 0, "below(): n must be positive");
    let mut x = u64_at(seed, purpose, index);
    let mut m = x as u128 * n as u128;
    if (m as u64) < n {
        // 2^64 mod n, computed in 64 bits: (2^64 − n) mod n.
        let threshold = n.wrapping_neg() % n;
        let mut attempt = 0u64;
        while (m as u64) < threshold {
            attempt += 1;
            x = Seed(x).derive_idx("below_retry", attempt).0;
            m = x as u128 * n as u128;
        }
    }
    (m >> 64) as u64
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
    fn below_rejection_path_is_deterministic_and_in_range() {
        // n just above 2^63: the threshold is 2^63 − 1, so about half of all first draws are
        // rejected, and the redraw path is exercised on every run.
        let n = (1u64 << 63) + 1;
        let threshold = n.wrapping_neg() % n;
        assert_eq!(threshold, (1u64 << 63) - 1);
        let s = Seed(42);
        let mut rejected = 0;
        for i in 0..2_000 {
            let first = u64_at(s, "big", i);
            if ((first as u128 * n as u128) as u64) < threshold {
                rejected += 1;
            }
            let v = below(s, "big", i, n);
            assert!(v < n);
            assert_eq!(v, below(s, "big", i, n), "below() must be a pure function");
        }
        assert!(rejected > 800, "expected ~half rejected, got {rejected}");
    }

    #[test]
    fn below_is_exactly_uniform_over_a_full_cycle() {
        // With n = 3 · 2^62 the plain multiply-shift is v = ⌊3x / 4⌋: every 4 inputs map onto 3
        // outputs, and the outputs with v ≡ 0 (mod 3) get two inputs each (x = 4j and 4j + 1).
        // Rejection removes exactly x ≡ 0 (mod 4) (threshold 2^64 mod n = 2^62), so the three
        // residue classes must come out equally often.
        let n = 3u64 << 62;
        let s = Seed(7);
        let mut by_residue = [0u32; 3];
        for i in 0..60_000 {
            by_residue[(below(s, "thirds", i, n) % 3) as usize] += 1;
        }
        let ratio = by_residue[0] as f64 / by_residue[1] as f64;
        assert!(
            (0.95..1.05).contains(&ratio),
            "residue counts {by_residue:?}: ratio {ratio:.3} (biased would be ~2)"
        );
        // The test is sensitive: the plain multiply-shift on the same keys shows the 2:1 bias.
        let mut plain = [0u32; 3];
        for i in 0..60_000 {
            let v = ((u64_at(s, "thirds", i) as u128 * n as u128) >> 64) as u64;
            plain[(v % 3) as usize] += 1;
        }
        let plain_ratio = plain[0] as f64 / plain[1] as f64;
        assert!(
            (1.9..2.1).contains(&plain_ratio),
            "plain multiply-shift should be biased: {plain:?}"
        );
    }

    #[test]
    fn below_known_answers() {
        // Pins `below` for ordinary n. These keys never hit the rejection path, so the values
        // equal the plain multiply-shift; a change here changes every world that uses `below`.
        let s = Seed(1234);
        let got: Vec<u64> = (0..6).map(|i| below(s, "kat", i, 1000)).collect();
        assert_eq!(got, KAT_BELOW_1000);
        let plain: Vec<u64> = (0..6)
            .map(|i| ((u64_at(s, "kat", i) as u128 * 1000) >> 64) as u64)
            .collect();
        assert_eq!(got, plain, "ordinary n must not reach the rejection path");
    }

    const KAT_BELOW_1000: [u64; 6] = [519, 275, 221, 94, 75, 814];

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
