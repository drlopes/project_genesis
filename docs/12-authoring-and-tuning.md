# 12 — Authoring & Tuning

This document describes how content is authored and tuned: the data files, the schemas, the
tooling, and the conventions that keep generation data-driven.

The goal: **adding variety should be a data edit, not a code change.** A designer should be able
to add a biome, a landform suite, a system profile, or an affinity without touching the
generator.

---

## Data-driven by default

Everything that is a *choice* or a *tuning constant* lives in data files:

| Data | Contents | Example doc |
|---|---|---|
| **System profiles** | weighting tables, narrative tags, POI density | [02](./02-galaxy-and-systems.md) |
| **Body-class weights** | how often each class appears at a given orbit | [04](./04-physical-parameters.md) |
| **Attribute ranges** | per-axis min/max/distribution | [04](./04-physical-parameters.md) |
| **Derived-label rules** | thresholds for `barren`, `habitable`, etc. | [04](./04-physical-parameters.md) |
| **Biomes** | suitability, palette, scatter, tint, landform bias | [07](./07-biomes-and-ecology.md) |
| **Landform suites** | feature modules and parameters | [06](./06-geology-and-landforms.md) |
| **Materials** | surface responses, colours, roughness | [09](./09-surface-field-stack.md) |
| **Affinities** | modifier stack entries | [08](./08-narrative-layer.md) |
| **POI types** | placement rules, contents | [08](./08-narrative-layer.md) |
| **Naming** | cultures, syllables, morphology | [08](./08-narrative-layer.md) |

Code owns *mechanism* (the field stack, the generators, the math). Data owns *choices*.

`DECIDED` — data-driven tuning is a hard requirement.

---

## Format and schema

Recommended: **TOML** for hand-authored content (readable, comment-friendly) and
**JSON/MessagePack** for baked descriptors (machine-written).

The rules live in two places:

- [16 — Data Contracts](./16-data-contracts.md) — IDs, references, units, versioning, merge
  semantics, LOD metadata, and validation.
- [17 — Biome & Landform Format](./17-biome-and-landform-format.md) — the concrete biome, suite,
  palette, material, scatter, and zone schemas.

This document covers the *workflow*; it does not restate the schemas.

`PROPOSED` — generate the validator from the code-side structs so data and code cannot drift.

### Hot reload

In-editor hot reload of data files is worth the investment early. The tuning loop for a game
about variation is *sample → see → adjust → resample*. If that loop requires a rebuild, it will
not be used enough.

---

## Composition over duplication

Data entries should be composable:

```toml
# A base biome
[biome.temperate_forest]
display_name  = "Temperate Forest"
kind          = "biome"
stratum       = "aerial"
palette       = "green_temperate"
feature_scale = 2.0
scatter       = ["broadleaf_tree", "fern"]

[biome.temperate_forest.suitability]
temp     = { range = [270, 300], falloff = 8 }
moisture = { range = [0.45, 0.9], falloff = 0.12 }

# A variant inherits and overrides
[biome.temperate_forest_alpine]
kind    = "variant"
extends = "temperate_forest"
scatter_add = ["alpine_shrub"]

[biome.temperate_forest_alpine.suitability]
temp     = { range = [250, 275], falloff = 4 }
altitude = { range = [1200, 3000], falloff = 300 }

# An affinity overlays a biome
[overlay.corrupted_forest]
target      = "temperate_forest"
tint        = "#3a2b4d"
scatter_add = ["anomalous_growth"]
```

This is how you get variety without `N × M` duplication. Inheritance/override should be
explicit and validated (cycles are an error).

---

## Tooling

The tools are as important as the generator. Build these early, because without them tuning is
guesswork.

### 1. Planet inspector

`seed → descriptor → preview`

- Show the full descriptor in a readable tree.
- Show a 2D/3D preview of the macro field, biome map, geological map, and material map.
- Show the derived labels and why each was assigned.
- Allow editing a parameter and instantly rebaking (with the same seed).

This is the primary iteration loop.

### 2. Seed browser

- Navigate systems/bodies by seed.
- Jump between interesting worlds (bookmarked golden seeds).
- Compare two worlds side by side.

### 3. Gallery

- Render a grid of N planets from a seed range.
- Sort/filter by derived labels.
- This is how you *see* sameness quickly.

### 4. Diversity report

- The audit from [11 — Determinism & Sampling](./11-determinism-and-sampling.md).
- Histograms, clusters, coverage, rare-label counts.
- Run on demand and as a regression check.

### 5. Debug overlays in-game

- Current body descriptor.
- LOD levels and patch boundaries.
- Biome weights at the player's position.
- Temperature, moisture, elevation, water depth.
- Seed chain for the current chunk.

These should be toggleable and cheap. They are how you diagnose "why does this look wrong".

`PROPOSED` — start with the inspector and the overlay; the rest can follow.

---

## Content pipeline

### Procedural-first, authored-second

Most content is generated. Hand-authored content is merged in via overrides:

- **Authored descriptor overrides** — a fixed body with specified parameters, merged at bake.
- **Authored POIs** — inserted into the POI list with stable IDs.
- **Authored set pieces** — a specific landmark or ruin placed at a known location.

The merge must be explicit and versioned, and must not break determinism.

### Content budgets

Define per-system and per-planet budgets for:

- number of POI types active,
- scatter instance counts per chunk,
- texture/material variants,
- unique set pieces per region.

Budgets prevent the generator from producing a world that is technically varied but
unplayably heavy.

### Variation budgets and per-body draws

Sameness usually comes from drawing a decision per-sample when it should have been per-body. The
contract is to draw each choice at the **coarsest scale that owns it**:

| Scale | Examples | Seed | Stored |
|---|---|---|---|
| **Per-body** | palette variant weights, suite parameter values, material tint/roughness jitter, scatter density multiplier | `contentSeed` | descriptor (`biomePalette`, `landformSuites`, `materials`) |
| **Per-region** | province palette bias, region biome weights, suite selection bias | `regionsSeed` | descriptor (`regions`) |
| **Per-chunk** | scatter instance placement, micro-prop scatter, local variation | `chunkSeed` / `scatterSeed` | regenerated cache |
| **Per-sample** | biome weights, material blend, height deltas | none — pure function of the fields | never stored |

Rules:

- **A per-body choice is frozen at bake.** If a temperate forest on body A is cooler and bluer than
  on body B, that is a per-body draw recorded in both descriptors. It does not change with the chunk
  the player stands in.
- **Per-sample decisions are continuous.** They may vary smoothly with the fields, but they must not
  jump at chunk boundaries.
- **Variation is sampled, not enumerated.** A biome declares a palette *family* and a scatter
  *range*; bake draws the per-body point. Do not ship `forest_1`, `forest_2`, `forest_3` as separate
  biomes.
- **The draw is part of identity.** Two bodies with the same seed chain and the same
  `contentVersion` get the same palette. Changing the draw is a `generatorVersion` change.
- **The diversity audit is the guard.** If a scale is under-used, the audit's entropy and
  nearest-neighbour metrics expose it before players do. See
  [11](./11-determinism-and-sampling.md#the-diversity-audit).

`DECIDED` — per-body and per-region choices are baked and frozen; per-chunk choices are seeded and
regenerated; per-sample choices are pure.

---

## Profiling and tuning hooks

- Per-layer timing (L0–L4) in the inspector.
- Chunk generation time histogram.
- Counts of patches per LOD.
- Memory per resident body.
- Scatter instance counts per chunk.

Tune against these, not vibes.

---

## Versioning of data

- Data files have a schema version.
- Baked descriptors reference data by ID; changing a biome's parameters changes all worlds that
  use it.
- Decide whether data changes are "cosmetic" (safe to apply to existing saves) or "structural"
  (require migration). This mirrors the descriptor version policy in
  [03](./03-planet-descriptor.md).

`OPEN` — whether data IDs are stable forever (renaming requires aliases) or versioned per
release.

---

## Conventions for agents and contributors

- **Add concepts to exactly one doc**, and link from others.
- **Prefer new data entries to new code branches.** If you are writing `if biome == "desert"`,
  the behaviour probably belongs in the biome's data.
- **Every derived label must have a documented rule** in
  [04](./04-physical-parameters.md).
- **Every new generator must satisfy the contract** in
  [13 — Generator Modules](./13-generator-modules.md).
- **Run the diversity audit** after any change to weights or palettes.
- **Update golden seeds** deliberately, never accidentally.

---

## Related

- [03 — The Planet Descriptor](./03-planet-descriptor.md) — the data the tools display.
- [04 — Physical Parameters](./04-physical-parameters.md) — label rules in data.
- [07 — Biomes & Ecology](./07-biomes-and-ecology.md) — biome data.
- [08 — Narrative Layer](./08-narrative-layer.md) — affinity and POI data.
- [11 — Determinism & Sampling](./11-determinism-and-sampling.md) — the audit.
