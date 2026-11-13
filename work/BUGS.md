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

### B-005 — The crater term has a vertical wall at every crater rim
- **Severity:** S2 · **Status:** fixed (2026-11-13, found during E7) · **Found:** 2026-11-12
- **Fixed in:** `e727230`
- **Where:** `spikes/field/src/lib.rs`, `Body::h0`, the crater loop.
- **Expected:** a continuous crater profile: bowl inside the rim, raised rim outside.
- **Actual:** the outside branch was `0.2·depth·(2 − t)`, which is `0.2·depth` at `t = 1` while the
  inside branch is `0` there. Every crater rim was a discontinuity of up to 60 m across a single
  sample — a vertical wall, and the source of the 12 m "seam outlier" seen in E7 (a 47 m step
  within 0.1 m). The heightfield and the voxel mesh each resolve the wall differently, so a
  pointwise height comparison there is meaningless.
- **Fix:** the outside branch is now `0.8·depth·u·(1 − u)` with `u = t − 1` (zero at both ends of
  the rim band, peak `0.2·depth` at `t = 1.5`), continuous with the bowl at `t = 1`.
- **Repro:** before the fix, `spike_voxel --radius 128 --voxel 0.5 --seam` reports a max seam step
  of ~12 m; a profile at 0.1 m spacing across the reported point jumps 47 m. After the fix, the
  max is within a voxel and the profile is continuous. E1–E6 conclusions are unaffected (their
  numbers are not read off crater rims). The fix changes the field's samples, so the E2 reference
  in `spikes/field/e2-expected.txt` was re-pinned deliberately (samples `550A2D4FDD6ECB63` →
  `E72B7878D4705E08`, bake unchanged; release and debug match on the reference machine).

### B-004 — docs/18 sphere-trace formula: inverted sign comment, unbracketed bisection
- **Severity:** S3 · **Status:** fixed (E6 write-back, 2026-11-12) · **Found:** 2026-11-12
- **Fixed in:** `ea97a75`
- **Where:** docs/18 "Raycasting a star-shaped solid"
- **Actual:** (1) `f(p) = (bulk.radius + height) − |p|` was commented "> 0 outside, < 0 inside",
  but that expression is positive *inside* the solid. (2) "Step while `f > tol`, then bisect the
  final segment" did not say the segment must bracket the crossing; implemented literally, the
  spike's trace returned hits ~`tol` (14 cm) short, until it walked on until the sign changed.
- **Fix:** docs/18 now uses `g(p) = |p| − surfaceRadius(dir(p))` (positive above ground) and states
  that the final segment is extended in small steps until `g` changes sign before bisecting.
- **Repro:** `spikes/physics`, `field_raycast`; E6 results.

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
- **Fixed in:** `649fea1`
- **Lesson for the production camera** (E5): never parameterise orientation on a sphere by angles
  from a fixed direction.
