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

### B-003 — Free-flight camera pivots, crawls, or flickers after flying around the body
- **Severity:** S2 · **Status:** fixed (E3, 2026-11-10)
- **Found:** 2026-11-10, reported by the user during the E3 re-check; present since the first
  free-flight camera.
- **Where:** `spikes/app/src/main.rs`, `drive_camera`.
- **Actual:** after flying for a while or circling the body, the camera turned by itself,
  sometimes seemed to freeze, and the terrain flickered.
- **Causes:**
  1. Heading was measured from "north" (world +Y projected onto the horizon). The default heading
     is north, so flying forward goes over the pole, where that frame is singular: north swings
     round, a fallback axis takes over at |up.y| > 0.99, and the view flips (the unit test
     reproduces a 180° turn in one step).
  2. Altitude, and therefore speed, was measured from the reference sphere. Over oceans and basins,
     which lie below it, it clamped to 1 m, so the camera crawled at 2 m/s, which looked frozen.
  3. Introduced with geomorphing: frustum-culling boxes covered only a patch's own shape, so
     mid-morph patches near the screen edge could be culled while visible.
- **Fix:** orientation is carried along the sphere by parallel transport and re-levelled, with no
  reference direction (`spikes/app/src/flycam.rs`, tests `crossing_the_poles_does_not_turn_the_camera`
  and `look_keeps_the_horizon_level_and_clamps_pitch`); altitude and speed are measured from the
  coarse ground, and the camera stays 2 m above it; each patch gets a culling box covering both
  morph shapes.
- **Lesson for the production camera** (E5): never parameterise orientation on a sphere by angles
  from a fixed direction.
