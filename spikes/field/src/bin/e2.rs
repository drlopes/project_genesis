//! E2 — determinism (docs/21-phase-1-spike-plan.md#e2--determinism).
//!
//! Hashes the full sample output for 10 golden seeds, evaluated with different thread counts and
//! shuffled orders. Digests are combined in sample-index order, so the final hash is identical
//! only if every individual sample is bit-identical.
//!
//!   cargo run --release --bin e2                          # print hashes
//!   cargo run --release --bin e2 -- --check <file>        # exit 1 unless they match the file

use genesis_core::seed::mix64;
use genesis_core::{Seed, rng};
use spike_field::{BAND_SPACING, Body, Depth, Sample, directions};

const GOLDEN_SEEDS: [u64; 10] = [
    1,
    2,
    3,
    42,
    1337,
    0xDEAD_BEEF,
    0x5EED_0001,
    7_777_777,
    123_456_789,
    u64::MAX,
];
const SAMPLES_PER_SEED: usize = 20_000;

fn digest(s: &Sample) -> u64 {
    let mut h = mix64(s.height.to_bits() as u64);
    for v in [s.temp, s.moisture, s.albedo[0], s.albedo[1], s.albedo[2]] {
        h = mix64(h ^ v.to_bits() as u64);
    }
    for w in &s.weights.family {
        h = mix64(h ^ w.to_bits() as u64);
    }
    h
}

/// Evaluate every sample of every golden body, in a given order, on `threads` threads.
fn run(bodies: &[(Body, Vec<[f64; 3]>)], threads: usize, shuffle: bool) -> u64 {
    let mut total = 0u64;
    for (body, dirs) in bodies {
        let n = dirs.len();
        let mut order: Vec<usize> = (0..n).collect();
        if shuffle {
            // Fisher–Yates with the counter RNG: a different order, same samples.
            for i in (1..n).rev() {
                let j =
                    rng::below(Seed(threads as u64), "shuffle", i as u64, i as u64 + 1) as usize;
                order.swap(i, j);
            }
        }
        let mut digests = vec![0u64; n];
        let chunk = n.div_ceil(threads);
        let parts: Vec<Vec<(usize, u64)>> = std::thread::scope(|scope| {
            let handles: Vec<_> = order
                .chunks(chunk)
                .map(|part| {
                    scope.spawn(move || {
                        let mut s = Sample::default();
                        part.iter()
                            .map(|&i| {
                                body.sample(dirs[i], BAND_SPACING[4], Depth::L4, &mut s);
                                (i, digest(&s))
                            })
                            .collect()
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for (i, d) in parts.into_iter().flatten() {
            digests[i] = d;
        }
        for d in digests {
            total = mix64(total ^ d);
        }
    }
    total
}

fn bake_hash(body: &Body) -> u64 {
    body.pyramid
        .data
        .iter()
        .fold(0u64, |h, v| mix64(h ^ v.to_bits() as u64))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let check = args
        .iter()
        .position(|a| a == "--check")
        .map(|i| args[i + 1].clone());

    let bodies: Vec<(Body, Vec<[f64; 3]>)> = GOLDEN_SEEDS
        .iter()
        .map(|&s| {
            let seed = Seed(s);
            (
                Body::bake(seed, 16, 4, 4, 4, 128),
                directions(seed.derive("e2"), SAMPLES_PER_SEED),
            )
        })
        .collect();

    let bake = bodies
        .iter()
        .fold(0u64, |h, (b, _)| mix64(h ^ bake_hash(b)));
    let runs = [(1, false), (4, true), (16, true)];
    let samples: Vec<u64> = runs.iter().map(|&(t, sh)| run(&bodies, t, sh)).collect();

    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    println!("E2 determinism — {} ({profile})", std::env::consts::OS);
    println!("bake    {bake:016X}");
    for (&(t, sh), h) in runs.iter().zip(&samples) {
        println!("samples {h:016X}  threads={t:<2} shuffled={sh}");
    }

    let consistent = samples.iter().all(|&h| h == samples[0]);
    if !consistent {
        println!("FAIL: results depend on thread count or order");
        std::process::exit(1);
    }

    if let Some(path) = check {
        let expected = std::fs::read_to_string(&path).expect("read expected file");
        let want: Vec<&str> = expected.split_whitespace().collect();
        let got = [format!("{bake:016X}"), format!("{:016X}", samples[0])];
        if want != got {
            println!(
                "FAIL: expected bake {} samples {}, got bake {} samples {}",
                want[0], want[1], got[0], got[1]
            );
            std::process::exit(1);
        }
        println!("OK: matches {path}");
    }
}
