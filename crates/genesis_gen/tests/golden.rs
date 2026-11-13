//! Golden descriptor hashes (docs/11 "Golden seeds", docs/20 determinism rule 6).
//!
//! The E2 seed set, each as body 0 of region 0, system 0. CI runs this on Windows and Linux, in
//! debug and release. **If this fails, you changed what these worlds are.** Update the constants
//! only on purpose, together with a `GENERATOR_VERSION` bump, and say so in the commit.

use genesis_gen::descriptor::GENERATOR_VERSION;
use genesis_gen::{RockyParams, SeedChain, bake_core};

const GOLDEN: [(u64, u64); 10] = [
    (1, 0x3F4F_0913_B700_11C8),
    (2, 0x89F5_FFBD_A77E_F8E0),
    (3, 0x3E30_7FB6_1101_880A),
    (42, 0xFD13_6CCA_6F08_BF79),
    (1337, 0x9C78_1FC5_2E39_60A8),
    (0xDEAD_BEEF, 0x86F6_2043_4A91_66F1),
    (0x5EED_0001, 0x73B1_59E6_D301_6060),
    (7_777_777, 0xD1BE_4CB3_8DEC_C966),
    (123_456_789, 0x70A3_A5C2_3F81_A8E2),
    (u64::MAX, 0x12AA_22E7_8A35_1641),
];

#[test]
fn golden_descriptor_hashes() {
    assert_eq!(
        GENERATOR_VERSION, 1,
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
