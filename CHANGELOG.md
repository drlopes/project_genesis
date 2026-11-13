# Changelog

All notable changes to this project. Format: [Keep a Changelog](https://keepachangelog.com/).
Versions follow roadmap phases: Phase 0 → `0.1.0`, Phase 1 → `0.2.0`, … (see
[work/README.md](work/README.md#changelog-rules)).

## [Unreleased]

### Changed
- **Classification look defaults decided (E8)**: *k* 4 and jitter 1× confirmed; mosaic strength
  **0.35 → 0.9**. docs/19 now records the mosaic noise's measured range (one field sd 0.27, a
  two-field difference sd 0.38) and the rule that sizes mosaic strength against the palette
  spread; the worked example is recomputed. The palette draw's spread is `PROPOSED`.
- **Phase 1 spike complete**: docs/21 exit criteria met; budget table filled (edit → remesh
  23–31 ms, voxel RAM, L0–L2 at palette 8); docs/18 records E6's `interactionRadius` of 40 m for a
  walking character (vehicle radii `OPEN`).
- **Voxel numbers decided (Q2 → D44)**, from E7: 0.5 m voxels (the coarsest that passed the
  user's look test from 2 m), a ±64 m region, surface nets in 32³ bricks, volume edits persisted as
  records and replayed. docs/18 gains the chunk ownership and winding rules, on-demand bricks,
  dirty-set rule, the record-based persistence and GC policy, and the rule that non-radial solids
  must be enclosed by a voxel region. docs/10, docs/14, docs/16, docs/21 updated.
- **Voxels only where edited** (I-014, accepted; refines D27): unedited ground near the player stays
  heightfield, and a chunk switches to voxels on its first edit. Keeps voxel builds off the path of
  fast movement near the surface. New idea I-015: races near the surface, and what they need from
  terrain streaming.
- **Physics engine decided: Avian 0.8** (docs/20), from E6: both candidates passed, Avian keeps
  resting bodies exactly still on terrain meshes. Terrain collision is per-patch triangle meshes
  built from the displayed patches, a few per frame; the character walks in a local physics frame.
  docs/18 now specifies walking on a heightfield (ground snap, no movement without input, 50°
  walkable limit, LOD changes move a standing character only along the up axis).
- E6 closed: the user's feel check passed ("works flawlessly; no glaring issues").

### Fixed
- B-005: the spike field's crater term had a vertical wall at every rim (the raised rim started at
  `0.2 · depth` where the bowl ended at 0, up to 60 m). The rim profile is now continuous. Found in
  E7 as a 12 m "seam outlier". The E2 reference sample hash is re-pinned deliberately for it
  (`spikes/field/e2-expected.txt`).
- E6: the walking character slid downhill forever, even with no input. It commanded a downward
  velocity while grounded, which the slide query projected onto the slope. Contact now uses a ground
  snap (downward cast, ≤ 0.5 m) and the movement solver is not run at all when there is no input, so
  idle drift is exactly zero (measured over five idle phases on ground up to 20°).
- B-003: the spike's free-flight camera flipped over the poles, crawled at 2 m/s below the
  reference sphere, and morphing patches could be frustum-culled while visible. Orientation now
  uses parallel transport (`spikes/app/src/flycam.rs`), speed is measured from the ground, and
  culling boxes cover both morph shapes.

### Added
- `spikes/field` `e8` binary (E8): top-down biome maps (separated and `--overlap` content), border
  width, patch sizes, mosaic noise statistics, and L2 cost per setting. `Body::jitter` scales the
  jitter amplitudes (exactly 1.0 by default; E2 hash unchanged) and
  `Content::set_mosaic_strength` sets the mosaic.
- `spikes/voxel` (E7): voxel regions in 32³ bricks around a site, surface nets and a blocky
  baseline, sphere/cylinder edits with on-demand bricks, an edit-record log with bit-exact replay,
  and metrics for RAM, remesh latency, seam against the heightfield (including holes), record
  storage and no-ops. `spike_app --bin voxel_view`: the look-test viewer (voxel size 1/2/3,
  blocky B, edits E, tint T, `--shot`).
- E7 results ([docs/spikes/E7-voxels-and-edits.md](docs/spikes/E7-voxels-and-edits.md)); ideas
  I-010 (non-radial solids), I-011 (edits visible from space), I-012 (floating debris), I-013
  (voxel LOD at distance), I-014 (voxelise on first edit).
- `spikes/physics` (E6): headless Avian 0.8 tests for capsule drops, raycasts (engine versus our
  field sphere trace), and resting bodies, with per-site local physics frames and a naive control.
  `spike_app` is now a library plus the app binary, so the physics tests reuse the patch builder.
- E6 character test in `spikes/physics` (both engines, same controller logic: 0 fall-through, 0
  snags, ~0.5 ms p99 per step) and in the app: Avian walking with colliders built from the
  displayed patches near the player, add-before-remove, at most 4 per frame, in a local physics
  frame; **C** toggles walking; `--auto-walk` measures idle drift, ground gap, and step cost.
- B-004 fixed (docs/18 sphere-trace formula): `g(p) = |p| − surfaceRadius`, and the bisection must
  bracket a sign change.
- E3 geomorphing in `spikes/app`: custom terrain material and WESL shader; each vertex carries its
  parent level's position, normal, and colour, and patches morph over 0.5 s on split and merge
  (`--morph-s`). Debug keys M/G/L/P, `--shots` screenshots during the scripted fly-in. Fixed: patches
  shown in the frame they were integrated never became visible; skirt triangles back-face culled.
- E3 spike app (`spikes/app`, Bevy 0.20.1): cube-sphere quadtree with screen-space-error selection
  and horizon culling, patch meshing on a fixed worker pool with a re-prioritised queue, hole-free
  display, a scripted fly-in that writes JSON summaries, per-frame CSV, peak-VRAM sampling, and a
  `valid` flag for runs disturbed by window focus.
- `spike_field::Body::bake_with_radius` for bodies of any radius (E2 hashes unchanged).
- Ideas I-004 (view-dependent LOD), I-005 (compressed vertices), I-006 (per-patch overhead),
  I-007 (Bevy `dynamic_linking` at the next upgrade).
- Work structure: `work/` with process rules, `NOW.md` (one active goal), `BUGS.md`, and
  `IDEAS.md`; this changelog.
- Research library and knowledge base: `research/sources/` (one card per source, committed),
  `research/library/` (local raw files, never committed), and `knowledge/` (distilled notes).
  First cards and a noise/hashing note covering the techniques already used in `genesis_core`.
- `art/` for concept art, stored with Git LFS; `art/references/` for third-party images, never
  committed.
- E2 determinism harness (`spikes/field`, `e2`): 10 golden seeds hashed across thread counts,
  shuffled orders, and build profiles; checked against recorded hashes.
- CI on GitHub Actions, Windows and Linux: fmt, clippy (warnings are errors), tests, E2
  determinism in release and debug, and doc checks.
- `tools/clean.ps1` to prune build bloat; `AGENTS.md` with working habits; VS Code settings that
  exclude build output from watching, search, and indexing.
- E1 field-cost harness (`spikes/field`, `e1`): a cost-faithful synthetic body with the L0–L4 stack
  and the docs/19 classifier.
- `genesis_core::noise`: seeded, band-limited 3D gradient noise and fBm, normalised by the full
  amplitude sum so band-limiting never rescales the coarse shape.
- `genesis_core`: seed chain (`Seed::derive`, `derive_idx`, `derive_path`, `derive_named`),
  counter-based RNG, and `libm` maths wrappers; clippy bans `std` transcendental maths and FMA in
  generation code. Known-answer tests pin the seed chain and noise output.
- Cargo workspace with pinned Rust 1.99.0, shared lints, and dev-profile optimisation.
- `tools/doc-checks`: TOML-block parsing, fences, links, and GitHub-compatible anchors.
- Architecture docs 00–21 (vision through the Phase 1 spike plan).

### Changed
- E5 harness in `spikes/app`: camera position kept in f64 body-local coordinates; `--frame
  relative|naive` and `--system-distance`; `precision.rs` emulates the GPU's f32 vertex path.
- E4 harness in `spikes/app`: `--path straight` runs (hold, then constant speed and height),
  streaming metrics (pending, deficit, latency, requests/completions, backlog trend, initial fill),
  `E4_HOLE_LOG` diagnostic. Default SSE is now 16 (was 2); the integration loop always integrates
  at least one patch per frame.
- I-005: compact patch vertex format, 32 bytes per vertex instead of 80 (16-bit positions in a
  per-patch box used as the entity transform, octahedral normals, gamma-2 RGBA8 colours, u16
  indices). Terrain mesh data at SSE 16: 331 MB, from 783 MB; peak VRAM above baseline 1.34–1.39 GB,
  from 2.44 GB, within the 2 GB terrain budget. The harness now reports resident terrain mesh bytes.
  User check: no visible artefacts; I-005 closed.
- Windows builds link with `rust-lld` (repository `.cargo/config.toml`): ~6 s incremental builds,
  against 6.7–14.9 s with the default linker (E9).
- Spikes workspace dev profile matches the root: dependencies optimised, own code `opt-level = 1`.
- Doc checks cover every Markdown file git tracks (root, `docs/`, `work/`, `research/`,
  `knowledge/`, `art/`), not only `docs/`.
- `genesis_core` noise: one hash mix per lattice corner and a 16-entry gradient table; 8.5× faster
  (119 → 14 ns). Known answers re-pinned; no world existed yet.
- The spikes workspace builds into the root `target/`.
- Doc checks recurse into subfolders and resolve links relative to each file.
- LOD bands are spacing-defined and body-independent; quadtree depth is per body (D37).
- Biome classification rewritten as docs/19: power normalisation, soft budget, ε fallback, stratum
  blending, variant families, ecotones (D38–D41).

### Fixed
- An anchor in docs/05 that resolved locally but not on GitHub; the checker now uses GitHub's slug
  rules.
- Build artefacts accidentally committed under `spikes/target/` (11 MB) removed from the tree and,
  by a history rewrite and force-push on 2026-11-10, from history; `target/` is ignored at any
  depth. Commits from the E1 harness onward have new hashes; docs cite both where it matters.

### Docs
- E5 done (user check: relative frame clean at 10¹⁰ m, naive control jitters at 10⁶ m): our own camera-relative frame passes at 10¹⁰ m
  (emulated error ≤ 0.0014 px); f32 system positions fail from 10⁶ m. `big_space` not adopted
  (pre-release only for Bevy 0.20). Rule in docs/10: render translations are f64 differences cast
  to f32. Resting-body physics test moved to E6.
- E4 results: pass at rover speed with 4 workers; full terrain detail while speed / height above
  ground ≤ ~6 per second (docs/01 travel constraint, docs/10); saturation degrades detail, never
  coverage; generation workers must never take all cores (docs/10, superseding E1's rule); the
  integration budget must guarantee progress. Ideas I-008 (corrected), I-009 (new); I-001, I-002,
  I-004, I-006 stay parked.
- E3 closed: default SSE 16; skirts plus time-based geomorphing decided in docs/10 (skirts alone
  failed the visual check; morphing passed). The compressed vertex format (I-005) is accepted and
  is the next goal, because morph data puts terrain VRAM at ~2.4 GB. I-004 and I-006 parked until
  E4. docs/21 status: E3 done.
- E3 results (pending the user's visual check): 65 × 65 patches; the error metric's useful range is
  8–16 px, not the planned 1–4; SSE 16 meets every budget, SSE 8 exceeds terrain VRAM (~2.7 GB)
  without compressed vertices; body size does not change the picture. Four streaming rules from
  spike bugs written into docs/10. Budget table in docs/21 filled with E3 measurements.
- E9 results: Bevy `dynamic_linking` is unavailable on 0.20.1 (`bevy_dylib 0.20.1` unpublished);
  static linking with `rust-lld` meets both build-time targets.
- Phase 1 budget table reviewed and decided: per-frame budgets apply to the main thread only;
  worker generation is a throughput target; added a worst-frame bound (≤ 33 ms while streaming),
  total RAM (≤ 6 GB) and VRAM (≤ 6 GB) caps, and a build-time target for the Bevy app crate.
  docs/10 frame-budget targets corrected to match.
- E1 results: field cost passes its budgets; cost is driven by noise and repeated `h0`
  evaluations, not content count; craters need a spatial index; generation workers = physical
  cores.
- E2 results: generation is bit-identical across threads, order, build profile, and OS.
- Tech stack decided: Rust + Bevy 0.20, PC only; generation crates never depend on Bevy (D42);
  cross-platform determinism rules (D43).
