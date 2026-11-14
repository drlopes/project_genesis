# Now

Rules: [work/README.md](README.md). One active goal; everything else is captured, not chased.

## Active goal

_None._ P1-B is done; P1-C (below) needs scoping with the user.

## Where we stopped

P1-A done: `genesis_gen` bakes a rocky body's descriptor core, `genesis descriptor --seed N`
prints it, golden hashes are pinned in CI. docs/14's Phase 0 exit is met.

## Waiting on the user

_Nothing._ (2026-11-13: the user confirmed soft-Voronoi plates and craters out of the pyramid,
now `DECIDED` in docs/09 and docs/06.)

## Up next

1. **P1-C** — a climate phase in the bake, biosphere, then palette and suite selection with a
   content digest in the descriptor (needs decisions on the docs/19 and docs/06 gaps: suite
   selection, palette coverage, lognormal spread).
2. **B-002** — make `spike_field` and `spike_voxel` clippy-clean, or exempt spikes from linting
   explicitly.

## Done recently

- 2026-11-14 — **P1-B — `genesis_content`.** Pack loading (manifests, dependency order, shorthand
  resolution, aliases), `extends` with the docs/16 merge rules, typed schemas with unknown keys
  rejected, validation of docs/16 checks 1–9 and docs/17 rules 1–19 (gaps listed in docs/16), every
  reference rewritten to a full ID, the content digest, and module parameter schemas. Starter pack
  `data/core`: 10 biomes in 9 families, 1 ecotone, 1 overlay, 2 suites, 8 palettes, 11 materials,
  6 scatter presets. `genesis validate data` runs in CI. 23 tests: each rule rejects a minimal bad
  input, and they caught an order-dependent depth-limit bug.

- 2026-11-13 — **P1-A — Descriptor bake for a rocky body.** `genesis_gen`: bulk + rotation, ocean
  fraction, plates (soft Voronoi, continuous at junctions), craters (age provinces, power-law
  sizes, Poisson-disk, index), coarse relief, sea level from hypsometry (within 0.001 of the
  target area), macro pyramid and crater index caches; 12 KB core, ~30 ms bake + ~60 ms caches.
  Canonical encoding + identity hash, ten golden seeds on Windows/Linux, debug/release.
  `genesis descriptor --seed N`. Erosion, lakes and every other phase deferred (explicit nulls).
- 2026-11-13 — **B-001 fixed**: `rng::below` is exactly unbiased (Lemire rejection).
- 2026-11-13 — **E8 — Classification cost and look.** *k* 4, mosaic **0.9** (was 0.35: the
  weaker of two equally suitable families never formed patches, because the mosaic noise spans
  ~1/3 of its nominal range and palette weights differ by ~1.5×), jitter 1×; L0–L2 ~0.9 µs.
  [Results](../docs/spikes/E8-classification-look.md). **Phase 1 spike complete.**
- 2026-11-13 — **E7 — Voxels and edits.** 0.5 m voxels (coarsest passing the user's look test;
  1 m reads as broken), ±64 m region, surface nets in 32³ bricks: 17 MB, seam max 20 mm, a dig
  remeshes in 23–31 ms. Edits as 66-byte records; replay bit-identical. Found and fixed B-005 (crater
  rim walls in the field) and two mesher bugs the viewer exposed.
  [Results](../docs/spikes/E7-voxels-and-edits.md).
- 2026-11-12 — **E6 — Collision and physics.** Avian 0.8; per-patch trimesh colliders from the
  displayed patches; character walks and jumps across seams and LOD changes (0 fall-through,
  0 snags, ~0.5 ms p99), stands still exactly when idle, and the user found it flawless.
  [Results](../docs/spikes/E6-collision-and-physics.md).
- 2026-11-12 — **E5 — Coordinate precision.** Our own camera-relative frame (f64 positions, render
  translations as f64 differences cast to f32) passes at 10¹⁰ m: emulated error ≤ 0.0014 px,
  user saw no jitter; the naive f32 control fails from 10⁶ m. No `big_space` (pre-release only).
  Physics part moved to E6. [Results](../docs/spikes/E5-coordinate-precision.md).
- 2026-11-11 — **E4 — Streaming throughput.** Pass at rover speed; full detail while speed /
  height above ground ≤ ~6 /s; generation never on all cores; integration budget guarantees
  progress. [Results](../docs/spikes/E4-streaming-throughput.md).
- 2026-11-11 — **I-005 — Compressed patch vertex format.** 32 bytes per vertex; terrain mesh
  331 MB; peak VRAM within budget; no visible artefacts.
