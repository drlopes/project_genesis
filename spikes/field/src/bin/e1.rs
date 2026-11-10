//! E1 — field cost (docs/21-phase-1-spike-plan.md#e1--field-cost).
//! Run: cargo run --release --bin e1   (from spikes/)

use genesis_core::Seed;
use spike_field::{BAND_SPACING, Body, Depth, Sample, directions};
use std::hint::black_box;
use std::time::Instant;

const N: usize = 50_000;
const RUNS: usize = 5;
const PATCH: f64 = 65.0 * 65.0;

fn time_ns(dirs: &[[f64; 3]], mut f: impl FnMut(&[f64; 3])) -> f64 {
    for d in &dirs[..2000] {
        f(d); // warm-up
    }
    let mut runs: Vec<f64> = (0..RUNS)
        .map(|_| {
            let t = Instant::now();
            for d in dirs {
                f(d);
            }
            t.elapsed().as_nanos() as f64 / dirs.len() as f64
        })
        .collect();
    runs.sort_by(f64::total_cmp);
    runs[RUNS / 2]
}

fn sample_cost(body: &Body, dirs: &[[f64; 3]], band: usize, depth: Depth) -> f64 {
    let mut s = Sample::default();
    time_ns(dirs, |d| {
        body.sample(black_box(*d), BAND_SPACING[band], depth, &mut s);
        black_box(&s);
    })
}

fn main() {
    let seed = Seed(0x5EED_0001);
    let dirs = directions(seed, N);

    println!("# E1 raw results\n");
    println!("Samples per run: {N}, runs: {RUNS} (median), single thread, release build.\n");

    // Bake cost and the default body.
    let t = Instant::now();
    let body = Body::bake(seed, 16, 4, 4, 4, 256);
    let bake_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("## Bake\n");
    println!("| What | Value |\n|---|---|");
    println!("| Macro pyramid 6×256×256 + caches, single thread | {bake_ms:.0} ms |");
    println!(
        "| Pyramid memory | {:.1} MB |\n",
        (body.pyramid.data.len() * 4) as f64 / 1e6
    );

    // Sanity: weights normalised, and how many families are active.
    let mut s = Sample::default();
    let (mut active_sum, mut max_active, mut bad) = (0usize, 0usize, 0usize);
    for d in &dirs {
        body.sample(*d, BAND_SPACING[4], Depth::L4, &mut s);
        let total: f32 = s.weights.family.iter().sum();
        if (total - 1.0).abs() > 1e-4 {
            bad += 1;
        }
        let active = s.weights.family.iter().filter(|&&w| w > 1e-4).count();
        active_sum += active;
        max_active = max_active.max(active);
    }
    println!("## Classification sanity (palette 16, K 4)\n");
    println!("| What | Value |\n|---|---|");
    println!("| Samples whose family weights do not sum to 1 | {bad} |");
    println!(
        "| Mean families with weight > 1e-4 | {:.2} |",
        active_sum as f64 / N as f64
    );
    println!("| Max families with weight > 1e-4 | {max_active} |\n");

    println!("## Cost per band (palette 16, suites 4, K 4)\n");
    println!("| Band | Spacing | Layers | ns / sample | 65×65 patch |\n|---|---|---|---|---|");
    for band in 0..5 {
        let depth = Depth::for_band(band);
        let ns = sample_cost(&body, &dirs, band, depth);
        println!(
            "| {band} | {} m | {depth:?} | {ns:.0} | {:.2} ms |",
            BAND_SPACING[band],
            ns * PATCH / 1e6
        );
    }

    println!("\n## Layer breakdown at band 4\n");
    println!("| Up to | ns / sample | Increment |\n|---|---|---|");
    let mut prev = 0.0;
    for depth in [Depth::L0, Depth::L1, Depth::L2, Depth::L3, Depth::L4] {
        let ns = sample_cost(&body, &dirs, 4, depth);
        println!("| {depth:?} | {ns:.0} | +{:.0} |", ns - prev);
        prev = ns;
    }

    let h0 = time_ns(&dirs, |d| {
        black_box(body.h0(black_box(*d)));
    });
    println!("\n`h0` alone: **{h0:.0} ns**. A full sample evaluates `h0` 4 times");
    println!("(here, upwind for orographic moisture, two slope differences).\n");

    // L2 breakdown: classifier alone on fixed axes, and one noise evaluation.
    let axes = spike_field::classify::Axes {
        temp: 285.0,
        moisture: 0.55,
        altitude: 300.0,
        slope: 8.0,
        substrate: [0.7, 0.2, 0.1],
    };
    let mut w = Sample::default().weights;
    let classify_ns = time_ns(&dirs, |d| {
        let p = [d[0] * 24_000.0, d[1] * 24_000.0, d[2] * 24_000.0];
        body.content
            .classify(black_box(p), black_box(&axes), &mut w);
        black_box(&w);
    });
    let noise_ns = time_ns(&dirs, |d| {
        let p = [d[0] * 12.0, d[1] * 12.0, d[2] * 12.0];
        black_box(genesis_core::noise::gradient3(seed, black_box(p)));
    });
    let pyramid_ns = time_ns(&dirs, |d| {
        black_box(body.pyramid.sample(black_box(*d)));
    });
    println!("| `h0` component | ns |\n|---|---|");
    println!("| pyramid bilinear fetch | {pyramid_ns:.0} |");
    println!(
        "| 3 noise evaluations (plate + 2 erosion octaves) | {:.0} |",
        3.0 * noise_ns
    );
    println!(
        "| remainder: 48-crater loop, no spatial index | ≈ {:.0} |\n",
        h0 - pyramid_ns - 3.0 * noise_ns
    );
    println!("| L2 component | ns |\n|---|---|");
    println!("| two extra `h0` for slope | {:.0} |", 2.0 * h0);
    println!(
        "| one `gradient3` (×5: jitter + substrate) | {noise_ns:.0} (×5 = {:.0}) |",
        5.0 * noise_ns
    );
    println!("| `classify` alone (palette 16, temperate axes) | {classify_ns:.0} |\n");

    println!("## Content sweeps at band 4, full stack\n");
    println!("| Palette | Suites | K | ns / sample |\n|---|---|---|---|");
    for (palette, suites, k_budget) in [
        (8, 4, 4),
        (16, 4, 4),
        (32, 4, 4),
        (32, 4, 8),
        (16, 2, 4),
        (16, 8, 4),
    ] {
        let b = Body::bake(seed, palette, suites, 4, k_budget, 256);
        let ns = sample_cost(&b, &dirs, 4, Depth::L4);
        println!("| {palette} | {suites} | {k_budget} | {ns:.0} |");
    }

    println!("\n## Thread scaling, band 4 full stack (palette 16)\n");
    println!("| Threads | Samples / s | 65×65 patches / s |\n|---|---|---|");
    for threads in [1usize, 4, 8, 16] {
        let chunk = dirs.len().div_ceil(threads);
        let t = Instant::now();
        std::thread::scope(|scope| {
            for part in dirs.chunks(chunk) {
                let body = &body;
                scope.spawn(move || {
                    let mut s = Sample::default();
                    for _ in 0..3 {
                        for d in part {
                            body.sample(*d, BAND_SPACING[4], Depth::L4, &mut s);
                            black_box(&s);
                        }
                    }
                });
            }
        });
        let rate = (3 * dirs.len()) as f64 / t.elapsed().as_secs_f64();
        println!("| {threads} | {:.2} M | {:.0} |", rate / 1e6, rate / PATCH);
    }

    println!("\n## libm versus std maths\n");
    let xs: Vec<f32> = (0..1_000_000).map(|i| i as f32 * 1e-3).collect();
    let bench = |f: &dyn Fn(f32) -> f32| {
        let t = Instant::now();
        let mut acc = 0.0f32;
        for _ in 0..5 {
            for &x in &xs {
                acc += f(black_box(x));
            }
        }
        black_box(acc);
        t.elapsed().as_nanos() as f64 / (5.0 * xs.len() as f64)
    };
    println!("| Function | libm ns | std ns |\n|---|---|---|");
    println!(
        "| sin | {:.2} | {:.2} |",
        bench(&genesis_core::math::f32::sin),
        bench(&f32::sin)
    );
    println!(
        "| exp | {:.2} | {:.2} |",
        bench(&|x| genesis_core::math::f32::exp(x * 0.01)),
        bench(&|x| (x * 0.01).exp())
    );
    println!(
        "| atan | {:.2} | {:.2} |",
        bench(&genesis_core::math::f32::atan),
        bench(&f32::atan)
    );
}
