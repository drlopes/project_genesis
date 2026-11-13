# Now

Rules: [work/README.md](README.md). One active goal; everything else is captured, not chased.

## Active goal

**E6 — Collision and physics.** Which physics crate accepts our field as terrain collision, at
what cost, with no fall-through? Definition:
[docs/21 E6](../docs/21-phase-1-spike-plan.md#e6--collision-and-physics).

- [x] Candidates: neither crate accepts a custom field collider cleanly (Avian's `AnyCollider`
      replaces the whole collider type; bevy_rapier exposes no contact dispatcher) → option (a),
      per-patch triangle meshes from the visual vertices
- [x] `spikes/physics` headless harness; `spike_app` split into lib + bin
- [x] Avian: drops (0 fall-through terrain-only in 12 000; 6/1 000 from body–body impacts),
      rays (5 ms/10 k vs 500 ms for the field trace; agreement 1.2 cm p99), rest (0 mm sleeping;
      never-sleeping bodies creep on slopes, a solver property; local frame fine at Earth radius,
      naive fails)
- [x] Rapier: the same three tests (0 fall-through everywhere incl. without CCD, verified by a
      no-terrain control; faster step tail; but bodies creep on mesh contacts before sleeping)
- [ ] Character walking and jumping across patch seams and LOD changes (Avian `MoveAndSlide`);
      physics ms/frame (pass ≤ 1 ms); **user** tries the feel
- [ ] Decisions; write-back to docs/18 (incl. B-004) and docs/20

## Where we stopped

Both engines measured headless; comparison in `docs/spikes/E6-collision-and-physics.md`. Next:
the character test (walk and jump across seams and LOD changes), which decides the engine.

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
