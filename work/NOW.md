# Now

Rules: [work/README.md](README.md). One active goal; everything else is captured, not chased.

## Active goal

**I-005 — Compressed patch vertex format.** E3's accepted setup (SSE 16 with geomorphing) peaks at
~2.4 GB of terrain VRAM against the 2 GB budget, because each vertex carries its own and its
parent's state (80 bytes). Make it compact, and re-measure, before E4 measures streaming on top of
it. Context: [I-005](IDEAS.md#closed), [docs/10 Seams](../docs/10-lod-and-streaming.md#seams).

- [x] Choose the layout: 2 × Unorm16x4 positions (box = entity transform), Snorm16x4 octahedral
      normals, 2 × Unorm8x4 gamma-2 colours, u16 indices: 32 bytes/vertex. 16-bit is enough at
      every level (step ≈ 0.0009 of the sample spacing; test)
- [x] Encode in `patch.rs`, decode in `terrain.wesl`; culling box = unit box over both shapes
- [x] Morph contract test on the encoded data; octahedral round-trip test (< 0.01°)
- [x] Re-measured grid 65, SSE 16 (two valid runs): terrain mesh 331 MB (783 MB in float format),
      peak VRAM above baseline 1.34–1.39 GB (was 2.44); frame and main-thread budgets unchanged;
      screenshots show no artefacts. Harness reports resident terrain mesh bytes.
- [ ] **User:** quick visual check for artefacts (stair-stepping, shading bands)
- [x] Write back: docs/10 (vertex format), E3 results (I-005 follow-up), docs/21 budget row

## Where we stopped

I-005 implemented, measured, and written up; only the user's visual check remains, then close the
goal and start E4.

## Waiting on the user

1. **I-005 visual check.** From `spikes/`: `cargo run --release -p spike_app -- --sse 16`. Fly
   low and look for stair-stepping on slopes, shading bands, or new cracks between patches
   (G = grey geometry makes shading easiest to judge).

## Up next

1. **E4 — Streaming throughput**, on the same app; then triage I-001, I-002, I-004, and I-006
   against its result.
2. **E5 — Coordinate precision.** Carry over B-003's lesson: no fixed-direction angles for
   orientation on a sphere.
3. **E6 — Collision and physics**, then **E7 — Voxels**.
4. **E8 — Classification look**, using top-down renders.
5. **B-001** — decide on the `rng::below` wording or rejection step before anything persistent
   depends on it.
6. **B-002** — make `spike_field` clippy-clean, or exempt spikes from linting explicitly.

## Done recently

- 2026-11-10 — **E3 — LOD and quadtree** (and **E9 — build iteration**). Grid 65 × 65, error metric
  one sample spacing in screen pixels with default SSE 16; skirts against cracks and time-based
  geomorphing against popping (skirts alone failed the first visual check); four streaming rules
  in docs/10; static Bevy + `rust-lld` (~6 s incremental builds). Found and fixed B-003 (camera).
  Ideas: I-005 accepted (next goal), I-004 and I-006 parked until E4, I-007 parked.
  [Results](../docs/spikes/E3-lod-and-quadtree.md).
- 2026-11-10 — **Work structure**: `work/`, changelog, research and knowledge base, concept art
  with LFS. History rewritten to drop 11 MB of build artefacts (force-pushed; backup bundle kept
  next to the repo). Budget table in docs/21 reviewed and decided.
