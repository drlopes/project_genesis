//! Golden descriptor hashes (docs/11 "Golden seeds", docs/20 determinism rule 6).
//!
//! The E2 seed set, each as body 0 of region 0, system 0. CI runs this on Windows and Linux, in
//! debug and release. **If this fails, you changed what these worlds are.** Update the constants
//! only on purpose, together with a `GENERATOR_VERSION` bump, and say so in the commit.

use genesis_gen::descriptor::GENERATOR_VERSION;
use genesis_gen::{RockyParams, SeedChain, bake_core};

// Re-pinned in generator version 2 (P1-C1: energy, atmosphere, water state, climate, biosphere).
const GOLDEN: [(u64, u64); 10] = [
    (1, 0x2DBB_34A4_9676_4BD6),
    (2, 0x0E94_7315_1CB3_EEA1),
    (3, 0xE3A5_9739_7F33_AE31),
    (42, 0xA701_0B1C_50F0_90C2),
    (1337, 0xA11F_E7C4_804C_31A0),
    (0xDEAD_BEEF, 0xEDD3_7E36_FAAF_3E4F),
    (0x5EED_0001, 0xF4CF_623C_628A_D9D1),
    (7_777_777, 0xE6A4_18ED_787D_09C5),
    (123_456_789, 0xB6C9_4418_1D30_E123),
    (u64::MAX, 0xE585_354C_2626_2154),
];

#[test]
fn golden_descriptor_hashes() {
    assert_eq!(
        GENERATOR_VERSION, 2,
        "generator version changed: re-pin the golden hashes"
    );
    let p = RockyParams::default();
    let got: Vec<(u64, u64)> = GOLDEN
        .iter()
        .map(|&(seed, _)| {
            (
                seed,
                bake_core(SeedChain::body(seed, 0), &p).unwrap().hash(),
            )
        })
        .collect();
    for (seed, hash) in &got {
        eprintln!("({seed:#X}, {hash:#018X}),");
    }
    assert_eq!(got, GOLDEN);
}
