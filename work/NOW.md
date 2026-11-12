# Now

Rules: [work/README.md](README.md). One active goal; everything else is captured, not chased.

## Active goal

**E6 — Collision and physics.** Which physics crate accepts our field as terrain collision, at
what cost, with no fall-through? Definition:
[docs/21 E6](../docs/21-phase-1-spike-plan.md#e6--collision-and-physics).

- [ ] Candidates on Bevy 0.20: `avian3d` 0.8 (has `f64`) and `bevy_rapier3d` 0.37 (f32 only);
      each with (a) per-patch heightfield colliders from the field, (b) a custom shape querying the
      field directly
- [ ] Headless tests: 1 000 capsule drops at 0–300 m/s on mixed terrain (fall-through count),
      10 k raycasts per frame, physics ms per frame
- [ ] Physics local frame (from E5): origin near the player, rebased as it moves; resting body
      for 10 minutes on a 24 km body and an Earth-radius body
- [ ] Character walking and jumping across patch seams and LOD changes; **user** tries the feel
- [ ] Results in `docs/spikes/E6-collision-and-physics.md`; write-back to docs/18 and docs/20

## Where we stopped

E5 closed. E6 not started; first step is checking both crates' APIs for custom shapes and
heightfields on Bevy 0.20.

## Waiting on the user

_Nothing._

## Up next

1. **E7 — Voxels.**
2. **E8 — Classification look**, using top-down renders.
3. **B-001** — decide on the `rng::below` wording or rejection step before anything persistent
   depends on it.
4. **B-002** — make `spike_field` clippy-clean, or exempt spikes from linting explicitly.

## Done recently

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
