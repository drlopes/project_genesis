# Now

Rules: [work/README.md](README.md). One active goal; everything else is captured, not chased.

## Active goal

**E3 — LOD spike app.** The first Bevy build: stream cube-sphere patches from the E1 field, fly
from high orbit to the ground, and measure frames, memory, cracks, and popping. Also measures E9
(build iteration) from the first build. Definition:
[docs/21 E3](../docs/21-phase-1-spike-plan.md#e3--lod-bands-and-quadtree).

- [ ] `spikes/app`: Bevy 0.20 app in the spikes workspace, depending on `spike_field`
- [ ] Record E9 timings for the first cold build and for incremental builds (both crates)
- [ ] Cube-sphere quadtree: level ↔ band mapping and screen-space-error selection, with hysteresis
      ([docs/10](../docs/10-lod-and-streaming.md#quadtree-level-versus-lod-band))
- [ ] Patch meshing on worker threads from `spike_field` samples; skirts for seams
- [ ] Fly camera and a scripted fly-in path (high orbit → standing on the ground), repeatable
- [ ] Capture per run: frame time p50/p99/p99.9, main-thread terrain ms, resident patches,
      triangles, VRAM, RAM
- [ ] Candidate sweep: grid 33/65/129; band spacings 0.5×/1×/2×; screen-space error 1/2/4 px;
      bodies of 8/24/40 km
- [ ] Recordings of the fly-in for the user to rate popping and cracks
- [ ] Results in `docs/spikes/E3-lod-and-quadtree.md`; write-back per the
      [docs/21 table](../docs/21-phase-1-spike-plan.md#write-back)

## Where we stopped

Work-structure goal finished, history rewritten, budgets reviewed. E3 not started; first step is
creating `spikes/app` and timing the first Bevy build.

## Waiting on the user

_Nothing._

## Up next

1. **E4 — Streaming throughput**, on the same app; then triage I-001 and I-002 against its result.
2. **E5 — Coordinate precision.**
3. **E6 — Collision and physics**, then **E7 — Voxels**.
4. **E8 — Classification look**, using top-down renders.
5. **B-001** — decide on the `rng::below` wording or rejection step before anything persistent
   depends on it.

## Done recently

- 2026-11-10 — **Work structure**: `work/`, changelog, research and knowledge base, concept art
  with LFS. History rewritten to drop 11 MB of build artefacts (force-pushed; backup bundle kept
  next to the repo). Budget table in docs/21 reviewed and decided.
