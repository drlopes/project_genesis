# Bugs

Rules and entry format: [work/README.md](README.md). Severity: **S1** stop and fix · **S2** fix
this goal or next · **S3** when convenient.

## Open

### B-001 — `rng::below` claims to be unbiased, but is not exactly
- **Severity:** S3 · **Status:** open
- **Found:** 2026-11-10, commit `1eca196` (`e13d139` before the history rewrite), while setting up the work structure (reviewing code
  cited in a research card)
- **Where:** `crates/genesis_core/src/rng.rs`, `below()`
- **Expected:** the doc comment matches the behaviour.
- **Actual:** the comment says "without modulo bias (Lemire's multiply-shift)", but Lemire's method
  is only exactly unbiased with its rejection step, which is not implemented. The bias is at most
  `n / 2^64` (negligible for any practical `n`), so this is a misleading comment, not a numerical
  problem.
- **Repro:** read the function; compare with Lemire (2019), see
  [research card](../research/sources/lemire-2019-fast-random-integer-in-interval.md).
- **Fix options:** reword the comment to state the bound, or add the rejection loop. Adding the
  loop changes outputs for some keys, so it must be decided before anything persistent depends on
  `below`.

### B-002 — The spikes workspace is not clippy-clean, and CI does not lint it
- **Severity:** S3 · **Status:** open
- **Found:** 2026-11-10, during E3 geomorphing (running clippy on `spike_app`)
- **Where:** `spikes/field/src/classify.rs` (lines ~183, 260, 284), `spikes/field/src/lib.rs`
  (lines ~291, 297); `.github/workflows/ci.yml` lints only the root workspace.
- **Expected:** `cargo clippy --all-targets -- -D warnings` passes in `spikes/` too, as in the root.
- **Actual:** 5 errors in `spike_field` (`needless_range_loop` ×4, `unnecessary_lazy_evaluations`
  ×1). `spike_app` alone passes with `--no-deps`.
- **Repro:** `cd spikes; cargo clippy -p spike_app --all-targets -- -D warnings`.
- **Fix options:** fix the five lints (mechanical; re-run the E2 check, since `spike_field` is
  hashed), then add a spikes clippy step to CI, or decide that spikes are exempt and say so in
  `AGENTS.md`.

## Closed

_None yet._
