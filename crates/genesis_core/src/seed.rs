//! The seed chain: `H(parent, purpose tag, indices…)`.
//!
//! Every random decision in the game is a pure function of an identity and a purpose
//! (docs/11-determinism-and-sampling.md#the-seed-chain). There is no stateful RNG stream.
//!
//! **The exact bit-level behaviour of this module is part of world identity.** Changing the
//! tag hash, the mixer, or the combination order changes every world ever generated. The
//! known-answer tests at the bottom of this file pin it; if they fail, you have changed the
//! universe.

/// A 64-bit seed. Seeds are identities, not streams: deriving never mutates anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seed(pub u64);

impl Seed {
    /// `H(self, tag)` — derive a subsystem seed, e.g. `body.derive("geology")`.
    #[inline]
    pub fn derive(self, tag: &str) -> Seed {
        debug_assert!(is_valid_tag(tag), "invalid purpose tag: {tag:?}");
        Seed(mix64(self.0 ^ tag_hash(tag)))
    }

    /// `H(self, tag, index)` — derive a seed for a list entry, e.g. `geology.derive_idx("plate", i)`.
    #[inline]
    pub fn derive_idx(self, tag: &str, index: u64) -> Seed {
        self.derive_path(tag, &[index])
    }

    /// `H(self, tag, i0, i1, …)` — derive a seed keyed by several integers,
    /// e.g. `body.derive_path("chunk", &[face, level, x, y])`.
    ///
    /// Index order matters: `[1, 2]` and `[2, 1]` give different seeds.
    #[inline]
    pub fn derive_path(self, tag: &str, indices: &[u64]) -> Seed {
        let mut h = self.derive(tag).0;
        for &i in indices {
            h = mix64(h.wrapping_add(GOLDEN_GAMMA) ^ mix64(i));
        }
        Seed(h)
    }

    /// `H(self, tag, content ID)` — derive a seed keyed by a content ID, so adding content
    /// never moves an existing draw (docs/11-determinism-and-sampling.md#feature-level-seeds).
    #[inline]
    pub fn derive_named(self, tag: &str, name: &str) -> Seed {
        let h = self.derive(tag).0;
        Seed(mix64(
            h.wrapping_add(GOLDEN_GAMMA) ^ fnv1a64(name.as_bytes()),
        ))
    }
}

/// The SplitMix64 increment (2^64 / φ, odd). Separates successive combination steps.
const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

/// SplitMix64's finaliser: a bijective, well-avalanched 64-bit mixer.
#[inline]
pub const fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// FNV-1a 64-bit over bytes. Stable, trivial, and good enough as input to `mix64`.
#[inline]
pub const fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    let mut i = 0;
    while i < bytes.len() {
        h ^= bytes[i] as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
        i += 1;
    }
    h
}

/// Purpose tags are mixed before use so similar tags give unrelated seeds.
#[inline]
const fn tag_hash(tag: &str) -> u64 {
    mix64(fnv1a64(tag.as_bytes()))
}

/// Tags are lowercase ASCII letters, digits, and underscores
/// (docs/11-determinism-and-sampling.md#tag-rules, rule 1).
pub fn is_valid_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_validated() {
        assert!(is_valid_tag("geology"));
        assert!(is_valid_tag("chunk_2"));
        assert!(!is_valid_tag(""));
        assert!(!is_valid_tag("Geology"));
        assert!(!is_valid_tag("geo-logy"));
    }

    #[test]
    fn derivation_is_pure_and_tag_sensitive() {
        let body = Seed(42);
        assert_eq!(body.derive("geology"), body.derive("geology"));
        assert_ne!(body.derive("geology"), body.derive("climate"));
        assert_ne!(Seed(42).derive("geology"), Seed(43).derive("geology"));
    }

    #[test]
    fn indices_are_ordered_and_distinct() {
        let s = Seed(7);
        assert_ne!(
            s.derive_path("chunk", &[1, 2]),
            s.derive_path("chunk", &[2, 1])
        );
        assert_ne!(s.derive_idx("plate", 0), s.derive_idx("plate", 1));
        // An index is not the same as no index.
        assert_ne!(s.derive_path("plate", &[]), s.derive_idx("plate", 0));
        assert_eq!(s.derive_path("plate", &[]), s.derive("plate"));
    }

    #[test]
    fn named_derivation_is_keyed_by_name() {
        let content = Seed(99).derive("content");
        let a = content.derive_named("mosaic", "core:biome/temperate_forest");
        let b = content.derive_named("mosaic", "core:biome/temperate_grassland");
        assert_ne!(a, b);
        assert_eq!(
            a,
            content.derive_named("mosaic", "core:biome/temperate_forest")
        );
    }

    /// Pins the seed chain bit-for-bit. These values define every world. If this test fails,
    /// the change is a universe-breaking `generatorVersion` change, not a test to update.
    #[test]
    fn known_answers() {
        let b = Seed(0x1234_5678_9ABC_DEF0);
        assert_eq!(b.derive("geology").0, 0x70F0_DB6D_DECF_425E);
        assert_eq!(b.derive_idx("plate", 3).0, 0xAB8F_19F1_B0D1_2B27);
        assert_eq!(
            b.derive_path("chunk", &[2, 8, 3, 5]).0,
            0x5D4E_FC3C_1300_C743
        );
        assert_eq!(
            b.derive("content")
                .derive_named("mosaic", "core:biome/temperate_forest")
                .0,
            0xCB2D_ECF8_DA95_B317
        );
    }

    #[test]
    fn fnv_reference_vectors() {
        // Published FNV-1a 64 test vectors.
        assert_eq!(fnv1a64(b""), 0xCBF2_9CE4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xAF63_DC4C_8601_EC8C);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_F739_67E8);
    }

    #[test]
    fn splitmix_reference_vector() {
        // First output of SplitMix64 seeded with 0 is mix64(0 + GOLDEN_GAMMA).
        assert_eq!(mix64(GOLDEN_GAMMA), 0xE220_A839_7B1D_CDAF);
    }
}
