# Now

Rules: [work/README.md](README.md). One active goal; everything else is captured, not chased.

## Active goal

**E7 — Voxels.** What voxel size and region radius give believable digging within budget?
Definition: [docs/21 E7](../docs/21-phase-1-spike-plan.md#e7--voxels-and-edits-q2).

- [ ] Fixed test site; scripted edits (flatten, tunnel, cliff carve, 500 small digs)
- [ ] Voxel size 0.25 / 0.5 / 1 m; region radius 32 / 64 / 128 m; surface nets versus marching
      cubes (start by checking which of these is worth measuring)
- [ ] Metrics: RAM per region, edit → remesh latency, seam at the voxel/mesh boundary, largest
      edit radius per frame, record storage per 1 000 edits, superseded-edit GC
- [ ] Look test with **the user** from 2 m; pick the coarsest size that passes

## Where we stopped

E6 closed: Avian chosen and written back; the user's walking feel check passed ("works
flawlessly; no glaring issues"). E7 is next, following the E6 collision model it depends on.

## Waiting on the user

_Nothing._

## Up next

1. **E8 — Classification look**, using top-down renders.
2. **B-001** — decide on the `rng::below` wording or rejection step before anything persistent
   depends on it.
3. **B-002** — make `spike_field` clippy-clean, or exempt spikes from linting explicitly.

## Done recently

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
- 2026-11-10 — **E3 — LOD and quadtree** (and **E9 — build iteration**). Grid 65 × 65, SSE 16,
  skirts + time-based geomorphing. [Results](../docs/spikes/E3-lod-and-quadtree.md).
