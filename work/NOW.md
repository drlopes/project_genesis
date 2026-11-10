# Now

Rules: [work/README.md](README.md). One active goal; everything else is captured, not chased.

## Active goal

**E3 — LOD spike app.** The first Bevy build: stream cube-sphere patches from the E1 field, fly
from high orbit to the ground, and measure frames, memory, cracks, and popping. Also measures E9
(build iteration) from the first build. Definition:
[docs/21 E3](../docs/21-phase-1-spike-plan.md#e3--lod-bands-and-quadtree).

- [x] `spikes/app`: Bevy 0.20 app in the spikes workspace, depending on `spike_field`
- [x] Record E9 timings for the first cold build and for incremental builds (both crates)
- [x] E9: measure Bevy `dynamic_linking` and linker options (→ static Bevy + `rust-lld`, ~6 s;
      `dynamic_linking` unavailable on 0.20.1, I-007)
- [x] Cube-sphere quadtree: level ↔ band mapping and screen-space-error selection, with hysteresis
      ([docs/10](../docs/10-lod-and-streaming.md#quadtree-level-versus-lod-band)); horizon culling
- [x] Patch meshing on worker threads from `spike_field` samples; skirts for seams
- [x] Fly camera and a scripted fly-in path (high orbit → standing on the ground), repeatable
- [x] Capture per run: frame time p50/p99/p99.9, main-thread terrain ms, resident patches,
      triangles, VRAM, RAM; per-frame CSV
- [x] Candidate sweep, first pass: grid 33/65/129 × SSE 2–16 (SSE 1–4 px from the plan proved far
      too dense; see results)
- [x] Final sweep: grid 65 at SSE 8/16, bodies 8/24/40 km, band spacing 0.5×/2×
- [x] Re-run the three runs disturbed by window focus; harness now flags invalid runs
- [x] Harness: sample peak VRAM during the run; re-measured grid 65 at SSE 8 (~2.7 GB, over the
      terrain budget) and SSE 16 (1.6–2.0 GB, within)
- [ ] **User:** watch the fly-in at SSE 8 and 16 and rate popping and cracks
- [x] Results in `docs/spikes/E3-lod-and-quadtree.md`; write-back to docs/10, docs/20, docs/21
- [ ] After the user check: record the default SSE in docs/10 and E3, and accept or keep I-005
      accordingly; triage I-004–I-007; close the goal

## Where we stopped

E3 measured and written up; E9 done (static Bevy + `rust-lld`). Everything is decided except the
default error threshold (SSE 16 vs 8), which needs the user's visual check.

## Waiting on the user

1. **E3 visual check.** From `spikes/`, run
   `cargo run --release -p spike_app -- --script --sse 16` and then `--sse 8`. Rate popping
   (patches visibly changing detail) and cracks (gaps between patches). SSE 16 fits every budget;
   SSE 8 needs I-005 (compressed vertices) to fit terrain VRAM.

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
