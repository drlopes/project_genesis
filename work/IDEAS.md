# Ideas

Rules and entry format: [work/README.md](README.md). Ideas are triaged at the end of each goal;
they are not acted on directly.

## Open

### I-001 — Bake classification inputs that need extra `h0` evaluations
- **Status:** parked (revisit after E4)
- **Raised:** 2026-11-10, from [E1](../docs/spikes/E1-field-cost.md#open-optimisations-not-needed-yet)
- **What:** store slope and the orographic upwind term on the macro grid at bake, instead of
  evaluating `h0` three more times per sample.
- **Why:** about 500 ns of the ~1.05 µs L0–L2 cost; roughly halves classification-band cost while
  keeping LOD independence.
- **Cost / risk:** one more cache (memory per body); slope resolution drops to the macro grid
  (~150 m on a 24 km body).
- **Decision:** parked until E4 shows whether throughput needs it.

### I-002 — Multi-channel noise for classification jitter
- **Status:** parked (revisit after E4)
- **Raised:** 2026-11-10, from [E1](../docs/spikes/E1-field-cost.md#open-optimisations-not-needed-yet)
- **What:** one lattice hash per corner, with different gradient bits per channel, so the four
  jitter axes cost about one noise evaluation instead of four.
- **Why:** ~50 ns per sample; small, but jitter runs on every classified sample.
- **Cost / risk:** new noise function to pin with known answers; low.
- **Decision:** parked at the end of the work-structure goal: E1 is within budget, so this only
  matters if E4 shows a throughput shortfall. Revisit together with I-001.

### I-003 — Mirror work items to GitHub Issues when collaborators join
- **Status:** parked
- **Raised:** 2026-11-10, while designing the work structure
- **What:** keep `work/*.md` as the source of truth and mirror open bugs and accepted ideas to
  GitHub Issues for discussion and boards.
- **Why:** Issues have better UI and notifications once more than one person is involved.
- **Cost / risk:** two sources can drift; needs a one-way sync rule or tooling.

### I-004 — View-dependent LOD: less detail outside the view frustum
- **Status:** parked (revisit after E4)
- **Raised:** 2026-11-10, from E3
- **What:** select patches outside the camera frustum with a coarser error threshold (e.g. 4× the
  screen-space error) instead of full detail.
- **Why:** E3's selection ignores view direction, so patches behind the camera are generated and kept
  resident at full detail. Bevy does not draw them, but they cost generation, RAM, and VRAM; roughly
  two thirds of the near-ground patch set is outside a 60° view.
- **Cost / risk:** pop-in when the camera turns quickly; needs a turn-rate test. Interacts with
  collision (E6), which needs geometry around the player regardless of view.
- **Decision:** parked at the end of E3. E4 measures streaming load; decide then whether the
  saving is needed. Note that geomorphing hides pop-in on turns only partly (0.5 s morph).

### I-006 — Fewer entities per patch set
- **Status:** parked (revisit after E4)
- **Raised:** 2026-11-10, from E3
- **What:** reduce per-patch main-thread overhead (one entity, one mesh asset, one draw call per
  patch): batch uploads, pool and reuse mesh buffers, or render patches through one custom
  instanced/indirect draw.
- **Why:** E3's grid-33 runs failed on main-thread integration rate, not on worker throughput: the
  per-patch fixed cost, not the vertex count, set the ceiling.
- **Cost / risk:** a custom render path is real engine work; pooling is cheap.
- **Decision:** parked at the end of E3. E4 measures integration rate under sustained movement,
  which is exactly what this would improve; decide with that data.

### I-007 — Revisit Bevy `dynamic_linking` at the next Bevy upgrade
- **Status:** parked (until the next Bevy upgrade)
- **Raised:** 2026-11-10, from E9
- **What:** check whether `bevy_dylib` matches the Bevy version and measure incremental builds with
  `dynamic_linking` in dev.
- **Why:** it usually cuts incremental link time sharply; on 0.20.1 it is unavailable because
  `bevy_dylib 0.20.1` was never published.
- **Cost / risk:** none to check; dev-only feature, never shipped.
- **Decision:** stays parked at the end of E3; static Bevy with `rust-lld` meets the targets.

## Closed

### I-005 — Compressed patch vertex format
- **Status:** accepted → goal in the `NOW.md` queue (before E4)
- **Raised:** 2026-11-10, from E3
- **What:** store patch vertices compactly: 16-bit positions relative to the patch, octahedral-
  encoded normals, RGBA8 colours, for both the own and the parent (morph) state.
- **Why:** with geomorphing each vertex carries two states (80 bytes), and SSE 16 peaks at ~2.4 GB
  of terrain VRAM against a 2 GB budget. A compact format should bring it to roughly 32 bytes.
- **Cost / risk:** custom vertex layout and shader decoding; precision of 16-bit positions over
  large patches (coarse levels may need 32-bit); must keep the morph contract (a fully morphed
  child equals its parent) within quantisation.
- **Decision:** accepted at the end of E3: the terrain VRAM budget cannot be met at the accepted
  default (SSE 16 with morphing) without it.
