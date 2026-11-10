# E2 — Determinism

Experiment definition: [21 — Phase 1 Spike Plan](../21-phase-1-spike-plan.md#e2--determinism).

| | |
|---|---|
| Date | 2026-11-10 |
| Commit | `440f5b8` (harness `spikes/field/src/bin/e2.rs`); `908c5dc` before the 2026-11-10 history rewrite, same content |
| Platforms | Windows 11 (reference machine); Windows and Ubuntu GitHub Actions runners |
| Builds | release and debug |

## Method

- **10 golden seeds** (`1, 2, 3, 42, 1337, 0xDEADBEEF, 0x5EED0001, 7777777, 123456789, u64::MAX`).
- Each body is baked (macro pyramid 6 × 128 × 128 and caches) and its pyramid hashed bit-for-bit.
- **20 000 samples per seed** at band 4, full L0–L4 stack, the same cost-faithful field as
  [E1](./E1-field-cost.md). Each sample's height, temperature, moisture, albedo, and every family
  weight are hashed.
- Each run evaluates the samples in a different way: **1 thread in order**, **4 threads
  shuffled**, **16 threads shuffled**. Digests are combined in sample-index order, so the final
  hash only matches if every individual sample is bit-identical.
- The result is checked against `spikes/field/e2-expected.txt`, recorded on the reference
  machine. CI repeats the check on Windows and Linux, in release and debug, on every push.

## Results

| Platform | Build | Bake hash | Sample hash (all three runs) |
|---|---|---|---|
| Windows 11, reference | release | `12BD45A57E50BBA5` | `550A2D4FDD6ECB63` |
| Windows 11, reference | debug | `12BD45A57E50BBA5` | `550A2D4FDD6ECB63` |
| Windows, CI runner (`windows-latest`) | release, debug | matches | matches |
| Ubuntu, CI runner (`ubuntu-latest`) | release, debug | matches | matches |

"Matches" means the `--check` step passed: both hashes equal the reference values exactly (CI run
for commit `908c5dc`, now `440f5b8` after the history rewrite with identical content; all steps
green on both platforms).

The harness itself was checked: with a deliberately wrong expected hash it exits with an error.

## Decisions

1. **Pass.** Generation is bit-identical across thread count (1 / 4 / 16), evaluation order,
   build profile (release / debug), and OS (Windows / Linux). The determinism rules in
   [20](../20-tech-stack-and-workspace.md#determinism-rules) work as intended; D43 is confirmed.
2. **The check is permanent.** It runs in CI on every push and pull request. A change that alters
   generation output fails CI unless `e2-expected.txt` is updated deliberately in the same commit.
3. **`libm` cost is acceptable.** Measured in [E1](./E1-field-cost.md#libm-versus-std): slower `exp`,
   faster `sin` and `atan`; negligible in sample totals.
4. **Golden-seed procedure** ([11](../11-determinism-and-sampling.md#golden-seeds)): this harness
   is the pattern. Production golden tests in `genesis_gen` hash a fixed seed set and run in the
   same CI matrix.

## Caveats

- One CPU vendor per platform (whatever the runners use); an ARM or very different x86 CPU is not
  covered. PC-only (D42) makes x86-64 the target.
- The field is the spike's, not production code. The same rules (`libm`, in-house noise,
  ordered iteration) apply to `genesis_gen`, and its golden tests will run in the same CI job.
