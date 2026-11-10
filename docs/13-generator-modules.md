# 13 — Generator Modules

Not every body should go through the same surface pipeline. Gas giants have no surface, ocean
worlds are mostly water, and artificial bodies may not obey geology at all.

This document defines the **contract** every body generator implements, the shared utilities
they use, and the built-in modules.

---

## The contract

A generator module is selected by the body's **class** (see
[04 — Physical Parameters](./04-physical-parameters.md)). It implements a small, fixed
interface:

```
bake(seed, params) -> Descriptor
sampleSurface(position, lod) -> SurfaceSample | null
scatter(chunk) -> [Instance]
```

Optional, class-dependent:

```
atmosphereSky(viewDir, position) -> SkySample
oceanSurface(position) -> WaterSample | null
volumeSample(position, lod) -> VolumeSample | null
sampleCollision(position, lod) -> CollisionSample | null
spaceImpostor() -> ImpostorParams
```

`SurfaceSample` and `VolumeSample` are defined in
[09 — Surface Field Stack](./09-surface-field-stack.md#the-sample-contract). `height(position,
lod)` and `material(position, lod)` are convenience views over `sampleSurface`, not separate
generation paths.

`CollisionSample`, `raycast`, and the edit overlay are defined in
[18 — Collision & Edits](./18-collision-and-edits.md#the-query-contract). Most generators do not
implement collision themselves: the shared collision utility derives it from `sampleSurface` plus
the body's cave volume. A generator only overrides `sampleCollision` when it has a non-heightfield
solid.

Rules:

1. **`bake` is deterministic and order-independent.** Same inputs, same descriptor.
2. **`sampleSurface` returns `null` for bodies with no solid surface** (gas giants, ice giants).
   Callers must handle `null`, not assume a surface.
3. **`sampleSurface` is band-limited to the requested LOD.** At LOD 0 it returns the macro
   silhouette; it never returns detail above the cutoff.
4. **`material`, water, and `scatter` must be consistent with the sample's height** (nothing
   floating, nothing inside the ground, water depth matching the waterline).
5. **No global mutable state.** All state comes from the descriptor and the seed chain.
6. **No dependency on other bodies** except by ID (e.g. a moon referring to its planet).
7. **Bodies with a water column implement `volumeSample`.** It returns the pelagic zone, light,
   current, turbidity, scatter density, and hazards. Bodies without a water column return `null`.

`DECIDED` — the contract above. Exact language/signatures depend on the engine.

---

## The bake contract

`bake` is split into two outputs with different lifetimes (see
[03 — The Planet Descriptor](./03-planet-descriptor.md#core-versus-caches)):

```text
bakeCore(seed, class, systemProfile, params, content) -> DescriptorCore
bakeCaches(core)                                      -> Caches
```

- The **core** is the descriptor: identity, physical blocks, content references, narrative,
  derived labels. It is deterministic and hashable.
- The **caches** are derived from the core and are never part of its hash: macro pyramid, region
  index, water-distance field, impostor data. A missing cache is rebuilt; it cannot change the
  world's identity.
- The split exists because the macro pyramid samples L0, and L0 needs the core's geology and
  climate. Building the core first breaks the cycle.

### Inputs

```text
bakeCore(
  seed          : SeedChain,        // galaxy / region / system / body
  class         : BodyClass,        // dispatch key
  systemProfile : ProfileWeights,   // weights and decoration, never physics gates
  params        : Params,           // class defaults + authored overrides
  content       : ContentSet,       // resolved packs + contentVersion
) -> DescriptorCore
```

- `seed` is identity; `params` are choices. Code owns mechanism, data owns choices (D20).
- `params` must not contain per-chunk detail. If it does, it is content that belongs in the field
  stack, not in bake.
- Bake may read another body **by ID only** (a moon's parent, a ringed planet's rings). It never
  reads another body's descriptor.
- Bake never reads the save. Authored overrides arrive explicitly through `params` (Q10).

### Phases

A class pipeline is a **subset** of the canonical phases in
[03](./03-planet-descriptor.md#bake-ordering-within-a-body), in the same order. Each phase writes
only its own block and may read the blocks already written:

| Phase | Writes | Reads |
|---|---|---|
| identity | `id`, `seedChain`, versions, `generator` | seed, class |
| bulk | `bulk`, `rotation`, `orbit` | seed, system |
| energy | `energy` | orbit, bulk, star |
| atmosphere | `atmosphere` | energy, bulk, seed |
| water inventory | `hydrosphere.waterInventory`, `state`, `waterBody` | temperature, pressure |
| geology | `geology` | bulk, energy, seed |
| coarse relief | hypsometric summary; L0 samples for the cache | geology |
| hydrosphere completion | `seaRadius`, `lakes` | coarse relief, water inventory |
| climate | `climate` parameters | energy, atmosphere, relief, rotation |
| regions | `regions` | geology, climate |
| biosphere | `biosphere` | climate, hydrosphere |
| content selection | `biomePalette`, `landformSuites`, `materials` | regions, climate, biosphere |
| aquatic | `aquatic` | hydrosphere, climate, geology, biosphere |
| narrative | `affinity`, `appliedOverlays`, `anomalies`, `pois`, `naming` | everything above |
| labels | `derivedLabels`, `tempBands`, `contentFlags` | everything above |
| caches | macro pyramid, region index, water-distance field | the finished core |

Skipping a phase is allowed only when the class does not use it. Writing a block from a phase that
did not run is a bug.

### Required and forbidden blocks

| Class | Required | Absent / `null` |
|---|---|---|
| `rocky` | identity, bulk, energy, geology, climate, regions, palette/suites, labels | hydrosphere and biosphere may be absent |
| `ocean` | `rocky` requirements + hydrosphere + aquatic | — |
| `gas_giant` | identity, bulk, energy, atmosphere, band/storm parameters, labels | geology, hydrosphere, biosphere, surface |
| `ice_giant` | `gas_giant` requirements, colder volatiles | geology, surface |
| `artificial` | identity, narrative, a geometry source | any physical block it does not use |
| `belt` | system marker, per-asteroid seeds | a single surface |

`sampleSurface` returns `null` exactly for the classes with no surface; `sampleCollision` follows.

### Per-class pipelines

**`rocky`** — the full pipeline:

```text
identity → bulk → energy → atmosphere → water inventory → geology
→ coarse relief → hydrosphere completion → climate → regions
→ biosphere → content selection → aquatic (if water) → narrative → labels
→ caches
```

Optional branches are not special cases: no atmosphere sets `atmosphere.present = false`; no water
sets `hydrosphere.state = "none"` and skips lakes; no biosphere sets `biosphere.level = "none"` and
the biome layer falls back to barren/geological suites.

**`ocean`** — `rocky` with the water branch mandatory:

```text
… → water inventory (oceanFraction near 1) → geology (seafloor)
→ coarse relief (seafloor) → hydrosphere completion (global seaRadius)
→ climate → regions (benthic + island) → biosphere
→ content selection → aquatic (mandatory) → narrative → labels → caches
```

The seafloor uses the same L0 stack; the surface the player touches is the water shell. Land is a
local high point of the seafloor, not a separate generator.

**`gas_giant`** — atmosphere only:

```text
identity → bulk → energy → atmosphere → band/storm parameters
→ narrative → labels → caches (pressure-level field, impostor data)
```

Produce no `geology`, no `hydrosphere`, no `biosphere`, no `sampleSurface`. The cloud deck is a
pressure level, not a solid surface. Moons and rings are separate bodies.

**`ice_giant`** — `gas_giant` with a colder palette and volatile composition. If a subsurface ocean
exists, it is descriptor data consumed by the aquatic layer, not a surface query.

**`artificial`** — construction first:

```text
identity → narrative (dominant) → geometry source (authored or grammar)
→ optional physical blocks → labels
```

Do not force it through geology or climate. A station or ring world may have gravity, atmosphere,
or a biosphere without obeying the physical pipeline.

**`belt`** — a system-scale marker:

```text
identity → distribution parameters → per-asteroid seeds → labels
```

It has no single surface. Individual asteroids are baked on demand as small `rocky` bodies from
their own seeds.

### Degenerate cases

These are valid worlds, not errors:

| Case | Behaviour |
|---|---|
| No atmosphere | `present = false`, pressure `0`, vacuum sky, no greenhouse or heat transport |
| No water | `state = "none"`, `seaRadius = null`, no aquatic block, `water == null` at every sample |
| No biosphere | `level = "none"`, barren/geological biome fallback, no scatter that requires life |
| Frozen hydrosphere | Water surface exists but is ice; same geometry, ice material |
| Ocean with no land | `oceanFraction ≈ 1`; every surface sample is benthic; islands are local highs |
| Tidally locked | Rotation block carries `tidalLock`; climate uses the substellar model |
| Very thin crust / high volcanism | Geology produces a young, resurfaced world; no special class |

Content must be able to author a valid world for every row. If a case requires a new block, the
block is missing from this contract.

### Bake tests

Every generator must pass, in addition to the runtime tests below:

1. **Determinism** — same inputs → byte-identical core and identical hash.
2. **Order independence** — baking the same body twice, in any order relative to other bodies,
   produces the same core.
3. **Required blocks** — the class table above is satisfied; absent blocks are explicitly `null`,
   never missing.
4. **No dangling references** — every content ID in the core resolves in the declared
   `contentVersion`.
5. **Cache reproducibility** — deleting the caches and rebuilding them from the core yields
   equivalent samples within tolerance.
6. **Size budget** — the core stays in the kilobyte range; caches stay in the hundreds of
   kilobytes per body (see [10](./10-lod-and-streaming.md#the-macro-pyramid)).

---

## Shared utilities

Generators should not reimplement common machinery. Provide libraries for:

- **Noise** — band-limited, seeded, domain-warpable.
- **Spherical math** — directions, caps, Voronoi on the sphere, great-circle distance.
- **Seed/RNG** — counter-based, keyed by identity.
- **Macro pyramid access** — sampling the baked L0 field.
- **Climate** — temperature/moisture evaluation from descriptor parameters.
- **Classification** — biome/landform/material weights.
- **Landform suites** — the data-driven detail modules.
- **Scatter** — Poisson-disk and density-based placement.
- **Water** — sea level + lake query (see [05](./05-climate-and-hydrosphere.md)).

This keeps generators thin and consistent.

---

## Built-in generators

### `rocky`

The workhorse. Heightfield-based surface with the full L0–L4 stack:

- plates, impacts, erosion → macro relief,
- climate → biome weights,
- landform suites → detail,
- micro detail near the player,
- optional hydrosphere (sea level + lakes),
- optional biosphere and affinity.

Most planets and moons use this, with different parameters.

### `ocean`

A global ocean over a seafloor:

- seafloor uses the `rocky` macro geology (plates, trenches, ridges),
- `sampleSurface` returns seafloor elevation; the ocean surface is a separate shell at `seaRadius`,
- no (or almost no) land; islands are local high points of the seafloor,
- biosphere applies to the water column and islands.

It also implements `volumeSample` for the water column, and it uses the aquatic descriptor block
(photic depth, turbidity, currents, benthic palette, pelagic zones). The seafloor is a benthic
surface classified by depth and light; the water column is a pelagic volume. See
[15 — Aquatic Environments](./15-aquatic-environments.md).

This is a distinct class because the surface the player interacts with is the water, not the
ground; rendering, collision, and gameplay differ enough to justify it. A `rocky` body with an
ocean on it still uses the `rocky` generator, with its own aquatic additions.

### `gas_giant`

No surface at all:

- `sampleSurface` returns `null`,
- the body is a **volumetric banded atmosphere**: latitude bands, storms, jets,
- the "surface" is a cloud deck at a defined pressure level; entering it is a gameplay boundary,
- moons and rings orbit it,
- no biosphere, no terrain, no voxels.

The visual model is procedural texture/volumetrics, not geometry. See
[10 — LOD & Streaming](./10-lod-and-streaming.md) for impostors.

### `ice_giant`

Like `gas_giant`, with a colder, volatile-rich palette and different band structure. May support
a "cloud deck" entry with extreme conditions.

### `artificial`

Constructed bodies: ring worlds, stations-as-worlds, engineered planets.

- geometry may be authored or generated from a construction grammar,
- may or may not have an atmosphere, gravity, biosphere,
- narrative layer is usually dominant.

Keep this as a per-instance generator; do not try to generalise it into the physical pipeline.

### `belt`

Not a single body: an asteroid belt. Rendered as instanced objects plus a system-scale marker.
Individual asteroids may be landable but are generated on demand from their own seeds.

---

## Dispatch

```
body.class ──► generator module
```

The `generator` field in the descriptor (see [03](./03-planet-descriptor.md)) records which
module produced it, so a save can be loaded even if the dispatch logic changes.

`DECIDED` — the descriptor stores the generator key and its version.

---

## Module registry and parameter schemas

Body generators and landform suites are both **modules**: code keyed by a stable string, with a
declarative parameter schema. The registry maps a key to its implementation, its `kind`
(`generator` or `suite`), its module `version`, and its parameter schema.

The schema **format** — parameter types, `default`, `min`/`max`, `unit`, `values` — is defined
once, in [17 — Biome & Landform Format](./17-biome-and-landform-format.md#module-parameter-schema),
with `dune_field` as the worked example. The schema is declared in code and surfaced to tools; this
section covers only the registry and versioning rules.

Rules:

- `kind` is `generator` or `suite`. Both use the same schema format; only the invocation site
  differs (class dispatch vs. biome `landform_bias`).
- Every parameter declares a `type` and a `default`, plus constraints (`min`/`max`, `values`) where
  one applies. Units are explicit for physical values.
- **Content** `params` may omit keys; the schema default applies. **Unknown** keys are errors;
  silent toleration hides content typos.
- **Descriptor** `params` are fully resolved: every schema key is present, after defaults, overlays,
  and per-body draws. A missing key in a descriptor is a bake bug.
- The module's schema is versioned **with the module**. Changing a default or behaviour is a
  `generatorVersion` change; adding an optional parameter whose default reproduces the old result
  is a schema-only change.
- The descriptor stores the **resolved values**, not a schema reference, so a save can be read after
  the schema evolves. Migration reads the stored values against the old schema.
- Module keys are append-only. Renaming a key ships an alias in the content manifest (see
  [16 — Data Contracts](./16-data-contracts.md#file-layout-and-packs)); a retired key is never
  reused for different behaviour.

`DECIDED` — modules declare typed parameter schemas; descriptor params validate against the
declared version.

---

## Variants vs new classes

Do not create a new class for every flavour. Prefer variants:

| Want | Do |
|---|---|
| Tidally locked world | `rocky` with `rotation.tidalLock` set; climate model switches |
| Desert world | `rocky` with low moisture; biome layer handles it |
| Ice world | `rocky` with frozen hydrosphere |
| Volcanic world | `rocky` with high volcanism |
| Water world | `ocean` class (or `rocky` with very high ocean fraction — decide once) |
| Subsurface ocean moon | `ocean` with a thick ice shell |

A new class is justified only when the **interaction model** changes: no surface (gas giants) or
a fundamentally different surface (global ocean). This keeps the codebase small.

`DECIDED` — new classes require a different interaction model, not just different parameters.

---

## Testing a generator

Every generator should pass:

1. **Determinism** — same seed → same descriptor hash.
2. **LOD continuity** — the surface does not change shape across LOD levels.
3. **Null safety** — callers handle `sampleSurface() == null`.
4. **Surface agreement** — material, water, and scatter agree with the sample; no floating or
   buried scatter.
5. **Water consistency** — `sampleSurface` and `oceanSurface` never contradict.
6. **Aquatic consistency** — `volumeSample`, `sampleSurface`, and the water surface agree about the
   waterline; benthic scatter is anchored to the seafloor.
7. **Performance budget** — bake time and query time within targets.
8. **Visible-from-orbit consistency** — the space appearance matches the surface.

Add a per-generator scene/test that exercises these.

---

## Extension points

- **Mods** can register new generators, biomes, landform suites, affinities, and POI types via
  the data layer (see [12 — Authoring & Tuning](./12-authoring-and-tuning.md)).
- **New modules** register a key and a typed parameter schema (above); the core only owns dispatch
  and the shared utilities.
- **DLC** can add generators without touching the core, provided they honour the contract.
- **Future body types** (ring worlds, megastructures, exotic anomalies) are best implemented as
  authored generators rather than shoehorned into `rocky`.

---

## Related

- [03 — The Planet Descriptor](./03-planet-descriptor.md) — the `generator` field and schema.
- [04 — Physical Parameters](./04-physical-parameters.md) — classes vs labels.
- [05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md) — ocean and water.
- [09 — Surface Field Stack](./09-surface-field-stack.md) — the `rocky` pipeline.
- [10 — LOD & Streaming](./10-lod-and-streaming.md) — impostors and LOD.
- [16 — Data Contracts](./16-data-contracts.md) — module schemas, units, and pack layout.
- [17 — Biome & Landform Format](./17-biome-and-landform-format.md) — suite and biome content.
- [12 — Authoring & Tuning](./12-authoring-and-tuning.md) — data-driven generators.
- [15 — Aquatic Environments](./15-aquatic-environments.md) — ocean, benthic and pelagic contract.
- [18 — Collision & Edits](./18-collision-and-edits.md) — collision query and the edit overlay.
