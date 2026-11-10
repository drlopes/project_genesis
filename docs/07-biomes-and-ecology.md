# 07 — Biomes & Ecology

This document defines what a biome is in this architecture, how biomes are derived rather than
painted, how region layers combine to give visual variation, and why a barren planet is not a
special case.

Read [06 — Geology & Landforms](./06-geology-and-landforms.md) alongside this. The two layer maps
are deliberately independent.

---

## The core idea

> **A biome is a classification of the climate/ecology field, not a painted region.**

The generator does not choose "this planet is a desert planet" and then paint desert. It
computes continuous climate fields and classifies each sample:

```
biomeWeights(p) = classify(axisVector(p), stratum)
```

`axisVector` is the set of axis values defined for the sample's stratum (see
[17 — Biome & Landform Format](./17-biome-and-landform-format.md#the-axis-registry)). For an
`aerial` sample it is `temp, moisture, substrate, altitude, slope, ice`; a `benthic` sample
replaces altitude and moisture with `depth, light, current, vent, water_body`, and so on. The
classifier is the same; only the input vector and the biome table change.

On a world with strong climate variation, that produces several biomes across the surface — a
multi-biome planet, without anyone authoring it as such. On a world with no biosphere, the
classifier returns `barren` everywhere — a single-biome planet, also without a special case.

`DECIDED` — biomes are derived per-sample from fields. They are never authored as regions.

---

## Strata

Biome classification is stratified by environment, not just by climate. Every sample has a
**stratum** — `aerial`, `benthic`, `pelagic`, `subglacial`, or `subterranean` — and the stratum
selects which inputs and which biome set apply:

| Stratum | Surface or volume | Classifier inputs |
|---|---|---|
| `aerial` | surface | temperature, moisture, substrate, altitude, slope, ice |
| `benthic` | surface | depth, light, substrate, current, vent proximity, water body, temperature at depth |
| `pelagic` | volume | light, depth fraction, current, turbidity, water body |
| `subglacial` | volume + surface | no light, chemosynthesis, ice ceiling, water body |
| `subterranean` | surface | temperature, humidity, substrate |

This is what lets underwater reuse the whole classification machinery instead of becoming a
parallel system. See [15 — Aquatic Environments](./15-aquatic-environments.md).

`DECIDED` — classification is per-stratum.

---

## Biome hierarchy

Biomes have at least two levels:

```
biome            e.g. desert
└── variant      e.g. highland desert, basin desert, polar desert, coastal desert
```

Variants are chosen from the same climate inputs (altitude, moisture, latitude) but share the
biome's identity. This gives **within-biome variety** without needing a multi-biome planet.

`PROPOSED` — three levels (biome → variant → local cover) if needed. Two is enough to start.

### Initial biome set

| Biome | Climate | Notes |
|---|---|---|
| `ocean` | any with water | not a land biome; the water surface |
| `coast` | land/water boundary | beaches, tidal flats |
| `desert` | hot/cold, dry | variants: erg, hamada, polar |
| `grassland` | temperate, moderate moisture | |
| `savanna` | warm, seasonal moisture | |
| `shrubland` | dry temperate | |
| `forest` | temperate, wet | deciduous/temperate rainforest |
| `rainforest` | hot, very wet | |
| `boreal_forest` | cold, wet | taiga |
| `tundra` | cold, dry | |
| `alpine` | high altitude, any latitude | rock, snow, ice |
| `wetland` | waterlogged | marsh, bog, swamp |
| `volcanic` | active geology | ash plains, lava fields |
| `barren` | no biosphere | the catch-all for uninhabited worlds |
| `cryo` | frozen water/volatiles | ice sheets, glaciers |

`PROPOSED` — extend freely; fantasy-inspired biomes are supported by the same mechanism (they
are just climate/substrate combinations with different palettes and scatter rules).

### Aquatic biomes

Benthic biomes use the same table format with a different input vector (`depth`, `light`,
`substrate`, `current`, `vent proximity`, `water body`, `temperature at depth`) and a different
table. They are not listed here to avoid duplicating the catalogue; see
[15 — Aquatic Environments](./15-aquatic-environments.md).

The key split is **light**: above `photicDepth` the seafloor supports photosynthetic biomes
(seagrass, kelp, coral); below it, life is sparse or clustered around chemical energy (vents,
seeps). The pelagic water column is not a biome surface at all — it is a volume with zones and
scatter.

---

## Soft classification and blending

Hard classification creates visible seams. Use **soft classification**:

1. Each biome has a **suitability function** over the climate/substrate inputs.
2. Weights are computed (e.g. via softmax over suitability, or normalised radial basis).
3. Terrain detail and materials are blended using those weights.

Because blending is a property of the weighting function, there is **no separate "blend pass"**.
Transitions are smooth by construction. See [09 — Surface Field Stack](./09-surface-field-stack.md).

Additional tools for natural transitions:

- **Domain warping** — warp the climate coordinates before classification, so boundaries are
  irregular rather than smooth contours.
- **Adjacency constraints** — a biome graph can bias which biomes may neighbour which,
  preventing implausible edges (e.g. rainforest directly against polar ice) without hard rules.
- **Edge modifiers** — ecotones (forest edge, marsh margin) get their own scatter rules.

`DECIDED` — soft weights, no post-hoc blending pass.

---

## Region layers are plural

This is the mental model that resolves the "barren planets look the same all around" problem.
A planet's visual variation comes from **several independent region layers**, not just biomes:

| Layer | Driven by | Goes flat when… | Typical variety |
|---|---|---|---|
| **Ecological** | climate + biosphere | biosphere = none | low on barren worlds |
| **Geological** | tectonics, impacts, volcanism, wind | almost never | high everywhere |
| **Material** | composition, weathering, dust | rarely | medium–high |
| **Narrative** | lore tags | usually | optional |

The final look is a composition: base shape from geology, colour/tint/vegetation from ecology,
surface response from material, and overrides from narrative.

### The barren case in detail

Take a Mars-like world:

- **Ecological layer**: flat. The classifier returns `barren` at every sample because
  `biosphere = none`. One biome.
- **Geological layer**: not flat. Provinces differ by age and process — cratered highlands,
  smooth volcanic plains, a rift system, dune seas, polar caps.
- **Material layer**: not flat. Basalt, iron-oxide dust, sulfates, water ice.
- **Result**: one biome, five visually distinct regions. It does **not** look the same all
  around.

This is why the landform suite and geological province must not be derived from the biome. A
barren world has one biome but many landform suites, chosen by province.

`DECIDED` — the ecological layer is not responsible for all regional variation. Geology and
materials carry it, especially on lifeless worlds.

### Single-biome worlds are the degenerate case

A single-biome world is what you get when a classifier field is flat. It needs no special code
path:

- **Barren** — flat because no biosphere.
- **Ocean world** — flat because ocean fraction ≈ 1.
- **Ice world** — flat because temperature < freezing everywhere.
- **Desert world** — flat because moisture ≈ 0 everywhere.
- **Lava world** — flat because volcanism ≈ 1 everywhere.

Each still has geological and material variation. Each can still have sub-biome variants along
altitude or latitude (e.g. a desert world with polar frost). And a handful of genuinely uniform
worlds are valuable as contrast.

`DECIDED` — no special-case code for single-biome planets.

---

## Biosphere

The biosphere axis is separate from the terrain axes:

| Level | Meaning | Effect |
|---|---|---|
| `none` | no life | `barren` classification; no vegetation scatter |
| `microbial` | microbial life | subtle colour/atmosphere effects, no large scatter |
| `complex` | plants/animals | full biome palette and vegetation scatter |
| `exotic` | non-carbon or unusual life | alternative palettes, colours, and scatter rules |

Biosphere probability is sampled from the system profile and the planet's habitability
(temperature, pressure, water, radiation). It is never assumed.

`DECIDED` — biosphere level is stored in the descriptor and drives the ecological layer and the
atmosphere's biosignature.

---

## Biome definitions are data

A biome is not hard-coded. It is a data entry:

```toml
[biome.temperate_forest]
display_name  = "Temperate Forest"
kind          = "biome"
stratum       = "aerial"
palette       = "green_temperate"
feature_scale = 2.0

scatter = [
  { preset = "broadleaf_tree", density = 0.02, scale = [2, 4], min_lod = 3 },
  { preset = "fern",           density = 0.15, scale = [0.3, 0.8], min_lod = 4 },
]

landform_bias = [
  { suite = "rolling_hills",   weight = 1.0 },
  { suite = "river_valleys",   weight = 0.6 },
  { suite = "glacial_cirques", weight = 0.2 },
]

[biome.temperate_forest.suitability]
temp      = { range = [270, 300], falloff = 8 }
moisture  = { range = [0.45, 0.9], falloff = 0.12 }
substrate = { any_of = ["soil", "sand"] }
```

Designers add biomes by editing this table. The generator only needs to understand the schema.
See [17 — Biome & Landform Format](./17-biome-and-landform-format.md) for the full field list and
[16 — Data Contracts](./16-data-contracts.md) for the shared rules.

### Composition, not enumeration

A biome definition can be composed from modifiers. Affinities are **overlays**, not new biome
generators:

```toml
[overlay.corrupted_forest]
target      = "temperate_forest"
tint        = "#3a2b4d"
scatter_add = ["anomalous_growth", "spores"]
```

This is how affinities interact with biomes without `N × M` authoring — see
[08 — Narrative Layer](./08-narrative-layer.md) and
[17](./17-biome-and-landform-format.md#variants-and-overlays).

---

## What the biome layer contributes to the final surface

| Contribution | Example |
|---|---|
| Colour / tint | vegetation green, sand yellow, ash grey |
| Vegetation scatter | trees, shrubs, grass clumps |
| Material overrides | soil vs rock, dust cover |
| Detail modulation | grass-scale noise vs dune-scale noise |
| Ambient dressing | birds, insects, wind-blown particles |
| Gameplay | forageables, hazards, cover |

It does **not** control macro shape (geology does), sea level (hydrosphere does), or matters of
narrative (overlays do).

---

## Failure modes to avoid

- **Biome as a painted region** — produces hard borders and repetition.
- **Biome controlling landform shape** — makes barren worlds featureless.
- **One biome = one visual look** — fixed by variants and landform suites.
- **Biome palette sampled uniformly** — leads to every forest looking the same. Sample palette
  weights, density, and scatter variance per planet.
- **No adjacency constraints** — produces implausible biome neighbours.

---

## Related

- [04 — Physical Parameters](./04-physical-parameters.md) — axes that feed classification.
- [05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md) — the fields being classified.
- [06 — Geology & Landforms](./06-geology-and-landforms.md) — the independent geological layer.
- [08 — Narrative Layer](./08-narrative-layer.md) — affinity modifiers on biomes.
- [09 — Surface Field Stack](./09-surface-field-stack.md) — where classification happens in the
  evaluation order.
- [12 — Authoring & Tuning](./12-authoring-and-tuning.md) — biome data files.
- [15 — Aquatic Environments](./15-aquatic-environments.md) — benthic and pelagic biomes.
