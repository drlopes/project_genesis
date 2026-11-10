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

## Closed

_None yet._
