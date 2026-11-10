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
- [x] **User:** watch the fly-in at SSE 8 and 16 → **neither acceptable: too much popping at both.**
      Skirts and instant LOD switches fail visually; per the E3 stop rule, try morphing once.
- [x] Results in `docs/spikes/E3-lod-and-quadtree.md`; write-back to docs/10, docs/20, docs/21
- [x] Geomorphing: per-vertex parent-level position, normal, and colour; **time-based** (0.5 s)
      rather than distance-based, because a 64-interval patch spans more than one distance band
- [x] Consistent colour across bands (colour always from L2 classification)
- [x] Debug view keys: M morph, G grey geometry, L colour by level, P freeze LOD
- [x] Re-measured with morphing: SSE 16 within frame/main-thread budgets but ~2.4 GB terrain VRAM
      (80-byte vertices); SSE 8 over on main thread (2.6 ms) and VRAM (3.5 GB)
- [ ] User re-check with morphing; then record the default SSE, accept or keep I-005, triage
      I-004–I-007, close the goal

## Where we stopped

First visual check failed (popping at SSE 8 and 16). Time-based geomorphing implemented, verified
by screenshots (`--shots`) and re-measured. This is the one morphing round the stop rule allows.

## Waiting on the user

1. **E3 re-check with morphing.** From `spikes/`: free flight
   `cargo run --release -p spike_app -- --sse 16`, toggling **M** to compare morph on/off; also
   `--script --sse 16`. Is popping now acceptable at SSE 16 (or only at 8)? Either way, morphing
   needs I-005 (compressed vertices) to fit terrain VRAM.

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
