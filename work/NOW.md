# Now

Rules: [work/README.md](README.md). One active goal; everything else is captured, not chased.

## Active goal

**E4 — Streaming throughput.** How fast can the camera move near the surface before detail falls
behind? Same app as E3 (SSE 16, geomorphing, 32-byte vertices). Definition:
[docs/21 E4](../docs/21-phase-1-spike-plan.md#e4--streaming-throughput).

- [ ] Straight-line script: great circle at constant height above ground; 5 s hold (initial fill),
      then 60 s at constant speed; `--path straight --speed --alt --workers`
- [ ] Metrics: detail deficit (wanted patches not shown at their own level), request-to-display
      latency per patch, requests and completions per second, backlog (queued + in flight) and its
      trend (last third vs middle third of the moving phase), initial fill time
- [ ] Sweep: speeds 5 / 30 / 200 / 1000 m/s × altitude 50 / 500 m × 4 / 8 workers (16 runs)
- [ ] Results in `docs/spikes/E4-streaming-throughput.md`: the speed curve, the speed cap for low
      flight; write-back to docs/10 and docs/01 per the
      [docs/21 table](../docs/21-phase-1-spike-plan.md#write-back)
- [ ] Triage I-001, I-002, I-004, I-006 against the result

## Where we stopped

I-005 closed (user: "looks flawless"). E4 starting: the straight-line script and metrics first.

## Waiting on the user

_Nothing._

## Up next

1. **E5 — Coordinate precision.** Carry over B-003's lesson: no fixed-direction angles for
   orientation on a sphere.
2. **E6 — Collision and physics**, then **E7 — Voxels**.
3. **E8 — Classification look**, using top-down renders.
4. **B-001** — decide on the `rng::below` wording or rejection step before anything persistent
   depends on it.
5. **B-002** — make `spike_field` clippy-clean, or exempt spikes from linting explicitly.

## Done recently

- 2026-11-11 — **I-005 — Compressed patch vertex format.** 32 bytes per vertex (from 80) with both
  morph states; terrain mesh 331 MB (from 783), peak VRAM above baseline 1.34–1.39 GB (from 2.44),
  within budget; 16-bit positions sufficient at every level; user found no artefacts.
- 2026-11-10 — **E3 — LOD and quadtree** (and **E9 — build iteration**). Grid 65 × 65, error metric
  one sample spacing in screen pixels with default SSE 16; skirts against cracks and time-based
  geomorphing against popping (skirts alone failed the first visual check); four streaming rules
  in docs/10; static Bevy + `rust-lld` (~6 s incremental builds). Found and fixed B-003 (camera).
  Ideas: I-005 accepted (next goal), I-004 and I-006 parked until E4, I-007 parked.
  [Results](../docs/spikes/E3-lod-and-quadtree.md).
- 2026-11-10 — **Work structure**: `work/`, changelog, research and knowledge base, concept art
  with LFS. History rewritten to drop 11 MB of build artefacts (force-pushed; backup bundle kept
  next to the repo). Budget table in docs/21 reviewed and decided.
