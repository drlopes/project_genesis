# 10 — LOD & Streaming

The seamless-flight requirement means the world must exist continuously from orbit to ground,
at bounded cost. This document covers the spatial data structure, level of detail, streaming,
collision, coordinate precision, and the atmosphere boundary.

Read [01 — World Model](./01-world-model.md) for the coordinate spaces and
[09 — Surface Field Stack](./09-surface-field-stack.md) for what each LOD evaluates.

---

## The central abstraction: cube-sphere quadtree

Each body's surface is a **cube-sphere**: six faces of a quad-sphere, subdivided into a quadtree
of **patches**.

```
body
└── 6 faces
    └── quadtree of patches (per face)
        ├── level 0  (1 patch per face)
        ├── level 1  (4 patches)
        └── ...      (4^n patches at level n)
```

The quadtree **level** is a structural index (it appears in chunk IDs and seeds). It is not the
same thing as the **LOD band** used by content and the field stack; see
[Quadtree level versus LOD band](#quadtree-level-versus-lod-band) below.

Why cube-sphere:

- uniform-ish sample density over the whole sphere,
- trivial quadtree indexing and neighbour lookup,
- no pole singularity in the grid,
- maps cleanly to the "four faces around a point" seam handling.

Each patch has:

- face, level, x, y → a stable ID,
- a world-space bounding volume,
- a target LOD derived from camera distance and screen-space error,
- a resolution (grid size) that is constant per patch (e.g. 33×33 or 65×65 vertices).

`DECIDED` — cube-sphere quadtree with fixed per-patch grid resolution.

---

## LOD levels and budgets

An **LOD band** is defined by its target **sample spacing**, in metres, independent of body size:

| LOD band | Camera regime | Max sample spacing (`spacing(band)`) | Resolvable feature (`≥ 2 × spacing`) | Layers evaluated | Collision |
|---|---|---|---|---|---|
| **0** | interplanetary | impostor, ≥ ~4 km | ≥ ~8 km | L0 (macro pyramid only) | none |
| **1** | high orbit | ~1 km | ≥ ~2 km | L0 + coarse L1 | none |
| **2** | low orbit / upper atmosphere | ~50 m | ≥ ~100 m | L0–L2 + coarse L3 | coarse |
| **3** | surface, mid-distance | ~4 m | ≥ ~8 m | L0–L3 | medium |
| **4** | near player | ~0.5 m | ≥ ~1 m | L0–L4 | fine |

These are starting points; the Phase 1 spike sets the real numbers (Q2). The important invariant:
**the same function produces every LOD**, and adding detail never changes the macro shape.

### Quadtree level versus LOD band

Patches have a fixed grid (e.g. 65×65 vertices, 64 intervals). A patch at quadtree level `n` on a
body of radius `R` therefore has a sample spacing of roughly:

```text
faceArc        = (π / 2) · R
spacing(n, R)  = faceArc / (2^n · intervalsPerPatch)
band(n, R)     = the finest band whose max spacing is >= spacing(n, R)
```

Within a band the actual spacing is never coarser than `spacing(band)`, so a feature that passes
the band's Nyquist test passes it at every level inside the band.

The band is what the field stack and content see; the level is what streaming, chunk IDs, and seeds
see. Several adjacent levels map to the same band. For a 24 km-radius body (`faceArc ≈ 37.7 km`)
with 64 intervals per patch:

| Quadtree level | Spacing | LOD band |
|---|---|---|
| 0 | ~590 m | 1 |
| 3 | ~74 m | 1 |
| 4 | ~37 m | 2 |
| 7 | ~4.6 m | 2 |
| 8 | ~2.3 m | 3 |
| 10 | ~0.58 m | 3 |
| 11 | ~0.29 m | 4 |

At LOD band 0 the body is an impostor and no patches are streamed. A larger body needs a deeper
quadtree to reach the same band; a smaller one needs a shallower one. Content never has to know.

`DECIDED` — LOD bands are spacing-defined and body-independent; quadtree depth is per body.

### LOD numbering and content windows

The number increases with closeness: **LOD 0 is the coarsest/farthest, LOD 4 is the finest/nearest**.
Content uses the same direction:

- `min_lod` is the **coarsest** LOD at which an entry is evaluated (smallest number).
- `max_lod` is the **finest** LOD at which it is evaluated (largest number).
- The entry is active when `min_lod <= lod <= max_lod`.

A tree is therefore `min_lod = 3`, not `max_lod = 3`: it is a near-field feature. A rule that omits
`min_lod` defaults to the coarsest LOD its `feature_scale` can resolve; a rule that omits `max_lod`
defaults to `4`. See
[16 — Data Contracts](./16-data-contracts.md#deriving-lod-bands-from-feature-scale) for the
Nyquist rule and the band spacing table.

Scatter has one extra wrinkle: below `min_lod`, a rule may still contribute **billboards/impostors**
inside its `cull_distance`. `min_lod` is where full instances begin, not necessarily where the
species becomes visible. This keeps a forest readable from a few kilometres without meshing every
tree.

### LOD selection

A patch splits when **one sample spacing projects to more than *SSE* pixels**:

```text
error_px = spacing(level) / distance · viewport_height_px / (2 · tan(fov / 2))
split if error_px > SSE            (and level < max level)
stay split while error_px > SSE / 2   (hysteresis)
```

- `distance` is measured to the patch centre **on the terrain**, minus a bound of
  `0.75 · edge + min(0.5 · edge, total relief)`. A fixed relief margin makes everything near the
  camera look adjacent and subdivides the whole neighbourhood (E3).
- Patches entirely below the horizon of the body's lowest-terrain sphere are not selected.
- A "must be at least LOD N" flag near the player still applies for collision (E6).

`DECIDED` (from [E3](./spikes/E3-lod-and-quadtree.md)) — the metric above, with a **65 × 65** patch
grid. The useful SSE range is **8–16 px**: at 2 px the near-ground cut is ~5 000 patches and ~45 M
triangles, and each doubling of SSE cuts both by ~2.5×. SSE 16 meets every Phase 1 budget with the
current vertex format; SSE 8 needs a compressed vertex format (I-005) to fit the terrain VRAM
budget. The default between them is set by the E3 visual check.

### Seams

Adjacent patches at different LODs will crack. Options, pick one:

- **Skirts** — extend each patch edge downward by a small amount to hide the gap. Simplest,
  works everywhere, small overdraw.
- **Stitching** — generate transition triangles/indices between LODs. Cleaner geometry, more
  complex.
- **Morphing** — vertex positions are blended toward the coarser LOD near boundaries. Best
  visual result, more shader work.

`PROPOSED` — start with skirts; upgrade to morphing if the seams are visible during flight. E3
used skirts (depth: four sample spacings) with no hole frames at SSE 16; the visual verdict on
cracks and popping is the E3 user check.

---

## The macro pyramid

The macro pyramid is a precomputed, low-resolution sampling of L0 (and optionally coarse L1/L2)
for a body. It is the **cached representation** used for far-LOD appearance and cheap queries; the
definition remains the descriptor plus the generator.

- Stored as a cube-sphere grid (e.g. 6 × 256 × 256 samples) or a mip-like hierarchy.
- Contains: macro height, sea radius, biome region ID, material ID, optionally climate summary.
  The contents are class-dependent: a gas giant stores a pressure-level field and impostor data
  instead of a sea radius.
- Size: a few hundred kilobytes per body.
- Generated from the core descriptor after bake; it must be consistent with the runtime query.

Used for:

- silhouette and impostor rendering at LOD 0–1,
- sea level and lake computation,
- POI placement,
- collision at distance (if needed),
- a fallback when detail generation is behind.

`DECIDED` — macro pyramid per body; it is a cache, not the definition. The definition is the
descriptor + generator.

---

## Coordinate precision

At interplanetary distances, single-precision floats lose centimetre accuracy. The renderer must
never operate on large coordinates.

`DECIDED` — floating origin (camera-relative rendering):

- Keep world coordinates in 64-bit (or double) for gameplay/physics at system scale.
- Each frame, translate all rendered geometry relative to the camera (or camera relative to a
  moving origin) so the renderer sees small numbers.
- Surface-local coordinates are single precision and small; convert at the boundary.
- Physics uses a local frame around the player (or around each active body).

Bevy transforms are single precision. A camera-relative frame (the `big_space` crate or an
equivalent of our own) is evaluated in the Phase 1 spike and verified at interplanetary ranges
before we rely on it. See [20 — Tech Stack & Workspace](./20-tech-stack-and-workspace.md#how-bevy-is-used).

---

## Collision

Collision is **not** a separate geometry generation path. It is the same query at a coarser LOD,
so the visual and physical surfaces cannot disagree:

- Near the player: full-detail collision built from the same samples as the chunk.
- At distance: coarse collision from a lower LOD patch, used only if the player can reach it.
- Bodies with no reachable surface (gas giants, distant bodies): no collision.

The collision LOD rule, query shapes (`sampleCollision`, `raycast`), sphere tracing, physics
materials, and the water-is-not-solid rule live in
[18 — Collision & Edits](./18-collision-and-edits.md).

`DECIDED` — single source of truth for rendering and collision.

### Voxels vs heightfield

Voxels are used for the leaf detail that the player can modify (dig, build). The macro and
mid-LOD surfaces can be heightfield/mesh based. The transition must be seamless: the voxel
region must sample the same L0–L3 field so its boundary matches the mesh.

`DECIDED` — strategy (D27):

- **Voxels near the player**, within an editing/interaction radius.
- **Heightfield/mesh at distance**, down to the macro pyramid.
- **One source of truth**: the voxel boundary is generated from the same field as the mesh, so
  there is no visual seam.
- **Boundary rule**: distance from the player plus editability — a base or dig site keeps its
  voxels resident even when the player leaves.

The overlap and seam-tolerance contract for the boundary is in
[18 — Collision & Edits](./18-collision-and-edits.md#the-voxelmesh-boundary).

`OPEN` — the numbers (Q2): voxel size, how far voxelisation extends, edit persistence
granularity, and how edits are garbage-collected. Resolve these with the Phase 1 spike.

### The voxel spike

The voxel numbers are settled by experiment **E7** of the
[Phase 1 spike plan](./21-phase-1-spike-plan.md#e7--voxels-and-edits-q2): voxel size, region
radius, meshing cost, the seam at the mesh boundary, the maximum edit radius within the frame
budget, and edit storage and garbage collection. The result also sets the voxel size used by the
`feature_scale` validation in [16](./16-data-contracts.md#lod-and-feature-metadata).

---

## Player edits

Player modifications (digging, building, destruction) are stored as **edits**, not as derived
terrain:

```text
finalSample(p) = applyEdits(generate(p), editsNear(p))
```

Edits are keyed by body and chunk, applied after generation, and sorted into a deterministic order.
That keeps the procedural function authoritative and makes saves small. The record shape, domains,
ordering, promotion, and persistence rules are in
[18 — Collision & Edits](./18-collision-and-edits.md#edits).

`DECIDED` — edits overlay, not terrain replacement.

---

## Streaming

### Asynchronous generation

- Generate chunks on worker threads. Size the generation pool to **physical cores**: in
  [E1](./spikes/E1-field-cost.md), 16 SMT threads were slower than 8 on an 8-core CPU.
- Prioritise by: distance, screen coverage, player trajectory, collision needs. Re-prioritise the
  queue every frame; the camera moves faster than the queue drains.
- Use a lower-LOD placeholder until a chunk is ready, so there is never a hole.

Rules learned the hard way in [E3](./spikes/E3-lod-and-quadtree.md#spike-bugs-found-on-the-way):

- **Track in-flight work.** A patch that is being built, or is built and waiting to be integrated,
  must never be queued again. Without this, the backlog rebuilt the same patches until it held
  13 GB.
- **Discard stale results on receipt.** If the camera no longer wants a finished patch, drop it
  instead of integrating it and despawning it a frame later.
- **Display without holes or overlaps:** a wanted patch that is not ready is covered by its nearest
  ready ancestor; when zooming out, by its ready descendants; never both. Implement with tree walks
  and hash lookups (O(patches × depth)); scanning all resident patches is O(n²) and cost 15–60 ms
  per frame at a few thousand patches.
- **The first ceiling is the main thread's integration rate**, not worker throughput: every patch
  carries a fixed main-thread cost. Fewer, larger patches beat many small ones (see I-006).

### Caching and eviction

- Keep a pool of chunk meshes by LOD.
- Evict by distance and time, but keep the macro pyramid resident for visited bodies.
- Descriptor cache: body descriptors for recently visited systems; evict by region.

### Frame budget targets

Chunk generation and mesh building run on **worker threads**, so they have a throughput target,
not a per-frame budget: the workers must keep up with the LOD-4 ring at rover speed (E4). The
per-frame budget is for the **main thread**:

- integrating finished patches (GPU upload, spawning and despawning patch entities) plus the LOD
  update: ≤ 2 ms per frame in total,
- no frame above 33 ms while streaming,
- draw calls: bounded by patch count; use instancing for scatter.

The full budget table, and the measurements against it, are in the
[Phase 1 spike plan](./21-phase-1-spike-plan.md#reference-hardware-and-budgets).

---

## Atmosphere and the space boundary

The transition from space to atmosphere is where "no loading screen" is most visible.

- **Atmosphere shell** — a volumetric or shell-based atmosphere rendered from both outside and
  inside. It must render at interplanetary distance (as a limb/glow) and from the surface (as
  sky).
- **Entry effects** — heat, plasma, sonic boom — are visual/audio, not a scene change.
- **Sky colour** — derived from the atmosphere descriptor, continuous from orbit to ground.
- **Clouds** — a shell or volumetric layer, LOD'd, fading in from space.
- **Fog/haze** — near-surface, continuous with the atmospheric haze seen from orbit.

`DECIDED` — atmosphere and sky are rendered as part of the same continuous world, from the same
descriptor.

### Ocean LOD

- Far: part of the macro pyramid / low-res shell.
- Mid: tessellated shell with coarse waves.
- Near: high-resolution displaced surface with shore foam and depth shading.
- Underwater: fog, caustics, attenuation.

Ocean geometry is never voxelised (see [05](./05-climate-and-hydrosphere.md)).

### Underwater LOD

Underwater is a performance gift if used well, because **fog limits visibility**:

- The seafloor can drop to a much coarser LOD underwater than on land at the same physical
  distance, because the player cannot see it.
- The water column adds fog and particles, not geometry.
- Most of an ocean world is deep, dark, and low-detail; shallow reefs are the expensive part.
- Do not render above-water detail while submerged, or vice versa.

Underwater is a first-class environment, so this is a different budget from the surface, not a
smaller one. The savings from fog are reinvested in reefs, fauna, and visibility mechanics.

Anchored scatter (kelp, coral, tube worms) culls with the seafloor chunk. Free-floating scatter
(fish, jellies, plankton) uses density fields and billboards at distance. See
[15 — Aquatic Environments](./15-aquatic-environments.md).

---

## Scatter and props

Vegetation, rocks, and props are **instanced**, generated per chunk at L3/L4, and evicted with
the chunk.

- Density scales with LOD and with the biome/landform weights.
- Use deterministic scatter from chunk ID + position hash (see
  [11](./11-determinism-and-sampling.md)).
- Distant props: billboards/impostors or none.
- No simulation of individual props beyond the near field.

---

## Performance risks

| Risk | Mitigation |
|---|---|
| Chunk generation hitches | async + time budget + placeholder LOD |
| Seam cracks visible in flight | skirts or morphing; test at every LOD boundary |
| Overdraw from atmosphere/ocean shells | depth prepass, culling, LOD |
| Memory growth from cached chunks | strict pool + eviction budget |
| Floating-point jitter far from origin | floating origin, local frames |
| Voxel/mesh boundary mismatch | voxels must sample the same field |
| Underwater overdraw from fog/particles | visibility-budgeted LOD; occlude with terrain |

---

## Vertical slice test

The first milestone that proves the architecture:

> Fly a ship from orbit to the surface of one planet and back, with no visible seam, no loading,
> and stable frame time.

If that works, the rest is content and tuning.

---

## Related

- [01 — World Model](./01-world-model.md) — coordinate spaces and hierarchy.
- [03 — The Planet Descriptor](./03-planet-descriptor.md) — the data driving generation.
- [05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md) — ocean and water surfaces.
- [09 — Surface Field Stack](./09-surface-field-stack.md) — what each LOD evaluates.
- [11 — Determinism & Sampling](./11-determinism-and-sampling.md) — deterministic chunk generation.
- [15 — Aquatic Environments](./15-aquatic-environments.md) — underwater visibility and LOD.
- [18 — Collision & Edits](./18-collision-and-edits.md) — collision, water, edits, voxel boundary.
