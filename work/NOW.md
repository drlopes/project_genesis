# Now

Rules: [work/README.md](README.md). One active goal; everything else is captured, not chased.

## Active goal

**E5 — Coordinate precision.** Does a floating origin hold at system-scale distances? Definition:
[docs/21 E5](../docs/21-phase-1-spike-plan.md#e5--coordinate-precision).

- [x] Candidates: `big_space` is pre-release only for Bevy 0.20 → our own camera-relative frame,
      with a naive f32 frame as the control
- [x] Emulated GPU precision (`precision.rs`): relative ≤ 0.0014 px at 0–10¹⁰ m; naive 37 px at
      10⁶ m, 34 632 px at 10¹⁰ m
- [x] In-app runs at 0 / 10⁶ / 10¹⁰ m with screenshots: relative at 10¹⁰ m identical to the origin;
      naive at 10¹⁰ m shredded (control works)
- [x] Physics part moved to E6 (plan change in docs/21); flight maths in f32 body-local noted as a
      large-body risk for E6
- [x] Results in `docs/spikes/E5-coordinate-precision.md`; write-back to docs/01, 10, 20, 21
- [ ] **User:** visual jitter check (below)

## Where we stopped

E5 measured and written up; only the user's visual check remains, then close the goal.

## Waiting on the user

1. **E5 visual check.** From `spikes/`, fly low (Q) and creep along the ground in each:
   - `cargo run --release -p spike_app -- --system-distance 1e10` (the real frame): should look
     exactly like the normal app, no shimmer.
   - `cargo run --release -p spike_app -- --system-distance 1e6 --frame naive` (the control):
     expected to jitter visibly near the ground. It is supposed to look wrong.

## Up next

1. **E6 — Collision and physics** (now including the resting-body test and large-body local
   frames), then **E7 — Voxels**.
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
  geomorphing against popping. [Results](../docs/spikes/E3-lod-and-quadtree.md).
- 2026-11-10 — **Work structure**: `work/`, changelog, research and knowledge base, concept art
  with LFS. Budget table in docs/21 reviewed and decided.
