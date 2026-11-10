# 08 — Narrative Layer

The narrative layer is everything about a world that is **not** physical: affinities, anomalies,
ruins, factions, points of interest, and naming.

It sits *on top of* the physical and ecological generation. It never changes how terrain is
shaped, only how it looks, what is placed on it, and what it means.

This separation is deliberate:

- Physical worlds stay plausible and varied without lore constraints.
- The same physical world can host different story flavours (a temperate forest world can be
  pristine, corrupted, or war-torn).
- Content and narrative can be added by designers without touching generation code.
- Contradictions can be explained by lore instead of being removed.

---

## Affinities

An **affinity** is a narrative overlay that changes the *feel* of a world: its palette, its
ambient effects, its hazards, and its scatter.

| Affinity | Feel | Typical effects |
|---|---|---|
| `gaia` | thriving, harmonious | richer palette, more vegetation, benign wildlife |
| `wild` | untamed, dangerous | dense growth, aggressive fauna, less structure |
| `mechanical` | industrialised, artificial | metal structures, machinery, no vegetation, drone fauna |
| `infected` | biological contamination | sickly palette, fungal growth, spores, decay |
| `corrupted` | reality damage / anomaly | wrong colours, glitches, impossible geometry, hazards |
| `precursor` | ancient alien | ruins, monoliths, dormant technology |
| `derelict` | abandoned | ruins, wreckage, no active life |
| `pristine` | untouched | no artificial content at all |

`PROPOSED` — this list should be data (see [12](./12-authoring-and-tuning.md)). Affinities are a
tag plus a set of modifier entries, not code branches.

### Affinity as a modifier stack

Applied in order, after base generation:

```
base material
  → affinity material override       (e.g. corrupted → sickly emissive tint)
  → affinity biome overlay           (e.g. infected → fungus scatter on forest)
  → affinity atmosphere/sky change   (e.g. mechanical → smog)
  → affinity hazard rules            (e.g. corrupted → radiation zones)
  → affinity POI set                 (e.g. precursor → monoliths)
```

This is why the system does not need `N biomes × M affinities` generators. Affinities compose
with everything because they are overrides, not new generators.

`DECIDED` — affinity is a post-generation modifier layer, not a physical axis.

### The `barren` + `precursor` example

An Ancient system can have a barren world with a `precursor` affinity: same barren terrain
(one biome, geological provinces), plus monoliths, ruins, and an anomalous sky. The physical
world is unchanged; the narrative makes it a place.

---

## Anomalies

Anomalies are the deliberate exceptions to physical plausibility. They are **content**, not
bugs.

- The generator may produce a world that violates a soft constraint (see
  [04 — Physical Parameters](./04-physical-parameters.md)).
- If it does, the world is tagged as an anomaly and given a narrative explanation: geothermal,
  artificial, tidally heated, greenhouse runaway, reality damage, precursor engineering.
- Anomalies should be **rare** and **memorable**.

`DECIDED` — plausibility violations are allowed, but must be tagged and explained.

### Anomaly types

| Type | Example | Explanation hook |
|---|---|---|
| Thermal | temperate world far outside the habitable zone | geothermal, tidal, artificial heating |
| Hydrological | ocean world with no atmosphere | subsurface ocean, ice shell |
| Biological | complex life on a hostile world | exotic biosphere, protected ecosystem |
| Structural | impossible geology | artificial construction, reality anomaly |
| Temporal | signs of a civilisation older than the star | precursor interference |
| Spatial | impossible position or orbit | captured body, engineered orbit |

---

## Points of interest (POIs)

POIs are placed at bake time with **stable IDs** and stored in the descriptor. They are what
gives a world a reason to visit.

### POI types

| Type | Examples | Placement rules |
|---|---|---|
| **Landmark** | monolith, arch, crater, waterfall | visible from distance; respects terrain |
| **Ruin** | settlement, temple, outpost | flat, dry, reachable |
| **Derelict** | crashed ship, abandoned station | any terrain, often dramatic |
| **Cave system** | cave entrance, underground complex | in rock, not underwater unless intentional |
| **Resource** | mine, deposit, geyser | geology-aware |
| **Anomaly site** | strange signal, rift, artefact | narrative-driven |
| **Settlement** | town, camp, station | flat, near water, habitable |

### Placement constraints

POIs are placed using coarse fields, before fine terrain is generated, so placement can respect:

- elevation (not underwater, not on a cliff face, unless intended)
- slope (flat enough for a settlement)
- proximity to water, resources, or other POIs
- biome (a ruin in a desert vs in a forest)
- narrative layer (faction territory, anomaly clusters)

Because the descriptor holds only positions, the terrain around a POI must be **conformed** at
generation time to fit it (flatten a pad, carve a cave mouth) or the POI must be chosen to fit
the terrain. Prefer the latter; use the former sparingly.

`DECIDED` — POIs have stable IDs and are placed from coarse fields at bake time.

### Stable IDs and persistence

A POI ID is derived from the body seed and an index (or a spatial hash). It must survive
regeneration so that saves, quests, and discoveries remain valid. See
[11 — Determinism & Sampling](./11-determinism-and-sampling.md).

---

## System and galaxy narrative

The system profile (see [02](./02-galaxy-and-systems.md)) contributes narrative at a larger
scale:

- **Faction presence** — who owns the system, what they built, what they left behind.
- **History** — war, plague, quarantine, abandonment, discovery.
- **System POIs** — stations, derelicts, anomalies between orbits.
- **Naming culture** — the language family used to name bodies.

These are tags and placements, not terrain changes. A war-torn system has the same physical
planets with different debris, damage overlays, and dangers.

---

## Naming

Names should be generated from **cultures** (syllable inventories, morphology rules), assigned
per region or faction, so that neighbouring systems sound related and distant ones do not.

- Bodies get a designation (catalogue-style) and optionally a local name.
- Names are stored in the descriptor so they are stable.
- Naming is pure flavour and must never affect generation.

`PROPOSED` — a small syllable-based generator per culture is enough.

---

## Content budget and rarity

Variety is not only about parameters; it is about **pacing**.

`PROPOSED` — define rarity tiers:

- **Common** — ordinary worlds, useful for resources and contrast.
- **Uncommon** — strong biome/landform combinations, unusual skies.
- **Rare** — anomalies, ruins, unique set pieces.
- **Unique** — hand-authored or one-per-galaxy content.

Aim for a distribution that guarantees the player encounters something remarkable at a
predictable cadence, rather than relying on pure randomness. See
[11 — Determinism & Sampling](./11-determinism-and-sampling.md).

### Hand-authored set pieces

Some content is better authored than generated: a specific crashed ship, a story location, a
landmark visible from orbit. The architecture supports this by letting a descriptor (or a POI)
be **authored** and merged into a generated world, rather than generated from scratch.

`PROPOSED` — descriptors support an `authoredOverrides` block merged at bake.

---

## Related

- [02 — Galaxy & Systems](./02-galaxy-and-systems.md) — system profiles and system POIs.
- [04 — Physical Parameters](./04-physical-parameters.md) — anomalies and soft constraints.
- [07 — Biomes & Ecology](./07-biomes-and-ecology.md) — affinity modifiers on biomes.
- [09 — Surface Field Stack](./09-surface-field-stack.md) — where narrative overrides apply.
- [12 — Authoring & Tuning](./12-authoring-and-tuning.md) — affinity and POI data.
