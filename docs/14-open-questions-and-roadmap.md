# 14 — Open Questions & Roadmap

This document tracks what is **decided**, what is **open**, what is **risky**, and the order in
which to build.

Keep it current. An out-of-date decision log is worse than none.

---

## Decisions made

| # | Decision | Where |
|---|---|---|
| D1 | Seamless surface↔space↔surface flight is a hard constraint | [00](./00-vision.md), [01](./01-world-model.md) |
| D2 | Aesthetic-first, causality-second, realism-third | [00](./00-vision.md) |
| D3 | Axes are inputs; labels are outputs (no flat planet-type enum) | [04](./04-physical-parameters.md) |
| D4 | Separate bake (descriptor) from query (pure function) | [03](./03-planet-descriptor.md) |
| D5 | Descriptors are the save format; terrain is never stored | [01](./01-world-model.md), [03](./03-planet-descriptor.md) |
| D6 | Deterministic, counter-based RNG keyed by identity + purpose | [11](./11-determinism-and-sampling.md) |
| D7 | Stratified/low-discrepancy sampling to prevent clumping | [11](./11-determinism-and-sampling.md) |
| D8 | No rivers, no fluid simulation; sea level + baked lakes only | [05](./05-climate-and-hydrosphere.md) |
| D9 | Water is rendered as a surface (shell + lake caps), not voxels | [05](./05-climate-and-hydrosphere.md) |
| D10 | Sea level derived from ocean fraction + hypsometry | [05](./05-climate-and-hydrosphere.md) |
| D11 | Biomes are derived per-sample, not painted; no blend pass | [07](./07-biomes-and-ecology.md), [09](./09-surface-field-stack.md) |
| D12 | Region layers are plural (ecological, geological, material, narrative) | [07](./07-biomes-and-ecology.md) |
| D13 | Landform suites are separate from biomes | [06](./06-geology-and-landforms.md) |
| D14 | Affinity is a post-generation modifier, not a physical axis | [08](./08-narrative-layer.md) |
| D15 | System profiles weight and decorate; they do not gate physics | [02](./02-galaxy-and-systems.md) |
| D16 | Bodies are on Keplerian rails; local physics in sphere of influence | [01](./01-world-model.md) |
| D17 | Cube-sphere quadtree with LOD; same function for render/collision | [10](./10-lod-and-streaming.md) |
| D18 | Player edits are an overlay applied after generation | [10](./10-lod-and-streaming.md) |
| D19 | Planets are small (tens of km); distances are design dials | [01](./01-world-model.md) |
| D20 | Data-driven tuning; code owns mechanism, data owns choices | [12](./12-authoring-and-tuning.md) |
| D21 | Strata (`aerial`, `benthic`, `pelagic`, `subglacial`, `subterranean`) select classifier and scatter | [07](./07-biomes-and-ecology.md), [15](./15-aquatic-environments.md) |
| D22 | Two query types: surface (height/material) and volume (water column) | [09](./09-surface-field-stack.md), [15](./15-aquatic-environments.md) |
| D23 | Underwater biomes are derived benthic biomes; biogenic geometry is a landform suite | [15](./15-aquatic-environments.md) |
| D24 | `waterBody` (marine/freshwater/brine/hydrocarbon/ammonia/subsurface) is part of the hydrosphere | [05](./05-climate-and-hydrosphere.md), [15](./15-aquatic-environments.md) |
| D25 | Underwater is a first-class environment, equal in weight to the surface | [00](./00-vision.md), [15](./15-aquatic-environments.md) |
| D26 | Freeze coarse decisions, migrate detail, never regenerate under a player | [03](./03-planet-descriptor.md#versioning-and-regeneration), [16](./16-data-contracts.md#frozen-derived-and-cached) |
| D27 | Voxel strategy: voxels near, mesh at distance, one source of truth; resolution is empirical | [10](./10-lod-and-streaming.md) |
| D28 | Content is declarative data: single-parent `extends`, typed suite parameters, overlays as a separate type resolved at bake | [17](./17-biome-and-landform-format.md), [16](./16-data-contracts.md) |
| D29 | Every geometry-producing entry declares `feature_scale`; LOD bands derive from it | [16](./16-data-contracts.md#lod-and-feature-metadata), [10](./10-lod-and-streaming.md) |
| D30 | Collision samples the same final field; water is analytic and never a solid; waves never move the gameplay waterline | [18](./18-collision-and-edits.md#collision), [09](./09-surface-field-stack.md) |
| D31 | Edits are total-ordered, idempotent records that pin their base generator version; classification is never re-run | [18](./18-collision-and-edits.md#edits), [03](./03-planet-descriptor.md#versioning-and-regeneration) |
| D32 | Bake outputs a hashable core and discardable caches; the macro pyramid is a cache, never identity | [03](./03-planet-descriptor.md#core-versus-caches), [13](./13-generator-modules.md#the-bake-contract) |
| D33 | The purpose-tag registry is append-only; feature lists are append-only within a generator version | [11](./11-determinism-and-sampling.md#the-seed-chain) |
| D34 | LOD increases with closeness (0 far, 4 near); `min_lod` is coarsest, `max_lod` finest | [10](./10-lod-and-streaming.md#lod-numbering-and-content-windows), [16](./16-data-contracts.md#deriving-lod-bands-from-feature-scale) |
| D35 | Variation is drawn at the coarsest owning scale; per-body draws are frozen, per-sample draws are pure | [12](./12-authoring-and-tuning.md#variation-budgets-and-per-body-draws), [11](./11-determinism-and-sampling.md#the-diversity-audit) |
| D36 | Modules declare typed parameter schemas; descriptor params validate against the declared version | [13](./13-generator-modules.md#module-registry-and-parameter-schemas), [16](./16-data-contracts.md#file-layout-and-packs) |
| D37 | LOD bands are spacing-defined and body-independent; quadtree depth is per body; bands gate content, actual spacing drives the fade | [10](./10-lod-and-streaming.md#quadtree-level-versus-lod-band), [09](./09-surface-field-stack.md#band-limiting-and-lod), [16](./16-data-contracts.md#deriving-lod-bands-from-feature-scale) |

---

## Open questions

### Q1 — Migration mechanics
The policy is settled (D26: freeze coarse, migrate detail, never regenerate under a player).
What remains open is the procedure: when migration runs, how affected worlds are detected, how
promotion-on-reference is recorded, and how the player is told.

See [03](./03-planet-descriptor.md#versioning-and-regeneration) and
[16](./16-data-contracts.md#frozen-derived-and-cached).

### Q2 — Voxel resolution and the mesh boundary
The strategy is settled (D27): voxels near the player, heightfield/mesh at distance, one source
of truth, and a boundary defined by distance and editability. The numbers are not: what voxel
size, how far voxelisation extends, and how edits are stored and garbage-collected.

Resolve this with a **Phase 1 spike**, not a design meeting. `feature_scale` (see
[16](./16-data-contracts.md#lod-and-feature-metadata)) lets content declare its minimum
resolution, so the boundary can be chosen after the spike without rewriting content. The edit
record and voxel/mesh boundary contract are in
[18](./18-collision-and-edits.md#the-voxelmesh-boundary).

See [10](./10-lod-and-streaming.md).

### Q3 — Time compression
Does the game support time acceleration while flying? How does it interact with survival,
physics, and orbital motion? How is it presented?

See [01](./01-world-model.md).

### Q4 — Planet radius targets
What is the actual target range (e.g. 5–40 km)? This drives voxel budget, LOD constants, gravity,
and art direction.

See [01](./01-world-model.md).

### Q5 — Interplanetary travel times
What are the target times for surface→orbit, planet→moon, planet→planet, system→system? These
determine orbital distances and ship speed.

See [01](./01-world-model.md).

### Q6 — Binary/multiple star climate
How much complexity to model for circumbinary vs circumstellar planets? A simplified combined
flux model is probably enough, but the sky and seasons differ.

See [02](./02-galaxy-and-systems.md).

### Q7 — Multiplayer / shared seeds
Is the game multiplayer? If so, is bit-identical terrain across platforms required, or only
logical agreement? This affects floating-point determinism requirements.

See [11](./11-determinism-and-sampling.md).

### Q8 — Galaxy topology
Spiral arms? Ring? Cluster? How large? How many systems per region?

See [02](./02-galaxy-and-systems.md).

### Q9 — Weather
Are there dynamic weather systems (storms, dust, rain) or only visual ambience? If dynamic, how
do they interact with the zero-simulation principle?

See [05](./05-climate-and-hydrosphere.md).

### Q10 — Hand-authored content integration
How exactly do authored descriptors/POIs merge into generated worlds without breaking
determinism or saves?

See [08](./08-narrative-layer.md), [12](./12-authoring-and-tuning.md).

### Q11 — Naming
How many naming cultures? Are names player-visible from the start? Do they need localisation?

See [08](./08-narrative-layer.md).

### Q12 — Destructibility and persistence scale
How much of the world is destructible? How are large-scale changes (a drained lake, a mined
mountain) represented without exploding save size?

See [10](./10-lod-and-streaming.md) for the voxel strategy and
[18](./18-collision-and-edits.md#large-scale-changes) for how large-scale edits are represented.

---

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| **Sameness** despite a large parameter space | The core product fails | Diversity audit; stratified sampling; landform suites; region layers |
| **Seamless flight is hard** | Central pillar unmet | Vertical slice early; LOD/seam tests; floating origin from day one |
| **Performance of voxel + procedural** | Unplayable frame times | Strict LOD; async generation; budgets; macro pyramid |
| **Scope creep in realism** | Never ships | Explicit non-goals; soft constraints; anomaly-as-content |
| **Generator drift breaks saves** | Player trust lost | Golden seeds; versioning; freeze policy |
| **Content authoring bottleneck** | Worlds feel samey in shipped content | Data-driven biomes/suites; inspector; gallery |
| **Combinatorial explosion returns** | Unmaintainable content | Axes + composition; no per-combination code |
| **"Boring" worlds feel empty** | Exploration unrewarding | POIs; rarity tiers; contrast; resources |
| **Underwater treated as a side system** | Half the game is missing | Pillar 7; dedicated vertical slice (Phase 1b); parity budgets |

---

## Roadmap

Build in this order. Each step de-risks the next.

### Phase 0 — Foundations (before content)
- Define the descriptor schema ([03](./03-planet-descriptor.md)).
- Write the data contracts ([16](./16-data-contracts.md)) and the content format
  ([17](./17-biome-and-landform-format.md)).
- Implement the seed chain and counter-based RNG ([11](./11-determinism-and-sampling.md)).
- Implement the planet inspector skeleton: seed → descriptor tree ([12](./12-authoring-and-tuning.md)).

**Exit:** you can create a body from a seed and inspect its descriptor.

### Phase 1 — One planet, end to end
- Implement `rocky` macro geology (plates + craters) at low resolution.
- Solve sea level from hypsometry; render an ocean shell.
- Run the voxel-resolution spike and record the numbers (Q2).
- Implement the cube-sphere quadtree with 4 LODs.
- Implement collision from the same field.
- Fly from orbit to ground on one planet.

**Exit:** the vertical slice. Seamless, stable frame time.

### Phase 1b — One ocean, end to end
- Extend the quadtree, collision, and streaming to the seafloor and the water volume.
- Implement oxygen, pressure, temperature, and visibility as real systems.
- Implement swimming and one underwater vehicle.
- Dive from the surface to the seafloor and back, with no seam and stable frame time.

**Exit:** underwater plays like a place, not a loading screen with fish.

### Phase 2 — Variety
- Implement climate fields and derived biomes.
- Implement 6–8 landform suites.
- Implement geological/material region layers.
- Add a barren world and a water world to prove the degenerate cases.
- Expand the aquatic layer: more benthic biomes, special volumes, biogenic landforms.
- Expand underwater content: fauna behaviour, resource nodes, underwater POIs.
- Build the diversity audit and run it on 10k descriptors.

**Exit:** the audit shows broad coverage; worlds are visibly distinct.

### Phase 3 — Systems
- Implement star generation, orbits, habitable zones.
- Implement system profiles as weight tables.
- Implement moons and rings.
- Fly between bodies in one system.

**Exit:** a whole system is explorable without loading.

### Phase 4 — Narrative
- Affinities, POIs, anomalies, naming.
- System POIs.
- Authored overrides.

**Exit:** worlds have reasons to visit.

### Phase 5 — Scale
- Galaxy regions, streaming between systems.
- Multi-system travel.
- Save/load and version migration.

**Exit:** the game's core loop works at scale.

---

## How to use this document

- When you make a decision, move it from **Open** to **Decisions** and update the relevant doc.
- When you discover a new risk, add it.
- When a phase is complete, mark it and update the roadmap.
- Link decisions to the exact section that implements them.

---

## Related

All documents. This one is the index of unresolved work:

[00](./00-vision.md) · [01](./01-world-model.md) · [02](./02-galaxy-and-systems.md) ·
[03](./03-planet-descriptor.md) · [04](./04-physical-parameters.md) ·
[05](./05-climate-and-hydrosphere.md) · [06](./06-geology-and-landforms.md) ·
[07](./07-biomes-and-ecology.md) · [08](./08-narrative-layer.md) ·
[09](./09-surface-field-stack.md) · [10](./10-lod-and-streaming.md) ·
[11](./11-determinism-and-sampling.md) · [12](./12-authoring-and-tuning.md) ·
[13](./13-generator-modules.md) · [15](./15-aquatic-environments.md) ·
[16](./16-data-contracts.md) · [17](./17-biome-and-landform-format.md) ·
[18](./18-collision-and-edits.md)
