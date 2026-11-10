# 17 — Biome & Landform Format

This document defines the concrete data format for the content layer: **biomes**, **variants**,
**overlays**, **landform suites**, **palettes**, **materials**, **scatter rules**, and **pelagic
zones**.

It is the content-side companion to [16 — Data Contracts](./16-data-contracts.md) (which defines
IDs, references, units, versioning, and merge semantics) and
[03 — The Planet Descriptor](./03-planet-descriptor.md) (which stores the *chosen* IDs and
sampled parameters).

The design goal: **a designer can add a biome or a landform suite without touching code.**

---

## Two schemas, two lifecycles

| Schema | Churn | Backed by | Changes how |
|---|---|---|---|
| **Biome / variant / overlay** | High | Pure data | Edit TOML, hot reload |
| **Landform suite** | Low–medium | A code module + typed parameters | Add a module for new *behaviour*; tune parameters in data |
| **Palette / material** | Medium | Data (IDs referenced by renderer) | Edit TOML |
| **Zone** (pelagic) | Medium | Data | Edit TOML |

Biomes are where variety is authored. Suites are stable machinery whose *parameters* vary. Do not
put behaviour in biome data, and do not put palettes in suite code.

---

## The axis registry

Suitability and context are expressed over a small, fixed set of **axes**. This registry is the
contract between the climate/geometry systems and content. Adding an axis is a schema change.

| Axis | Unit / range | Strata | Meaning |
|---|---|---|---|
| `temp` | kelvin | aerial, benthic, subterranean | annual-mean surface, at-depth, or cave temperature |
| `moisture` | 0..1 | aerial | annual-mean water availability at the surface |
| `humidity` | 0..1 | subterranean | water availability in cave air and rock |
| `substrate` | enum | aerial, benthic, subterranean | `rock`, `rubble`, `sand`, `silt`, `mud`, `soil`, `ice`, `coral`, `organic` |
| `altitude` | metres | aerial | height above sea level |
| `slope` | degrees | aerial, benthic | local slope |
| `latitude` | degrees | all | absolute latitude |
| `depth` | metres | benthic | water surface to sample |
| `depth_fraction` | 0..1 | benthic, pelagic | depth normalised by total ocean depth |
| `light` | 0..1 | benthic, pelagic | fraction of surface light reaching the sample |
| `current` | 0..1 | benthic, pelagic | normalised current speed |
| `vent` | 0..1 | benthic | proximity/intensity of a vent or seep |
| `water_body` | enum | benthic, pelagic | `marine`, `freshwater`, `brine`, `hydrocarbon`, `ammonia`, `subsurface` |
| `ice` | 0..1 | all | ice cover |

Stratum semantics are defined in [15 — Aquatic Environments](./15-aquatic-environments.md) and
[07 — Biomes & Ecology](./07-biomes-and-ecology.md). Climate axes (`temp`, `moisture`) are annual
means; seasons and day/night deltas never enter classification. See
[05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md#time-mean-versus-instantaneous).

---

## Biome schema

Biomes live in `data/<pack>/biomes/*.toml`, one or more per file. The table key is the ID; inside
the `core` pack, `[biome.temperate_forest]` declares `core:biome/temperate_forest`. Overlays use
their own prefix — `[overlay.corrupted_forest]` declares `core:overlay/corrupted_forest` — and
therefore need no `kind` field. The table prefix always declares the type.

```toml
[biome.temperate_forest]
display_name  = "Temperate Forest"
kind          = "biome"          # biome | variant
stratum       = "aerial"
weight        = 1.0              # global bias, default 1.0
palette       = "green_temperate"
feature_scale = 2.0              # metres; smallest feature, see 16
tags          = ["forest", "temperate"]

scatter = [
  { preset = "broadleaf_tree", density = 0.02, scale = [2, 4], slope_max = 30, min_lod = 3 },
  { preset = "fern",           density = 0.15, scale = [0.3, 0.8], min_lod = 4 },
]

landform_bias = [
  { suite = "rolling_hills",   weight = 1.0 },
  { suite = "river_valleys",   weight = 0.6 },
  { suite = "glacial_cirques", weight = 0.2 },
]

gameplay = { forage = ["berries", "mushrooms"], cover = 0.6, hazard = "low" }

[biome.temperate_forest.suitability]
temp      = { range = [270, 300], falloff = 8 }
moisture  = { range = [0.45, 0.9], falloff = 0.12 }
substrate = { any_of = ["soil", "sand"] }
altitude  = { range = [0, 1800], falloff = 400 }
```

Fields:

| Field | Required | Notes |
|---|---|---|
| `display_name` | yes | UI only; localisable |
| `kind` | yes (biomes only) | `biome` or `variant`; overlays are a separate type |
| `stratum` | yes (biome) | `aerial`, `benthic`, `pelagic`, `subglacial`, `subterranean` |
| `weight` | no | Global bias before suitability; default 1.0 |
| `palette` | yes | Reference to a palette ID |
| `material_tint` | no | Overrides the palette tint |
| `feature_scale` | yes | Metres; drives LOD validation |
| `suitability` | yes | Axis expression map; see below |
| `scatter` | no | Inline rules or preset references |
| `landform_bias` | no | Suite weights for the geological province |
| `tags` | no | Free-form, used by overlays and queries |
| `gameplay` | no | Forageables, hazards, cover, etc. |

Fields marked **yes** may be inherited through `extends` (except `kind`, which is always
explicit); the fully resolved entry must have them.

Shorthand: a bare string in `scatter` means `{ preset = "<id>" }` with default density and
scale; a bare string in `landform_bias` means `{ suite = "<id>", weight = 1.0 }`. The long form is
preferred when a value differs from the default.

### Suitability expressions

Each axis maps to an expression. Four forms:

```toml
# 1. Range with soft edges: 1 inside, 0 beyond falloff
temp = { range = [270, 300], falloff = 8 }

# 2. Piecewise-linear curve, clamped at the ends
moisture = { curve = [[0.0, 0.0], [0.3, 0.6], [0.6, 1.0], [1.0, 0.8]] }

# 3. Enum membership
substrate = { any_of = ["soil", "sand"] }
# or: substrate = { none_of = ["ice", "rock"] }

# 4. Weight multiplier (optional on any form)
altitude = { range = [0, 1800], falloff = 400, weight = 0.8 }
```

### Scoring and blending

```
score(biome, sample) = Π  axis_score(axis)
                       (or max if mode = "any")

weighted(biome)      = score × weight
biome_weights        = normalise(weighted)   # softmax with tunable sharpness
```

- The default combine mode is **product** (all conditions must hold, softly).
- `mode = "any"` on the suitability map uses the maximum instead.
- Normalisation is a softmax with a global `sharpness` (softer = smoother transitions).
- If all scores are zero, the classifier falls back to `weight` alone; if that is also zero, it
  returns the stratum's `barren`-equivalent. This should be rare and is logged in the inspector.

There is no separate blend pass. Soft weights *are* the blend (see
[09 — Surface Field Stack](./09-surface-field-stack.md)).

---

## Variants and overlays

### Variants

A variant narrows a parent's suitability and modifies scatter. It shares the parent's identity
for UI and gameplay but is its own entry.

```toml
[biome.temperate_forest_alpine]
kind    = "variant"
extends = "temperate_forest"

scatter_add    = ["alpine_shrub", "rock_outcrop"]
scatter_remove = ["broadleaf_tree"]

[biome.temperate_forest_alpine.suitability]
temp     = { range = [250, 275], falloff = 4 }
altitude = { range = [1200, 3000], falloff = 300 }
```

Rules (from [16](./16-data-contracts.md)):

- `extends` takes exactly one parent.
- Scalars replace; maps merge; lists replace unless `_add`/`_remove` is used.
- A bare string in `scatter_add` / `scatter_remove` is shorthand for a preset reference with
  default density and scale, e.g. `"alpine_shrub"` means `{ preset = "alpine_shrub" }`.
- A variant may not change `stratum`.
- Variants are selected by the same classifier; they compete with all other biomes.

### Overlays

An overlay applies a narrative or affinity modification to a target. It does not generate terrain;
it modifies the resolved appearance and scatter. Overlays live in `overlays/*.toml` and are their
own content type, so the table prefix declares them and no `kind` is needed.

```toml
[overlay.corrupted_forest]
target  = "temperate_forest"
tint    = "#3a2b4d"
material_override = "corrupted_soil"
scatter_add = ["anomalous_growth"]
```

```toml
[overlay.infected_wetland]
target = { tag = "wetland" }       # or an explicit ID
tint   = "#4a5a2f"
scatter_add = ["fungal_mat", "spore_cloud"]
hazard_add  = "biological"
```

| Field | Required | Notes |
|---|---|---|
| `target` | yes | A biome ID, or `{ tag = "..." }` to target a group |
| `tint` | no | sRGB hex; multiplied into the resolved palette |
| `material_override` | no | Replaces the resolved base material |
| `scatter_add` / `scatter_remove` | no | Same shorthand as variants |
| `hazard_add` / `hazard_remove` | no | Gameplay hazard tags |
| `priority` | no | Ordering when several overlays match; lower runs first, ties broken by ID |

Overlays apply after variants, in `priority` then ID order, so the resolved entry never depends
on file load order (see [16](./16-data-contracts.md#precedence)).

Overlays are selected by the affinity layer (see
[08 — Narrative Layer](./08-narrative-layer.md)) and applied after the base biome resolves. They
never mutate the base entry.

---

## Landform suite schema

A landform suite is a **code module** with a **typed parameter schema**. The data chooses the
module and supplies parameters; it does not contain behaviour.

```toml
[suite.dune_seas]
module        = "dune_field"
feature_scale = 4.0          # metres
min_lod       = 2
max_lod       = 4

scatter = [
  { preset = "boulder", density = 0.005, scale = [1, 3], min_lod = 3 },
]

[suite.dune_seas.context]
strata     = ["aerial"]
substrates = ["sand"]
biomes     = ["desert"]
slope      = [0, 15]

[suite.dune_seas.params]
dune_height  = 8.0
dune_spacing = 120.0
asymmetry    = 0.7
dust_cover   = 0.4
orientation  = [0.3, 0.0, 1.0]
```

Fields:

| Field | Required | Notes |
|---|---|---|
| `module` | yes | Code key; must exist in the module registry |
| `feature_scale` | yes | Smallest feature; drives LOD and the voxel-size warning |
| `min_lod` / `max_lod` | no | Coarsest/finest LOD; `max_lod` defaults to 4, `min_lod` derives from `feature_scale` |
| `context` | no | Where the suite is allowed; empty = anywhere compatible |
| `params` | yes | Validated against the module's parameter schema |
| `scatter` | no | Suite-specific features; same rule schema as biomes |
| `weight` | no | Default bias among compatible suites |

### Module parameter schema

Each module declares its parameters in code, and that declaration is surfaced to tools and the
validator. A module's schema looks like this (illustrative — the real one lives in code):

```toml
[module.dune_field.params]
dune_height  = { type = "float", default = 5.0,  min = 0.0,  max = 50.0,  unit = "m", step = 0.5 }
dune_spacing = { type = "float", default = 80.0, min = 10.0, max = 500.0, unit = "m", step = 5.0 }
asymmetry    = { type = "float", default = 0.5,  min = 0.0,  max = 1.0 }
dust_cover   = { type = "float", default = 0.0,  min = 0.0,  max = 1.0 }
orientation  = { type = "vec3",  default = [0.0, 0.0, 1.0] }
```

### Parameter types

| Type | Example | Notes |
|---|---|---|
| `float` | `8.0` | `min`, `max`, `step`, `unit` |
| `int` | `12` | same |
| `bool` | `true` | |
| `enum` | `"transverse"` | `values` list |
| `vec3` | `[0.3, 0.0, 1.0]` | normalised where documented |
| `color` | `"#6b7d4a"` | sRGB hex |
| `curve` | `[[0, 0], [0.5, 1], [1, 0]]` | piecewise linear |
| `range` | `[2, 8]` | min/max pair |

The parameter schema is what lets the inspector render sliders, the validator reject nonsense,
and designers tune without reading code.

### Context matching

A suite is eligible for a sample when all declared context conditions hold:

- `strata` — the sample's stratum is in the list,
- `substrates` — the substrate is in the list,
- `biomes` — the biome weight is non-zero for at least one listed biome,
- `slope` — the sample's slope is within range.

Eligible suites are weighted by `weight × landform_bias` and normalised. The terrain is the
weighted sum of suite contributions (plus the base terrain), as described in
[09](./09-surface-field-stack.md). Suites do not replace each other; they blend.

Suite `scatter` is evaluated only where the suite is active and adds to the biome's scatter. It
obeys the same density, LOD, and instance-cap rules as any other scatter preset.

---

## Palettes and materials

Palettes are the visual identity of a biome; materials are the physical surface response. They
are separate so one palette can be reused with different materials.

```toml
[palette.green_temperate]
base      = "soil"          # material ID shorthand
accent    = "grass"
tint      = "#6b7d4a"
roughness = 0.8
```

```toml
[material.basalt]
albedo         = "#3a3a3a"
roughness      = 0.9
wet_variant    = "basalt_wet"
scatter_tint   = "#2e2e2e"
```

```toml
[material.coral_rubble]
albedo      = "#c9b8a0"
roughness   = 0.7
wet_variant = "coral_rubble_wet"
```

Rules:

- A biome references a palette, not a material, for its base look.
- A palette references materials by ID.
- Aquatic materials should declare a `wet_variant` where relevant; the renderer blends to it
  below the waterline.
- `tint` is the single override that affinity overlays most often change.

---

## Scatter rules

Scatter is split into **anchored** (surface) and **free-floating** (volume), per
[15](./15-aquatic-environments.md). A rule declares which.

```toml
[scatter.broadleaf_tree]
anchor        = "surface"      # surface | volume | both
density       = 0.02           # instances per square metre (surface) or cubic metre (volume)
scale         = [2, 4]         # metres
slope_max     = 30             # degrees
altitude      = [0, 1800]      # metres; empty = no limit
clustering    = 0.4            # 0 = Poisson, 1 = strongly clumped
min_lod       = 3              # full instances from LOD 3 inward
cull_distance = 800            # metres
instance_cap  = 4000           # per chunk
```

```toml
[scatter.fish_school]
anchor  = "volume"
zone    = "photic"
density = 0.01
scale   = [1, 2]
min_lod = 3
```

Rules:

- `anchor = "surface"` requires the sample to be above the waterline (or benthic, if the biome is
  benthic) and respects `slope_max`, `altitude`, and `depth`.
- `anchor = "volume"` requires a matching zone and ignores terrain slope; it must not place
  instances inside terrain.
- `anchor = "both"` is for presets that have an anchored and a free-floating part (kelp holdfasts
  plus drifting fronds); each part is placed by its own rule.
- `density` is an expectation, not a count; actual placement is deterministic from the chunk seed
  (see [11](./11-determinism-and-sampling.md)).
- `instance_cap` protects against pathological density.
- `max_lod` and `cull_distance` are mandatory in practice; omitting them defaults to the global
  scatter budget.

Biomes reference presets by name, and may override density/scale per biome.

| Field | Applies to | Notes |
|---|---|---|
| `anchor` | all | `surface`, `volume`, or `both` |
| `density` | all | Instances per m² (surface) or m³ (volume) |
| `scale` | all | Min/max in metres |
| `feature_scale` | all | Defaults to `scale[0]`; drives LOD band-limiting |
| `slope_max`, `altitude` | surface | Terrain constraints |
| `depth`, `zone` | benthic surface, volume | Depth range and pelagic zone |
| `clustering` | all | 0 = Poisson, 1 = strongly clumped |
| `min_lod`, `max_lod`, `cull_distance`, `instance_cap` | all | LOD and budget limits |

---

## Pelagic zones

Zones are not biomes. They describe the water column's depth bands and their ambient properties.

```toml
[zone.twilight]
depth_fraction = [0.15, 0.4]
light          = [0.001, 0.05]
scatter        = { plankton = 0.2, fish = 0.15, jellies = 0.4, leviathan = 0.005 }
fog            = { density = 0.04, color = "#0b2b3a" }
hazards        = { pressure = 0.3 }
```

| Field | Notes |
|---|---|
| `depth_fraction` | Normalised by total ocean depth (see [15](./15-aquatic-environments.md)) |
| `light` | Expected light range; checked against the body's `photicDepth` |
| `scatter` | Species weights; normalised |
| `fog` | Density and colour; drives underwater visibility |
| `hazards` | Pressure, temperature, toxins; gameplay inputs |

Zones are ordered and must not overlap. Special volumes (vent plumes, brine layers) are separate
entries, not zones.

---

## Validation rules

In addition to the shared checks in [16](./16-data-contracts.md):

1. `stratum` is valid for the entry kind.
2. Every suitability axis exists in the registry and is legal for the stratum.
3. Ranges are ordered (`min <= max`) and curves are monotonic in x.
4. `any_of` / `none_of` values are legal enum members.
5. `extends` does not change `stratum`.
6. Variants form a tree with no cycles and depth ≤ 8.
7. Overlay targets resolve.
8. Suite `module` exists; `params` match the module schema.
9. `feature_scale > 0`; warn if below the current voxel size.
10. `min_lod <= max_lod`.
11. Scatter `anchor`, `slope_max`, `altitude`, `depth`, and `zone` are consistent.
12. Zone `depth_fraction` ranges do not overlap.
13. Weights are non-negative; normalisation is reported.

---

## Acceptance test

The format is good enough when, with no code changes, you can:

- author **20 biomes**, including 3 variants and 2 overlays,
- author **8 landform suites** with typed parameters,
- validate all of it in CI,
- open the inspector and see the palettes, suitability, scatter, and suite weights resolve for a
  sampled world,
- hot-reload an edit and see it in the preview.

If a designer needs code to add a biome, the format failed.

---

## Worked examples

### Benthic biome

```toml
[biome.coral_reef]
display_name = "Coral Reef"
kind         = "biome"
stratum      = "benthic"
palette      = "reef_tropical"
feature_scale = 1.0

scatter = [
  { preset = "staghorn_coral", density = 0.4, scale = [0.5, 2.0], min_lod = 3 },
  { preset = "reef_fish",      density = 0.05, min_lod = 3 },
]

landform_bias = [
  { suite = "reef_framework",     weight = 1.0 },
  { suite = "carbonate_platform", weight = 0.4 },
]

[biome.coral_reef.suitability]
depth     = { range = [0, 60], falloff = 20 }
light     = { range = [0.05, 1.0], falloff = 0.05 }
substrate = { any_of = ["rock", "rubble"] }
temp      = { range = [295, 305], falloff = 3 }
```

### Suite

```toml
[suite.reef_framework]
module        = "reef_growth"
feature_scale = 0.5

[suite.reef_framework.context]
strata     = ["benthic"]
substrates = ["rock", "rubble"]

[suite.reef_framework.params]
spur_height     = 1.5
spur_spacing    = 6.0
crest_sharpness = 0.7
lagoon_depth    = 3.0
```

### Zone

```toml
[zone.abyss]
depth_fraction = [0.75, 1.0]
light          = [0.0, 0.001]
scatter        = { detritivores = 0.02, leviathan = 0.001 }
fog            = { density = 0.09, color = "#04080c" }
hazards        = { pressure = 0.9 }
```

---

## Related

- [16 — Data Contracts](./16-data-contracts.md) — IDs, units, versioning, merge rules.
- [03 — The Planet Descriptor](./03-planet-descriptor.md) — where chosen IDs and params are stored.
- [06 — Geology & Landforms](./06-geology-and-landforms.md) — what suites do.
- [07 — Biomes & Ecology](./07-biomes-and-ecology.md) — classification and strata.
- [09 — Surface Field Stack](./09-surface-field-stack.md) — evaluation order.
- [12 — Authoring & Tuning](./12-authoring-and-tuning.md) — tooling and workflow.
- [15 — Aquatic Environments](./15-aquatic-environments.md) — benthic biomes and pelagic zones.
