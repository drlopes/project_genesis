# Changelog

All notable changes to this project. Format: [Keep a Changelog](https://keepachangelog.com/).
Versions follow roadmap phases: Phase 0 → `0.1.0`, Phase 1 → `0.2.0`, … (see
[work/README.md](work/README.md#changelog-rules)).

## [Unreleased]

### Added
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
