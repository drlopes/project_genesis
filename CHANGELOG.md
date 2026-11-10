# Changelog

All notable changes to this project. Format: [Keep a Changelog](https://keepachangelog.com/).
Versions follow roadmap phases: Phase 0 → `0.1.0`, Phase 1 → `0.2.0`, … (see
[work/README.md](work/README.md#changelog-rules)).

## [Unreleased]

### Added
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
- Build artefacts accidentally committed under `spikes/target/` removed from the tree; `target/` is
  ignored at any depth. (Still present in the history of `61e6dc6`.)

### Docs
- E1 results: field cost passes its budgets; cost is driven by noise and repeated `h0`
  evaluations, not content count; craters need a spatial index; generation workers = physical
  cores.
- E2 results: generation is bit-identical across threads, order, build profile, and OS.
- Tech stack decided: Rust + Bevy 0.20, PC only; generation crates never depend on Bevy (D42);
  cross-platform determinism rules (D43).
