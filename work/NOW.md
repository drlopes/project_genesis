# Now

Rules: [work/README.md](README.md). One active goal; everything else is captured, not chased.

## Active goal

**E8 — Classification cost and look.** Are the docs/19 defaults cheap enough and good-looking
enough? Definition: [docs/21 E8](../docs/21-phase-1-spike-plan.md#e8--classification-cost-and-look).

- [ ] Top-down biome-map renders from the macro grid, stub content pack (8 aerial biomes,
      1 ecotone, 1 variant)
- [ ] Sharpness *k* 2 / 4 / 6; mosaic strength 0.2 / 0.35 / 0.6; jitter 0.5× / 1× / 2×
- [ ] Metrics: L2 cost (from E1), border width, patch size distribution
- [ ] The user picks a default set (one round; further tuning is Phase 2)

## Where we stopped

E7 closed: 0.5 m voxels passed the user's look test; ±64 m region, surface nets, edits as
records. E8 is the last Phase 1 experiment.

## Waiting on the user

- **I-014 — voxelise on first edit** (proposed in docs/10): keep the heightfield for unedited
  ground near the player and switch a chunk to voxels on its first edit. Agree, reject, or park.

## Up next

1. **B-001** — decide on the `rng::below` wording or rejection step before anything persistent
   depends on it.
2. **B-002** — make `spike_field` and `spike_voxel` clippy-clean, or exempt spikes from linting
   explicitly.

## Done recently

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
