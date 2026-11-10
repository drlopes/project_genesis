# World Generation — Design Documentation

This folder is the reference for how the game generates galaxies, star systems, planets,
and their surfaces. It exists for two audiences:

- **The designer/developer** — a place to reason about the system without holding it all in
  your head.
- **AI agents and future contributors** — enough context to make changes that fit the
  architecture instead of fighting it.

The docs are **living**. They describe the intended design, not necessarily the current
state of the code. Where something is not yet decided it is marked `OPEN`; where a decision
has been made it is marked `DECIDED`; where a concrete design is proposed but not yet
accepted it is marked `PROPOSED`.

---

## The one-paragraph summary

The game is a seamless space exploration/survival game: the player takes off from a planet,
flies into space, lands on another body, and dives to the seafloor — or visits any point of
interest — **without a loading screen**. Surface and underwater are peer environments. Worlds are
generated procedurally and must stay varied across thousands of planets.
The architecture therefore separates **what a world is** (a small, deterministic descriptor
baked once per body) from **how it is rendered, collided with, and queried** (a pure function
evaluated per chunk at whatever level of detail is needed). Physical, ecological, geological,
and narrative properties are modelled as **independent axes and layers** that compose, rather
than as one big enumeration of planet types.

Start with [Vision & Pillars](./00-vision.md), then [World Model](./01-world-model.md), then
[The Planet Descriptor](./03-planet-descriptor.md).

---

## Document map

| Doc | Concern | Read it when… |
|---|---|---|
| [00 — Vision & Pillars](./00-vision.md) | Goals, non-goals, success criteria | You need to know *why* the architecture looks like this |
| [01 — World Model](./01-world-model.md) | Hierarchy, scale, coordinates, traversal | You are touching anything spatial or LOD-related |
| [02 — Galaxy & Systems](./02-galaxy-and-systems.md) | Galaxy, system profiles, stars, orbits, moons | You are generating the setting |
| [03 — The Planet Descriptor](./03-planet-descriptor.md) | Bake vs query, schema, versioning | You are adding anything a planet "knows" |
| [04 — Physical Parameters](./04-physical-parameters.md) | Orthogonal axes and derived labels | You are tempted to add a new "planet type" enum |
| [05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md) | Temperature, moisture, sea level, lakes, water | You are working on oceans, weather, or habitability |
| [06 — Geology & Landforms](./06-geology-and-landforms.md) | Tectonics, craters, erosion, landform suites | You are working on surface shape below the biome layer |
| [07 — Biomes & Ecology](./07-biomes-and-ecology.md) | Derived biomes, region layers, barren worlds | You are working on surface appearance, vegetation, or "samey" planets |
| [08 — Narrative Layer](./08-narrative-layer.md) | Affinities, anomalies, POIs, ruins, naming | You are adding lore, factions, or hand-placed content |
| [09 — Surface Field Stack](./09-surface-field-stack.md) | Sample contract, L0–L4 evaluation, band-limiting, invariants | You are writing terrain evaluation code |
| [10 — LOD & Streaming](./10-lod-and-streaming.md) | Cube-sphere, LOD, collision, seamless transitions | You are working on rendering, physics, or performance |
| [11 — Determinism & Sampling](./11-determinism-and-sampling.md) | RNG, seeds, stratified sampling, diversity audit | You are debugging reproducibility or "sameness" |
| [12 — Authoring & Tuning](./12-authoring-and-tuning.md) | Data files, schema, inspector, tooling | You are adding content or tuning distributions |
| [13 — Generator Modules](./13-generator-modules.md) | Plugin contract for body classes | You are implementing gas giants, ocean worlds, etc. |
| [14 — Open Questions & Roadmap](./14-open-questions-and-roadmap.md) | Undecided items, risks, build order | You are planning work |
| [15 — Aquatic Environments](./15-aquatic-environments.md) | Strata, benthic biomes, pelagic volume query, first-class underwater gameplay | You are working below the waterline |
| [16 — Data Contracts](./16-data-contracts.md) | IDs, references, units, versioning, merge semantics, LOD metadata | You are defining or changing any schema |
| [17 — Biome & Landform Format](./17-biome-and-landform-format.md) | Concrete biome, suite, palette, scatter, and zone schemas | You are authoring or validating content |
| [18 — Collision & Edits](./18-collision-and-edits.md) | Solid model, raycasts, waterline, edit records, voxel boundary | You are writing physics, interaction, or persistence code |

---

## Reading orders

- **Mental model (30 min):** 00 → 01 → 03 → 04.
- **Generation pipeline:** 02 → 04 → 05 → 06 → 07 → 09.
- **Runtime:** 01 → 03 → 09 → 10 → 11.
- **Physics & persistence:** 09 → 18 → 10 → 11 → 03.
- **Content/tuning:** 04 → 07 → 08 → 12.
- **Aquatic:** 05 → 15 → 07 → 10.
- **Data/schema:** 16 → 17 → 03 → 12.

---

## Glossary

Consistent terminology matters here more than usual, because the same word can mean three
different things in procedural generation. These are the definitions used throughout.

| Term | Meaning |
|---|---|
| **Body** | Anything in a system that orbits something: a star, a planet, a moon, an asteroid belt marker. |
| **Planet** | Shorthand for a landable or approachable body. Includes gas giants, which have no solid surface. |
| **Descriptor** | The baked, compact, deterministic record of everything a body *is*. See [03](./03-planet-descriptor.md). |
| **Bake** | The one-time generation of a descriptor from a seed. Runs when a body is first encountered. |
| **Query** | The pure runtime function `(descriptor, position, LOD) → sample`, used for rendering, collision, and gameplay. |
| **Collision sample** | The physical view of a surface point: solidity, physics material, medium, and water. Derived from the same field as the visual sample. See [18](./18-collision-and-edits.md#collision). |
| **Edit** | A saved, ordered, idempotent record of a player or authored change, applied after generation. Never replaces terrain. See [18](./18-collision-and-edits.md#edits). |
| **Field** | A scalar or vector defined at every point on a body (height, temperature, moisture, biome weight…). |
| **Layer (L0–L4)** | A stage in the [surface field stack](./09-surface-field-stack.md). Layers are frequency bands, not code passes. |
| **Region layer** | One of several independent maps that give a planet regional variation: ecological, geological, material, narrative. See [07](./07-biomes-and-ecology.md). |
| **Landform suite** | A parameterized family of geomorphology (dune seas, karst, hoodoos, glacial valleys…) applied on top of a biome. See [06](./06-geology-and-landforms.md). |
| **Stratum** | The environment class of a sample: `aerial`, `benthic`, `pelagic`, `subglacial`, `subterranean`. Selects the classifier and scatter rules. See [15](./15-aquatic-environments.md). |
| **Benthic** | The seafloor or lakebed surface. Uses the surface pipeline with a water-aware classifier. |
| **Pelagic** | The water column. Uses a volume query, not terrain. |
| **Volume query** | A runtime sample of a non-surface volume (water column, atmosphere), returning zone, light, current, scatter, and hazards. See [15](./15-aquatic-environments.md). |
| **Water body** | The liquid filling a hydrosphere: `marine`, `freshwater`, `brine`, `hydrocarbon`, `ammonia`, `subsurface`. |
| **Content ID** | Namespaced identifier for authored data, e.g. `core:biome/temperate_forest`. See [16](./16-data-contracts.md). |
| **Frozen / derived / cache** | The three classes of descriptor field: frozen is authoritative and persisted; derived is recomputed; cache is discardable. See [16](./16-data-contracts.md#frozen-derived-and-cached). |
| **Feature scale** | The smallest feature a biome, landform suite, or scatter preset produces, in metres. Drives LOD band-limiting and voxel-size validation. See [16](./16-data-contracts.md#lod-and-feature-metadata). |
| **LOD band** | One of five spacing-defined detail bands (0 = interplanetary impostor, 4 = ~0.5 m near the player). Body-independent; distinct from the quadtree **level**, which is per body and appears in chunk IDs. See [10](./10-lod-and-streaming.md#quadtree-level-versus-lod-band). |
| **LOD window** | The inclusive band range `[min_lod, max_lod]` in which an entry is evaluated. LOD 0 is farthest/coarsest and LOD 4 is nearest/finest, so `min_lod` is the coarsest and `max_lod` the finest. `max_lod` defaults to 4; `min_lod` derives from `feature_scale`. See [16](./16-data-contracts.md#deriving-lod-bands-from-feature-scale). |
| **Purpose tag** | The stable string in a seed key that names the decision it feeds, e.g. `"geology"` or `"chunk"`. Tags are append-only; adding one never shifts existing values. See [11](./11-determinism-and-sampling.md#the-seed-chain). |
| **System profile** | The narrative/weighting flavour of a whole system: Ancient, Untouched, Lifeless, Oasis, etc. Corresponds to "system type" in the original design. **Not** a hard gate on physics. |
| **Body class** | The dispatch key for a body's surface generator: `rocky`, `gas_giant`, `ice_giant`, `ocean`, `artificial`, `belt`. Corresponds to "terrain type" in the original design. |
| **Derived label** | A human-readable classification computed *from* the physical parameters (barren, volcanic, ocean world, ice world…). Used for UI, weighting, and dispatch — never as an input that gates other parameters. |
| **Affinity** | A narrative overlay on a physically generated world: mechanical, infected, corrupted, wild, gaia… See [08](./08-narrative-layer.md). |
| **POI** | Point of interest: a ruin, derelict, settlement, anomaly, crash site, cave system. Has a stable ID. |
| **Macro pyramid** | A low-resolution precomputed sample grid per body (height, climate, region IDs), used for cheap far-LOD queries. See [10](./10-lod-and-streaming.md). |
| **Chunk / patch** | A node of the cube-sphere quadtree at a given LOD. The unit of generation and streaming. |
| **Seed chain** | The hierarchical derivation `galaxy → region → system → body → feature`, always hashed with a purpose tag. See [11](./11-determinism-and-sampling.md). |

---

## Conventions

- **Units** are SI unless stated (metres, kelvin, pascals, seconds). Distances inside a system
  may be expressed in game units; the mapping is defined in [01](./01-world-model.md).
- **Numbers in these docs are placeholders** unless explicitly marked as a target or budget.
  Treat them as tuning starting points, not requirements.
- **Doc links are relative** (`./05-climate-and-hydrosphere.md`) so the folder can be moved or
  read on GitHub without changes.
- **When you add a concept, put it in exactly one doc** and link to it from the others. If two
  docs need the same paragraph, that paragraph belongs in the more fundamental doc.
