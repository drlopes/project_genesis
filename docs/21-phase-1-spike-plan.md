# 21 — Phase 1 Spike Plan

A **spike** is a short, deliberately throwaway experiment that replaces a guess with a
measurement. The docs contain numbers marked "starting point" or "set by the Phase 1 spike". This
plan lists each one, the experiment that settles it, and the pass criteria.

The spike does not try to make numbers "right" in general. It finds values that hold up on the
reference hardware, and it tests whether Rust + Bevy ([20](./20-tech-stack-and-workspace.md)) carries
this architecture without fighting it.

---

## Scope and exit

**In scope:** generation cost, determinism, LOD and quadtree, streaming throughput, coordinate
precision, collision and physics, voxel editing, classification cost and look, build iteration time.

**Out of scope:** content quality, art direction, gameplay, the aquatic slice (Phase 1b), and
anything about the galaxy beyond one system.

**Exit criteria** — the spike is done when:

1. every experiment below has a recorded result and a decision;
2. Q2 (voxel numbers) is closed in [14](./14-open-questions-and-roadmap.md);
3. the physics crate and the floating-origin approach are chosen;
4. the [budget table](#reference-hardware-and-budgets) is filled with measured values;
5. every number in the [write-back table](#write-back) is updated in its owning doc.

Each experiment has a **stop rule**. If it is hit, record the failure and the best value found,
and move on; a spike that never ends is a design meeting in disguise.

---

## Reference hardware and budgets

| | Reference (development machine) |
|---|---|
| CPU | AMD Ryzen 7 9800X3D (8 cores / 16 threads) |
| RAM | 32 GB |
| GPU | NVIDIA GeForce RTX 4060 (8 GB) |
| OS | Windows 11 |
| Display target | 1440p, 60 fps |

The GPU is a reasonable mid-range reference. The CPU is high-end, so every CPU-bound measurement is
also run with the **generation thread pool capped at 4 workers**, to approximate a mid-range CPU.
A result counts only if it passes under the cap.

Initial budgets, to be confirmed or revised by the spike:

| Budget | Target | Measured |
|---|---|---|
| Frame time p99 | ≤ 16.6 ms | — |
| Chunk generation, amortised per frame (from [10](./10-lod-and-streaming.md#frame-budget-targets)) | ≤ 2 ms | — |
| Mesh building, amortised per frame (from 10) | ≤ 2 ms | — |
| LOD update per frame (from 10) | ≤ 1 ms | — |
| Terrain + scatter VRAM | ≤ 2 GB | — |
| Terrain + scatter + voxel RAM per resident body | ≤ 1.5 GB | — |
| Full L0–L4 sample, single thread | ≤ 4 µs | — |
| L0–L2 sample, single thread | ≤ 1 µs | — |
| Body bake (core + caches) | ≤ 2 s on worker threads | — |
| Edit → remesh visible | ≤ 50 ms | — |
| Incremental build after a one-line change in `genesis_gen` | ≤ 15 s | — |

`PROPOSED` — these targets. The user may revise them before the spike starts.

---

## Shared harness

The spike needs just enough real code to measure honestly.

| Piece | Lives in | Kept after the spike? |
|---|---|---|
| Hash `H`, counter RNG, `libm` maths wrappers, band-limited noise | `genesis_core` | **yes** — production code from day one |
| Synthetic body: macro noise + plate-like ridges, a sea level, `N` stub suites | `spikes/field` | no |
| The real classifier from [19](./19-biome-classification.md) over a stub content pack | `spikes/field` | partly; the algorithm moves to `genesis_gen` |
| A Bevy app streaming cube-sphere patches, with a fly camera and scripted paths | `spikes/app` | no |
| Benchmarks (`criterion`) and golden hashes | `spikes/bench` | the golden-test pattern is kept |

Spike code lives under `spikes/` and is **not** a workspace member of the production crates. It may
be ugly; it must be measurable. Anything worth keeping is rewritten properly in its production
crate.

The stub body uses `bulk.radius = 24 km` unless an experiment says otherwise. Experiments that
depend on body size also run at **8 km** and **40 km**, pending Q4.

---

## Experiments

Run in this order: E1 and E2 need no engine and gate everything else.

### E1 — Field cost

| | |
|---|---|
| **Question** | What does one sample cost per layer, and how does it scale with content count? |
| **Candidates** | palette size 8 / 16 / 32 biomes; active suites 2 / 4 / 8; *K* = 4 / 8 |
| **Setup** | `criterion` benchmark of `sampleSurface` at each LOD band over 100 k random positions; one thread |
| **Metrics** | ns per sample per layer (L0, L1, L2, L3, L4); patch time for a 65×65 patch |
| **Pass** | within the sample budgets above at palette 16, suites 4, *K* 4 |
| **Stop rule** | if over budget by > 3× after one optimisation pass, record it and reduce defaults (*K*, palette cap, suites) |
| **Records into** | [09](./09-surface-field-stack.md#performance-budget), [19](./19-biome-classification.md#tuning-constants) |
| **Who runs it** | Claude, in this environment |

### E2 — Determinism

| | |
|---|---|
| **Question** | Is generation bit-identical across threads, run order, build profile, and OS? |
| **Setup** | hash 1 M samples for 10 golden seeds; thread counts 1 / 4 / 16; shuffled order; debug and release; Windows and Linux (CI runner or WSL) |
| **Metrics** | hash equality; `libm` versus `std` maths cost |
| **Pass** | all hashes identical; `libm` overhead recorded |
| **Stop rule** | any mismatch blocks the spike until explained |
| **Records into** | [11](./11-determinism-and-sampling.md#golden-seeds), [20](./20-tech-stack-and-workspace.md#determinism-rules) |
| **Who runs it** | Claude (Windows); CI or WSL for Linux |

### E3 — LOD bands and quadtree

| | |
|---|---|
| **Question** | Which band spacings, patch grid, and screen-space error give stable frames without visible popping? |
| **Candidates** | grid 33 / 65 / 129; band spacings at 0.5× / 1× / 2× the values in [10](./10-lod-and-streaming.md#lod-levels-and-budgets); screen-space error 1 / 2 / 4 px; skirts versus morphing |
| **Setup** | Bevy app; a scripted fly-in from high orbit to standing on the ground, repeated per candidate; 8 / 24 / 40 km bodies |
| **Metrics** | frame time p50/p99, resident patches, triangles, VRAM; cracks; popping rated by the user from recordings |
| **Pass** | p99 ≤ 16.6 ms; no cracks; popping rated acceptable |
| **Stop rule** | if skirts fail visually, try morphing once; then record and move on |
| **Records into** | [10](./10-lod-and-streaming.md), [16](./16-data-contracts.md#deriving-lod-bands-from-feature-scale) |
| **Who runs it** | Claude builds and scripts it; the user runs it and judges the visuals |

### E4 — Streaming throughput

| | |
|---|---|
| **Question** | How fast can the player move near the surface before detail falls behind? |
| **Candidates** | speeds 5 (walk), 30 (rover), 200 (low flight), 1000 m/s (fast pass) at 50 m and 500 m altitude; 4 and 8 workers |
| **Setup** | same app; straight-line runs; patch requests versus completions per second |
| **Metrics** | generation backlog over time; time for the LOD-4 ring to fill; frames with missing patches |
| **Pass** | backlog stays bounded at rover speed with 4 workers; the result defines the speed cap for low flight |
| **Stop rule** | none needed; the output is a curve, not a pass/fail |
| **Records into** | [10](./10-lod-and-streaming.md), [01](./01-world-model.md#travel-time-as-a-dial) |
| **Who runs it** | Claude builds; the user runs |

### E5 — Coordinate precision

| | |
|---|---|
| **Question** | Does the floating-origin approach hold at system-scale distances? |
| **Candidates** | `big_space` crate versus our own camera-relative frame |
| **Setup** | stand and walk on a body placed 10⁸, 10⁹, and 10¹⁰ m from the system origin; orbit it; physics active |
| **Metrics** | vertex jitter on screen; camera jitter; physics stability (a resting body must stay at rest) |
| **Pass** | no visible jitter at 10¹⁰ m; resting bodies stay at rest for 10 minutes |
| **Stop rule** | if neither candidate passes, record the distance where it fails; that caps system size |
| **Records into** | [01](./01-world-model.md#precision-rules), [10](./10-lod-and-streaming.md#coordinate-precision), [20](./20-tech-stack-and-workspace.md#how-bevy-is-used) |
| **Who runs it** | Claude builds; the user verifies visually |

### E6 — Collision and physics

| | |
|---|---|
| **Question** | Which physics crate accepts our field as terrain collision, at what cost, with no fall-through? |
| **Candidates** | Avian, Rapier; each with (a) per-patch heightfield colliders built from the field, (b) a custom shape that queries the field directly |
| **Setup** | 1000 drops of a capsule at 0–300 m/s onto mixed terrain; a character walking and jumping across patch seams and LOD changes; 10 k raycasts per frame |
| **Metrics** | fall-through count; physics ms per frame; raycast throughput; seam snagging |
| **Pass** | zero fall-through; physics ≤ 1 ms/frame in the walking test; no snagging at seams |
| **Stop rule** | if neither crate accepts a custom field collider cleanly, use (a) and record the cost |
| **Records into** | [18](./18-collision-and-edits.md#collision-lod), [20](./20-tech-stack-and-workspace.md#how-bevy-is-used) |
| **Who runs it** | Claude builds and runs the headless tests; the user tries the character feel |

### E7 — Voxels and edits (Q2)

| | |
|---|---|
| **Question** | What voxel size and voxel-region radius give believable digging within budget? |
| **Candidates** | voxel size 0.25 / 0.5 / 1 m; region radius 32 / 64 / 128 m; surface nets versus marching cubes |
| **Setup** | a fixed test site; scripted edits (flatten, tunnel, cliff carve, 500 small digs); the voxel/mesh overlap from [18](./18-collision-and-edits.md#the-voxelmesh-boundary) |
| **Metrics** | RAM per region; edit → remesh latency; seam step at the voxel/mesh boundary; largest single edit radius within the frame budget; edit-record storage per 1000 edits; garbage-collection policy for superseded edits |
| **Pass** | remesh ≤ 50 ms; RAM within budget; no visible seam from 2 m |
| **Stop rule** | pick the coarsest size that passes the look test |
| **Records into** | [10](./10-lod-and-streaming.md#voxels-vs-heightfield), [18](./18-collision-and-edits.md), Q2 in [14](./14-open-questions-and-roadmap.md) |
| **Who runs it** | Claude builds and measures; the user judges the look |

### E8 — Classification cost and look

| | |
|---|---|
| **Question** | Are the defaults in [19](./19-biome-classification.md#tuning-constants) cheap enough and good-looking enough? |
| **Candidates** | sharpness *k* = 2 / 4 / 6; mosaic strength 0.2 / 0.35 / 0.6; jitter amplitude 0.5× / 1× / 2× |
| **Setup** | the stub content pack (8 aerial biomes, 1 ecotone, 1 variant); top-down biome-map renders from the macro grid, side by side |
| **Metrics** | L2 cost (from E1); border width; patch size distribution; visual rating |
| **Pass** | L2 within budget; the user picks a default set |
| **Stop rule** | one round of comparisons; further tuning is Phase 2 |
| **Records into** | [19](./19-biome-classification.md#tuning-constants) |
| **Who runs it** | Claude renders; the user picks |

### E9 — Build iteration

| | |
|---|---|
| **Question** | How long does a one-line change take to see? |
| **Candidates** | Bevy `dynamic_linking` on/off; default linker versus `rust-lld`; dev-profile `opt-level` settings |
| **Setup** | touch one function in `genesis_gen` and one in the Bevy app; measure incremental build plus launch |
| **Metrics** | wall-clock seconds, cold and warm |
| **Pass** | ≤ 15 s for a `genesis_gen` change |
| **Stop rule** | take the best measured setup |
| **Records into** | [20](./20-tech-stack-and-workspace.md#toolchain-and-build-settings) |
| **Who runs it** | Claude |

---

## Order and dependencies

```text
Phase 0 workspace ──► E1 field cost ──► E2 determinism ──┬─► E3 LOD ──► E4 streaming
                                                         ├─► E5 precision
                                                         ├─► E6 collision ──► E7 voxels
                                                         └─► E8 classification
E9 build iteration: measured continuously from the first Bevy build
```

E1 and E2 can run as soon as `genesis_core` and the synthetic field exist. E3–E7 share one Bevy
app, built once and extended.

---

## Recording results

Each experiment gets a results file in `docs/spikes/E<n>-<name>.md`:

```text
# E1 — Field cost
Date, commit, hardware, build profile, thread cap
Candidates tried
Raw measurements (table, plus links to criterion reports or recordings)
Decision and why
Docs updated (with links)
```

Results files are permanent. They are the evidence for each `DECIDED` that the spike produces.

---

## Write-back

| Number | Owning doc | Experiment |
|---|---|---|
| Per-layer sample cost and budgets | [09](./09-surface-field-stack.md#performance-budget) | E1 |
| *K*, palette cap, sharpness, mosaic, jitter defaults | [19](./19-biome-classification.md#tuning-constants) | E1, E8 |
| Golden-seed procedure and platforms | [11](./11-determinism-and-sampling.md#golden-seeds) | E2 |
| LOD band spacings, patch grid, seam method | [10](./10-lod-and-streaming.md), [16](./16-data-contracts.md#deriving-lod-bands-from-feature-scale) | E3 |
| Low-altitude speed cap, worker count | [10](./10-lod-and-streaming.md), [01](./01-world-model.md#travel-time-as-a-dial) | E4 |
| Floating-origin approach, maximum system size | [01](./01-world-model.md#precision-rules), [20](./20-tech-stack-and-workspace.md#how-bevy-is-used) | E5 |
| `interactionRadius`, `collisionRadius`, physics crate | [18](./18-collision-and-edits.md#collision-lod), [20](./20-tech-stack-and-workspace.md) | E6 |
| Voxel size, region radius, mesher, edit storage | [10](./10-lod-and-streaming.md#voxels-vs-heightfield), [18](./18-collision-and-edits.md), Q2 | E7 |
| Build settings | [20](./20-tech-stack-and-workspace.md#toolchain-and-build-settings) | E9 |

---

## Related

- [14 — Open Questions & Roadmap](./14-open-questions-and-roadmap.md) — Phase 1 and Q2.
- [20 — Tech Stack & Workspace](./20-tech-stack-and-workspace.md) — the stack being tested.
- [10 — LOD & Streaming](./10-lod-and-streaming.md), [18 — Collision & Edits](./18-collision-and-edits.md),
  [19 — Biome Classification](./19-biome-classification.md) — the docs whose numbers this settles.
