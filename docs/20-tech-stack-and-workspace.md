# 20 — Tech Stack & Workspace

This document records **what we build with** and **how the code is organised**. The architecture in
docs 00–19 is engine-agnostic; this is where it meets a concrete language and engine.

`DECIDED` — **Rust** is the language and **Bevy** is the engine. Target platform: **PC only**
(Windows first, Linux welcome). No console plans.

---

## Why Rust and Bevy

The project is unusually data-driven, and it needs a lot of tooling around the game: validator,
inspector, diversity audit, golden-seed tests, benchmarks. That shaped the choice more than raw
engine features did.

| Need | How the stack meets it |
|---|---|
| Content is data, schemas are contracts ([16](./16-data-contracts.md), [17](./17-biome-and-landform-format.md)) | `serde` + TOML gives typed loading; the validator is generated from the same Rust types, so data and code cannot drift |
| Fast tuning loop ([12](./12-authoring-and-tuning.md)) | Bevy asset hot-reload for data files; tuning rarely needs a recompile |
| Bit-stable generation ([11](./11-determinism-and-sampling.md)) | no GC, no implicit fast-math, explicit control over float maths (see [Determinism rules](#determinism-rules)) |
| Parallel, order-independent chunk generation | pure functions plus `rayon` / Bevy's task pools |
| Game and tools share one codebase | the generator is ordinary crates; every tool links the same code the game runs |
| AI-agent-friendly workflow | text-only, `cargo build` / `cargo test`, precise compiler errors, no editor to restart |
| Freedom to build planet-scale systems | Bevy imposes no world model; cube-sphere LOD, voxels, and the ocean shell are ours anyway |

Engines that were considered and rejected: **Unreal** (heavy, slow iteration, poor fit for
agent-driven work, provides much we would replace), **Unity** and **Godot** (same general-purpose
trade-off, not tried). **No engine** was rejected because Bevy's renderer, ECS, asset system, and
windowing are worth having, and the [boundary rule](#the-boundary-rule) keeps us from being locked
in.

### Known risks

| Risk | Mitigation |
|---|---|
| Bevy is pre-1.0, with breaking releases every few months | pin a minor version; upgrade deliberately (see [Upgrade policy](#upgrade-policy)); keep Bevy-facing code thin |
| Third-party Bevy crates lag behind releases | prefer few plugins; anything world-defining is our own code |
| Rust compile times | crate split, dev-profile tuning, dynamic linking in dev, data hot-reload |
| Renderer less mature than Unreal's | aesthetic-first art direction (D2); custom shaders where needed |
| Learning curve (Rust syntax, ownership, ECS) | conventions below; generator code is plain functions over plain data |

---

## The boundary rule

> **The generator never depends on Bevy.**

Everything specified in docs 03–19 (descriptor, bake, field stack, classification, collision
field, edit records, content loading, validation) lives in crates with **no Bevy dependency**. Bevy
appears only in the crates that display, stream, simulate, and take input.

Consequences:

- A Bevy upgrade touches only the outer layer.
- Tools and tests run without a window or a GPU.
- If Bevy ever fails us, the world generation survives; only the presentation layer is replaced.
- The engine-free benchmarks in the Phase 1 spike can run from day one.

`DECIDED` — generation crates do not depend on Bevy. Enforced by `cargo` dependency checks in CI.

---

## Workspace layout

```text
genesis/
├── Cargo.toml              # workspace: members, shared dependency versions, profiles
├── Cargo.lock              # committed; reproducible builds
├── rust-toolchain.toml     # pinned stable toolchain
├── crates/
│   ├── genesis_core        # math wrappers, units, frames, seeds, counter RNG, noise
│   ├── genesis_content     # content schemas, pack loading, extends/overlay resolution, validation
│   ├── genesis_gen         # bake (DescriptorCore + caches) and query (L0–L4, 19, water, collision field)
│   ├── genesis_edits       # edit records, ordering, application (18)
│   ├── genesis_cli         # validate, audit, golden seeds, bench, descriptor dump
│   ├── genesis_game        # Bevy app: streaming, rendering, physics, gameplay
│   └── genesis_inspector   # Bevy + egui planet inspector
├── data/
│   └── core/               # the core content pack (layout in 16)
├── docs/
├── spikes/                 # throwaway experiments (separate workspace, shared target/)
├── tools/
│   ├── doc-checks/
│   └── clean.ps1
├── .github/workflows/ci.yml
├── .vscode/settings.json
└── AGENTS.md               # working habits for humans and agents
```

Dependency direction (arrows point at dependencies):

```text
genesis_game ─┐
genesis_inspector ─┼──► genesis_edits ──► genesis_gen ──► genesis_content ──► genesis_core
genesis_cli ──┘                                 └───────────────────────────► genesis_core
```

| Crate | May depend on Bevy | Owns |
|---|---|---|
| `genesis_core` | no | `H(...)`, counter RNG, band-limited noise, deterministic math, units |
| `genesis_content` | no | everything in [16](./16-data-contracts.md) and [17](./17-biome-and-landform-format.md) |
| `genesis_gen` | no | [03](./03-planet-descriptor.md), [05](./05-climate-and-hydrosphere.md), [06](./06-geology-and-landforms.md), [09](./09-surface-field-stack.md), [13](./13-generator-modules.md), [15](./15-aquatic-environments.md), [19](./19-biome-classification.md), the solid model in [18](./18-collision-and-edits.md) |
| `genesis_edits` | no | the edit half of [18](./18-collision-and-edits.md) |
| `genesis_cli` | no | the tools in [11](./11-determinism-and-sampling.md) and [12](./12-authoring-and-tuning.md) that need no window |
| `genesis_game` | yes | [10](./10-lod-and-streaming.md) streaming and rendering, physics, gameplay |
| `genesis_inspector` | yes | the visual tools in [12](./12-authoring-and-tuning.md) |

`PROPOSED` — the crate names and the split between `genesis_gen` and `genesis_edits`. Split
further when a crate's compile time hurts, not before.

Built so far (2026-11-14):
- `genesis_core`;
- `genesis_content` (pack loading, `extends`, validation, content digest; P1-B);
- `genesis_gen` (rocky bake, P1-A);
- `genesis_cli`, binary `genesis`, with the `descriptor` and `validate` commands.

`genesis_gen` does not depend on `genesis_content` yet: the bake reads content from P1-C.
`toml` is a workspace dependency for content parsing only. `serde`, `serde_json` and `thiserror` are workspace
dependencies; they are allowed in generation crates because identity never depends on them (the
canonical encoding in [03](./03-planet-descriptor.md#core-versus-caches) does). `genesis_gen`
carries the same `clippy.toml` as `genesis_core`.

---

## Determinism rules

These turn [11](./11-determinism-and-sampling.md) into code rules for the engine-free crates.

1. **No platform transcendental maths in generation.** Rust's `f32::sin`, `cos`, `exp`, `powf`,
   etc. call the platform's maths library, whose last bits can differ between Windows and Linux.
   Generation code uses `genesis_core::math`, which wraps the pure-Rust `libm` crate. Clippy's
   `disallowed_methods` lint bans the `std` versions in `genesis_core`, `genesis_content`, and
   `genesis_gen`.
2. **World-defining code has no third-party dependency.** Hashing (`H`), the counter RNG, and noise
   are implemented in `genesis_core`. A minor release of a noise crate could otherwise change every
   world's terrain. `libm` is the one exception: pinned, and covered by golden tests.
3. **No fast-math, no FMA by choice.** Do not enable target features or flags that change float
   results. Basic `+ − × ÷` and `sqrt` are IEEE-exact and safe.
4. **Ordered iteration.** Never iterate a `HashMap` where order affects results. Use `BTreeMap`,
   sorted `Vec`s, or `IndexMap` with a defined insertion order. Content IDs are sorted before
   accumulation ([19](./19-biome-classification.md#invariants)).
5. **No hidden state.** No `static mut`, no thread-locals, no global RNG in generation crates.
   Everything comes from the descriptor and the seed chain.
6. **Golden tests in CI on Windows and Linux.** Descriptor hashes and a fixed set of sample hashes
   must match on both. A mismatch blocks the merge.

`DECIDED` — rules 1–6. Cross-platform bit-identity is required for generation, even PC-only,
because golden seeds and save compatibility depend on it.

---

## How Bevy is used

The Bevy side is a thin layer that drives the generator:

| Concept | Bevy shape |
|---|---|
| A loaded body (descriptor + caches) | an asset / resource, shared read-only with generation tasks |
| A streamed patch | an entity with patch-ID, LOD band, and mesh components |
| Patch generation | an async task on Bevy's compute pool calling `genesis_gen`; results become components |
| Scatter instances | batched instance buffers per patch, not one entity per pebble |
| The ocean shell, atmosphere | custom materials / shaders |
| Content packs | Bevy assets with hot-reload; on reload, re-resolve content and rebake in the inspector |
| Floating origin | our own camera-relative render frame (f64 positions, f32 render translations from f64 differences); `big_space` not used ([E5](./spikes/E5-coordinate-precision.md)) |

`PROPOSED` — the table above. The spike validates it.

Physics: collision against terrain uses **our** collision field ([18](./18-collision-and-edits.md)).
The physics engine is **Avian 0.8** (`avian3d`, features `3d`, `parry-f32`, `xpbd_joints`,
`parallel`), chosen in E6. Both candidates passed the character and fall-through tests; Avian wins
because a body resting on the terrain mesh stays exactly still once asleep, where Rapier crept
0.3–1.5 m before sleeping — visible as props sliding down slopes. Full comparison and numbers:
[E6](./spikes/E6-collision-and-physics.md).

- Terrain collision is **per-patch triangle meshes built from the displayed patch's vertices**, at
  the visual LOD near a physics body ([collision LOD](./18-collision-and-edits.md#collision-lod)).
  Neither crate accepts the field as a custom collider: Avian's `AnyCollider` replaces the whole
  collider type, and `bevy_rapier` exposes no contact dispatcher.
- Colliders are built a few per frame, and added before the old ones are removed. Rebuilding a
  whole block at once cost 16–39 ms.
- The character moves in a local physics frame (f64 origin, f32 inside) and stays on the ground by
  a ground snap, never by a commanded downward velocity
  ([walking on a heightfield](./18-collision-and-edits.md#walking-on-a-heightfield)).
- Known limits: violent body–body impacts on the ground occasionally let a body through Avian's
  terrain (6 per 1 000 at 300 m/s); dynamic bodies creep ~0.4–1.0 m on mesh contacts if never
  allowed to sleep. Mitigations are listed in E6.
- The generator crates never depend on the physics crate, as with Bevy (D42).

`DECIDED` — Avian, from E6 (measured, plus the user's feel check).

---

## Toolchain and build settings

- **Rust:** stable, pinned in `rust-toolchain.toml`, edition 2024. Bump deliberately, like Bevy.
- **Bevy:** pinned to **0.20** (`bevy = "0.20"` allows only 0.20.x patch releases).
- **`Cargo.lock` is committed.** The game is an application; builds must be reproducible.
- **Dev profile:** optimise dependencies (`opt-level = 3` for `[profile.dev.package."*"]`) and
  lightly optimise our own code (`opt-level = 1`). Unoptimised Bevy is too slow to test with.
- **Dev iteration:** static Bevy linked with **`rust-lld`** on Windows, set by name in the
  repository's `.cargo/config.toml` (it ships with the toolchain; nothing to install). Measured in
  [E9](./spikes/E3-lod-and-quadtree.md#e9--build-iteration): a one-line change rebuilds in
  ~6 s in either the app or the generation crate, against 6.7–14.9 s with the default MSVC linker.
  Bevy's `dynamic_linking` is unavailable on 0.20.1 (`bevy_dylib 0.20.1` is not published);
  revisit at the next upgrade (I-007). Changing the linker forces one full rebuild (~6 min).
- **Release profile:** full optimisation, LTO evaluated in the spike.
- **Formatting and lints:** `rustfmt` and `clippy` are mandatory and run in CI. Warnings are
  errors in CI.
- **One `target/` directory.** The `spikes/` workspace builds into the root `target/`
  (`spikes/.cargo/config.toml`), so dependencies are built once.
- **Keep build data small.** `tools/clean.ps1` prunes incremental caches (`-All` does a full
  `cargo clean`); run it when a task is done. `.vscode/settings.json` excludes `target/` and
  `node_modules/` from watching, search, and the explorer, and gives rust-analyzer its own
  subfolder of `target/`. The working habits are in `AGENTS.md`.

---

## Testing

| Kind | Tool | Covers |
|---|---|---|
| Unit | `cargo test` | maths, seeds, parsers, single functions |
| Property | `proptest` | the invariants in [09](./09-surface-field-stack.md), [05](./05-climate-and-hydrosphere.md), and [19](./19-biome-classification.md) (continuity, normalisation, zero-stays-zero) |
| Golden | `genesis_cli golden` | descriptor and sample hashes for the golden seeds |
| Content | `genesis_cli validate` | every pack, in CI |
| Benchmark | `criterion` | per-layer sample cost, bake time |
| Docs | `tools/doc-checks` | TOML blocks, links, anchors |

`DECIDED` — CI on GitHub Actions (`.github/workflows/ci.yml`), on every push to `main` and every
pull request: Windows and Linux runners run fmt, clippy (warnings are errors), tests, and the E2
determinism check against recorded hashes in release and debug; a separate job runs the doc checks.
Content validation and production golden tests join the same job when those crates exist.

---

## Upgrade policy

- **Bevy:** upgrade one minor version at a time, on a dedicated branch, after its ecosystem crates
  (egui integration, physics) have caught up. Golden tests must pass unchanged. If they don't, the
  boundary rule was violated somewhere.
- **Rust toolchain:** bump with the same branch discipline; run golden tests on both platforms.
- **Generation-affecting dependencies** (`libm`): treat any version change as a potential
  `generatorVersion` change and confirm with golden tests.
- Never upgrade in the same change as feature work.

---

## Conventions for a Rust newcomer

The generation crates are deliberately plain: **data structures and pure functions**. Most of the
harder Rust features are only needed at the Bevy layer.

- **Prefer owned data and plain structs.** A `DescriptorCore` is a struct with `serde` derives. Pass
  `&Descriptor` to query functions; never mutate it.
- **Errors:** library crates return `Result<T, E>` with a typed error enum (`thiserror`). Tools and
  the game use `anyhow` at the top level. No `unwrap()` outside tests and truly impossible cases.
- **Newtypes for units.** `Metres(f32)`, `Kelvin(f32)`, `ContentId`, `BodyId`. The compiler then
  enforces the unit rules from [16](./16-data-contracts.md#units-and-frames) and the
  height-vs-altitude distinction.
- **Numbers:** `f32` for surface-local fields, `f64` for system and galaxy positions, exactly as in
  [01](./01-world-model.md#coordinate-spaces).
- **No `unsafe`** in generation crates.
- **Docs in code link back to docs here** (`/// See docs/19-biome-classification.md#9-normalisation`),
  so an agent or a newcomer can follow a function to its specification.

---

## Related

- [01 — World Model](./01-world-model.md) — coordinate spaces and precision.
- [10 — LOD & Streaming](./10-lod-and-streaming.md) — what the Bevy layer streams.
- [11 — Determinism & Sampling](./11-determinism-and-sampling.md) — what the determinism rules
  protect.
- [12 — Authoring & Tuning](./12-authoring-and-tuning.md) — the tools these crates host.
- [13 — Generator Modules](./13-generator-modules.md) — the generator contract, now Rust traits.
- [14 — Open Questions & Roadmap](./14-open-questions-and-roadmap.md) — Phase 0 sets up this
  workspace.
- [16 — Data Contracts](./16-data-contracts.md) — the schemas `genesis_content` implements.
