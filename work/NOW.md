# Now

Rules: [work/README.md](README.md). One active goal; everything else is captured, not chased.

## Active goal

**E5 — Coordinate precision.** Does a floating origin hold at system-scale distances? Place the
body 10⁸, 10⁹, and 10¹⁰ m from the system origin; stand, walk, and orbit; measure vertex and
camera jitter, and physics stability. Definition:
[docs/21 E5](../docs/21-phase-1-spike-plan.md#e5--coordinate-precision).

- [ ] Choose the candidates' shape: `big_space` (check its Bevy 0.20 support first) versus our own
      camera-relative frame (f64 world positions, f32 render transforms rebased each frame)
- [ ] Carry B-003's lesson: no fixed-direction angles for orientation on a sphere
- [ ] Physics part depends on E6's engine choice; decide whether E5 tests a minimal resting-body
      check or waits for E6
- [ ] Metrics, runs, results in `docs/spikes/E5-coordinate-precision.md`; user visual check

## Where we stopped

E4 closed. E5 not started; first step is checking `big_space`'s Bevy 0.20 support and deciding
how E5 handles physics before E6.

## Waiting on the user

_Nothing._

## Up next

1. **E6 — Collision and physics**, then **E7 — Voxels**.
2. **E8 — Classification look**, using top-down renders.
3. **B-001** — decide on the `rng::below` wording or rejection step before anything persistent
   depends on it.
4. **B-002** — make `spike_field` clippy-clean, or exempt spikes from linting explicitly.

## Done recently

- 2026-11-11 — **E4 — Streaming throughput.** Pass at rover speed (4 workers, ~57 of ~620
  patches/s). Full detail while speed / height above ground ≤ ~6 /s; above that, detail lags but
  never shows holes. Generation must never use all cores (8 workers on 8 cores: frame p99 49 ms
  under load). The integration budget must guarantee progress. Holes in the metrics were invisible
  far-side patches (I-009). Ideas: I-008 kept open, I-009 new, I-001/I-002/I-004/I-006 parked.
  [Results](../docs/spikes/E4-streaming-throughput.md).
- 2026-11-11 — **I-005 — Compressed patch vertex format.** 32 bytes per vertex (from 80) with both
  morph states; terrain mesh 331 MB (from 783), peak VRAM above baseline 1.34–1.39 GB (from 2.44),
  within budget; 16-bit positions sufficient at every level; user found no artefacts.
- 2026-11-10 — **E3 — LOD and quadtree** (and **E9 — build iteration**). Grid 65 × 65, error metric
  one sample spacing in screen pixels with default SSE 16; skirts against cracks and time-based
  geomorphing against popping (skirts alone failed the first visual check); four streaming rules
  in docs/10; static Bevy + `rust-lld` (~6 s incremental builds). Found and fixed B-003 (camera).
  [Results](../docs/spikes/E3-lod-and-quadtree.md).
- 2026-11-10 — **Work structure**: `work/`, changelog, research and knowledge base, concept art
  with LFS. History rewritten to drop 11 MB of build artefacts (force-pushed; backup bundle kept
  next to the repo). Budget table in docs/21 reviewed and decided.
