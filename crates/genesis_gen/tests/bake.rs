//! Bake contract tests (docs/13 "Bake tests"): determinism, order independence, required and
//! explicitly absent blocks, cache reproducibility, size budget; plus the sea-level solve and the
//! continuity of L0 (no walls: the B-005 lesson).

use genesis_core::math::f64::{cos, sin};
use genesis_gen::bake::{area_below, h0_cached, relief};
use genesis_gen::cube::normalize;
use genesis_gen::{RockyParams, SeedChain, bake_caches, bake_core};

const SEEDS: [u64; 4] = [1, 42, 1337, 0xDEAD_BEEF];

#[test]
fn bake_is_deterministic_and_order_independent() {
    let p = RockyParams::default();
    // Same chain twice, and interleaved with other bodies: identical bytes.
    let a = bake_core(SeedChain::body(42, 3), &p).unwrap().encode();
    let _ = bake_core(SeedChain::body(7, 0), &p).unwrap();
    let b = bake_core(SeedChain::body(42, 3), &p).unwrap().encode();
    assert_eq!(a, b);
    // On other threads, in reverse order.
    let handles: Vec<_> = (0..4u64)
        .rev()
        .map(|i| {
            std::thread::spawn(move || {
                (
                    i,
                    bake_core(SeedChain::body(42, i), &RockyParams::default())
                        .unwrap()
                        .hash(),
                )
            })
        })
        .collect();
    for h in handles {
        let (i, hash) = h.join().unwrap();
        assert_eq!(hash, bake_core(SeedChain::body(42, i), &p).unwrap().hash());
    }
}

#[test]
fn siblings_differ() {
    let p = RockyParams::default();
    let h: Vec<u64> = (0..4)
        .map(|b| bake_core(SeedChain::body(42, b), &p).unwrap().hash())
        .collect();
    for i in 0..h.len() {
        for j in (i + 1)..h.len() {
            assert_ne!(h[i], h[j]);
        }
    }
}

#[test]
fn absent_blocks_are_explicit_nulls() {
    let core = bake_core(SeedChain::body(1, 0), &RockyParams::default()).unwrap();
    let v = serde_json::to_value(&core).unwrap();
    for key in [
        "orbit",
        "energy",
        "atmosphere",
        "climate",
        "regions",
        "biomePalette",
        "landformSuites",
        "materials",
        "biosphere",
        "aquatic",
        "narrative",
        "contentVersion",
    ] {
        assert!(
            v.get(key).is_some_and(|x| x.is_null()),
            "{key} must be present and null"
        );
    }
    for key in [
        "bulk",
        "rotation",
        "hydrosphere",
        "geology",
        "id",
        "seedChain",
    ] {
        assert!(
            v.get(key).is_some_and(|x| !x.is_null()),
            "{key} must be baked"
        );
    }
    assert!(v["hydrosphere"]["lakes"].is_null());
    assert!(v["geology"]["erosion"].is_null());
}

#[test]
fn core_stays_in_the_kilobyte_range() {
    // docs/13 bake test 6.
    for s in SEEDS {
        let n = bake_core(SeedChain::body(s, 0), &RockyParams::default())
            .unwrap()
            .encode()
            .len();
        assert!(n < 64 * 1024, "core is {n} bytes");
    }
}

#[test]
fn sea_level_puts_the_ocean_fraction_below_it() {
    let p = RockyParams::default();
    for s in SEEDS {
        let core = bake_core(SeedChain::body(s, 0), &p).unwrap();
        let Some(sea) = core.hydrosphere.sea_radius else {
            continue;
        };
        // Checked on an independent, finer grid than the bake used.
        let r = relief(&core, &p);
        let below = area_below(&r, sea - core.bulk.radius, 96);
        let want = core.hydrosphere.ocean_fraction;
        eprintln!("sea: seed {s} below {below:.4} want {want:.4}");
        assert!(
            (below - want).abs() < 0.02,
            "seed {s}: {below:.4} of the area below the sea, wanted {want:.4}"
        );
    }
}

#[test]
fn caches_are_reproducible_and_the_crater_index_is_exact() {
    let p = RockyParams::default();
    let core = bake_core(SeedChain::body(42, 0), &p).unwrap();
    let a = bake_caches(&core, &p);
    let b = bake_caches(&core, &p);
    assert_eq!(a.pyramid, b.pyramid);
    assert_eq!(a.crater_index.cells, b.crater_index.cells);
    // The index is an exact optimisation (docs/19's rule for candidate filtering applies here
    // too): the indexed crater sum equals the full sum bit for bit.
    let r = relief(&core, &p);
    for d in sphere_points(20_000) {
        let full = r.crater_delta(d, 0..core.geology.craters.len());
        let listed = r.crater_delta_listed(d, a.crater_index.at(d));
        assert_eq!(full.to_bits(), listed.to_bits());
    }
}

#[test]
fn cached_h0_tracks_the_analytic_field() {
    // The pyramid is a cache of macroHeight; the query (pyramid + analytic deltas) must stay
    // close to the field the bake evaluated. Tolerance: 1 % of the relief range (measured
    // 2.4–4.8 m p99 against 2–3 km ranges at pyramid 129).
    let p = RockyParams::default();
    for s in SEEDS {
        let core = bake_core(SeedChain::body(s, 0), &p).unwrap();
        let caches = bake_caches(&core, &p);
        let r = relief(&core, &p);
        let spacing = caches.pyramid.spacing(core.bulk.radius);
        let mut errs: Vec<f64> = sphere_points(5_000)
            .into_iter()
            .map(|d| (h0_cached(&r, &caches, d) - r.h0(d, spacing)).abs())
            .collect();
        errs.sort_by(f64::total_cmp);
        let p99 = errs[errs.len() * 99 / 100];
        let q = &core.geology.hypsometry.quantiles;
        let range = q[20] - q[0];
        eprintln!(
            "cache: seed {s} p99 {p99:.1} m, max {:.1} m, range {range:.0} m",
            errs[errs.len() - 1]
        );
        assert!(
            p99 < 0.01 * range,
            "seed {s}: p99 |cached − analytic| {p99:.1} m vs relief range {range:.0} m"
        );
    }
}

#[test]
fn l0_has_no_walls() {
    // Walk great circles in 10 m steps. A continuous field changes by at most (max slope × 10 m);
    // a discontinuity, as at a crater rim before B-005 or at a triple junction with a "nearest
    // boundary" rule, jumps by tens to hundreds of metres in one step.
    let p = RockyParams::default();
    for s in [42u64, 1337] {
        let core = bake_core(SeedChain::body(s, 0), &p).unwrap();
        let r = relief(&core, &p);
        let radius = core.bulk.radius;
        let step = 10.0 / radius;
        let mut worst: f64 = 0.0;
        for k in 0..6 {
            let axis = sphere_points(6)[k];
            // Two unit vectors orthogonal to the axis span the great circle.
            let a = normalize(if axis[0].abs() < 0.9 {
                [0.0, -axis[2], axis[1]]
            } else {
                [-axis[1], axis[0], 0.0]
            });
            let b = genesis_gen::cube::cross(axis, a);
            let at = |t: f64| {
                normalize([
                    a[0] * cos(t) + b[0] * sin(t),
                    a[1] * cos(t) + b[1] * sin(t),
                    a[2] * cos(t) + b[2] * sin(t),
                ])
            };
            let steps = (std::f64::consts::TAU / step) as usize;
            let mut prev = r.h0(at(0.0), 0.0);
            for i in 1..=steps {
                let h = r.h0(at(i as f64 * step), 0.0);
                worst = worst.max((h - prev).abs());
                prev = h;
            }
        }
        eprintln!("walls: seed {s} worst 10 m step {worst:.2} m");
        assert!(worst < 25.0, "seed {s}: largest 10 m step is {worst:.1} m");
    }
}

#[test]
fn invalid_params_are_rejected() {
    let p = RockyParams {
        plates: (1, 40),
        ..RockyParams::default()
    };
    assert!(bake_core(SeedChain::body(1, 0), &p).is_err());
}

/// Deterministic points spread over the sphere (golden-angle spiral).
fn sphere_points(n: usize) -> Vec<[f64; 3]> {
    let ga = std::f64::consts::PI * (3.0 - 5.0f64.sqrt());
    (0..n)
        .map(|i| {
            let y = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
            let r = (1.0 - y * y).sqrt();
            let t = ga * i as f64;
            [r * cos(t), y, r * sin(t)]
        })
        .collect()
}
