# 15 — Aquatic Environments

This document covers everything below the waterline: the seafloor (benthic), the water column
(pelagic), ice-covered oceans, and the shoreline interface.

Underwater is a **first-class pillar**, equal in weight to the planetary surface. This is not a
side-system document: it defines a peer environment with its own generation, gameplay, content,
and budget.

It exists because underwater is not just "more biomes". It is a **second kind of environment**:
half *surface* and half *volume*. The surface half reuses the existing classifier; the volume
half needs a new query type. Getting that abstraction in early is cheap; retrofitting it into
the LOD and streaming layers later is not.

---

## Why this is more than a biome list

The main architecture has one kind of query:

```
surfaceSample(position, lod) → { height, material, biome weights, scatter }
```

That works everywhere there is ground under the player. Underwater it is only half the story:

- **The seafloor** is a surface. It can reuse the entire L0–L4 stack, with a water-aware
  classifier (depth and light instead of altitude and insolation).
- **The water column** has no height field. It needs a **volume query** that returns
  zone, light, current, turbidity, scatter density, and hazards at an arbitrary point in space.

Trying to express the water column as terrain produces nonsense (an "underwater mountain" made
of fog). Treating the seafloor as a special case duplicates the classifier. The clean split is
surface + volume.

`DECIDED` — the runtime exposes two sample types: **surface** and **volume**. See
[09 — Surface Field Stack](./09-surface-field-stack.md).

---

## Strata

Rather than a boolean `underwater` flag, compute the **stratum** of a sample and let it select
the classifier and scatter rules:

```
stratum(p):
    inside terrain                     → subterranean
    below water surface, on seafloor   → benthic
    below water surface, above bottom  → pelagic
    under an ice shell                 → subglacial
    otherwise                          → aerial
```

| Stratum | What it is | Query | Notes |
|---|---|---|---|
| `aerial` | land surface and atmosphere | surface | the existing land pipeline |
| `benthic` | the seafloor / lakebed | surface | land pipeline + water-aware classifier |
| `pelagic` | the water column | volume | zones, scatter, fog, hazards |
| `subglacial` | sealed ocean under ice | volume + surface | no surface access; chemosynthesis |
| `subterranean` | caves and flooded caves | volume + surface | cave volumes; `floodedCaves` lists the water-filled ones |

The stratum generalises cleanly to caves and ice moons. It also gives the renderer and the
gameplay systems a single word to switch on, instead of scattered `isUnderwater` checks.

Cave geometry is a sampled volume, not a stored grid; collision and the solid model for caves are
in [18 — Collision & Edits](./18-collision-and-edits.md#the-solid-model).

`PROPOSED` — the stratum is computed per sample, not stored. It is a function of the water
surface radius, the terrain height, and the ice shell.

---

## Benthic environments

The seafloor is classified with the **same soft-classification machinery** as land biomes, using
a water-specific input vector.

### Classification inputs

| Input | Replaces | Notes |
|---|---|---|
| **depth** | altitude | `waterSurface − height` |
| **light** | insolation | `exp(−k · depth) × surfaceIllumination`; `k` from turbidity |
| **substrate** | substrate | sand / silt / rock / rubble / coral / ice, from material + current |
| **temperature at depth** | surface temperature | the two-layer water profile in [05](./05-climate-and-hydrosphere.md#water-temperature-at-depth) |
| **current speed** | wind | analytic current field |
| **vent / seep proximity** | — | hydrothermal and cold-seep sites from bake |
| **water body type** | — | marine / freshwater / brine / hydrocarbon / ammonia |
| **ice cover** | — | seasonal caps, permanent shelves, full shells |

**Light is the dominant axis.** It separates photosynthesis from chemosynthesis, which is the
most legible distinction underwater and the one players will infer immediately: bright, colourful
shallows; sparse, strange depths. Compute a `photicDepth` per body (the depth at which light drops
below ~1% of surface) and use it to split the biome set.

`DECIDED` — benthic biomes are derived from depth, light, substrate, and water body type. They
are not painted regions.

### Benthic biome set

| Biome | Light | Substrate | Notes |
|---|---|---|---|
| `seagrass_meadow` | photic, shallow | sand/mud | anchors sediment, high biomass |
| `kelp_forest` | photic, cold | rock | 3D scatter anchored to the floor |
| `coral_reef` | photic, warm | rock/rubble | biogenic geometry (see below) |
| `rocky_reef` | photic–mesophotic | rock | crevices, caves, ledges |
| `sandy_shelf` | photic–mesophotic | sand | ripples, burrows, sparser scatter |
| `maerl_bed` | mesophotic | rubble | slow-growing, pale, fragile |
| `muddy_basin` | aphotic | mud | soft sediment, turbidites |
| `abyssal_plain` | aphotic | mud | nodules, sparse life |
| `trench_wall` | aphotic–hadal | rock | steep, high pressure |
| `hydrothermal_vent` | aphotic | rock | chimneys, chemosynthesis, heat |
| `cold_seep` | aphotic | mud/rock | clams, carbonate crusts |
| `brine_pool` | aphotic | brine | density layers, toxic, anoxic |
| `under_ice_shelf` | aphotic | rock/ice | no light, chemosynthetic |
| `freshwater_littoral` | photic, shallow | sand/mud | macrophytes, reeds |
| `freshwater_profundal` | aphotic | mud | low oxygen, sparse |

`PROPOSED` — start with `sandy_shelf`, `rocky_reef`, `muddy_basin`, `abyssal_plain`, and grow.
Freshwater and exotic-solvent sets are later.

### Depth bands are world-relative

Do **not** use Earth's absolute depths (200 m, 1000 m, 4000 m). Planets are tens of kilometres
across, so a 6 km ocean is proportionally enormous. Derive bands from `photicDepth` and the
body's total ocean depth:

```
surface     0 .. 0.05 × maxDepth
photic      0 .. photicDepth
twilight    photicDepth .. 2 × photicDepth
deep        2 × photicDepth .. 0.6 × maxDepth
abyss       0.6 × maxDepth .. maxDepth
```

These are gameplay and presentation bands, not physical constants. Tune them per body.

`DECIDED` — depth bands scale with the body, not with Earth.

---

## Pelagic environments

The water column is a **volume**. It has no surface to sculpt, so it is modelled as zones,
special volumes, and scatter — not as terrain.

### Zones

| Zone | Depth | Character | Content |
|---|---|---|---|
| `surface` | top few metres | bright, turbulent | light scatter, wave interaction |
| `photic` | to `photicDepth` | sunlit | plankton, fish, marine life |
| `twilight` | to ~2× photic | dim, blue | jellies, vertical migrators |
| `deep` | to 0.6 × maxDepth | dark | bioluminescence, sparse life, leviathans |
| `abyss` | to bottom | black, cold, high pressure | chemosynthetic life, vents, wrecks |

### Special volumes

These are local regions that override the ambient zone:

- **vent plume** — hot, mineral-rich, chemosynthetic life, hazardous
- **brine layer** — a density interface, anoxic, toxic, visually striking
- **anoxic zone** — dead water; no fish, preserved wrecks
- **plankton bloom** — dense scatter, green/turquoise, feeds everything above it
- **ice ceiling** — the underside of an ice shell; no light, anchor ice, cryofauna
- **migration corridor** — a generated path that concentrates fauna (a good place for gameplay)

`PROPOSED` — special volumes are sparse entries in the descriptor with bounds, not a field.

### The volume query

```
volumeSample(position, lod) →
    zone            // surface | photic | twilight | deep | abyss
    light           // 0..1, attenuated by depth and turbidity
    turbidity
    current         // direction + speed
    scatterDensity  // 0..1
    speciesWeights  // data-driven, per zone/volume
    hazards         // pressure, temperature, toxins, predators
```

Constraints on the query:

- **Pure and positional.** Same point, same result, any order.
- **Band-limited.** At low LOD it returns zone averages; near the player it can return local
  variation.
- **Cheap.** It is evaluated per scatter candidate, not per pixel.

`DECIDED` — the water column is a volume query, not terrain.

### Scatter: anchored vs free-floating

There are two distinct scatter passes:

| Pass | Anchor | Examples | Generated by |
|---|---|---|---|
| **Benthic scatter** | seafloor position | kelp, coral, tube worms, seagrass, rocks | surface query + benthic biome |
| **Pelagic scatter** | volume position | plankton, fish schools, jellies, leviathans | volume query + zone |

Anchored scatter must be consistent with the seafloor height (nothing floating, nothing buried).
Free-floating scatter only needs to respect the zone and avoid terrain penetration.

`DECIDED` — split anchored and free-floating scatter.

---

## Water body types

The hydrosphere's state and salinity are promoted to a small enum that changes palette,
buoyancy, life, and hazards:

| Type | Density | Palette | Life | Hazards |
|---|---|---|---|---|
| `marine` | normal, saline | blue-green | fish, coral, kelp | salt, predators |
| `freshwater` | normal, drinkable | green-brown | macrophytes, fish | none |
| `brine` | high, density-stratified | white/teal | halophiles, none above | toxic, density trap |
| `hydrocarbon` | low, cryogenic | amber/dark | exotic | flammable, toxic |
| `ammonia` | low, cryogenic | pale | exotic | extreme cold, toxic |
| `subsurface` | sealed under ice | dark | chemosynthetic | ice shell, no surface access |

This is a high-value exotic axis: a methane-lake world or an ammonia sea gives a planet a
personality for almost no code. It composes with everything else, in the spirit of
[04 — Physical Parameters](./04-physical-parameters.md).

`DECIDED` — `waterBody` is part of the hydrosphere, not a biome.

---

## Biogenic landforms (where biology shapes geometry)

Most biomes only change material and scatter. A few are **biogenic landforms** and must modify
height. These are landform suites (see [06 — Geology & Landforms](./06-geology-and-landforms.md)),
not biomes:

| Suite | Signature |
|---|---|
| `reef_framework` | coral spurs, grooves, reef crest, drop-off |
| `atoll` | ring reef with a lagoon; can reach sea level |
| `carbonate_platform` | broad shallow carbonate bank |
| `vent_chimneys` | mineral towers, black-smoker structures |
| `tube_worm_field` | dense vertical tubes around vents |
| `microbial_mounds` | stromatolite-like mounds |
| `submerged_karst` | blue holes, cenotes, drowned caves |
| `sandy_ripples` | current-aligned bedforms |
| `sediment_dunes` | deep-water bedforms |
| `turbidite_channels` | sediment flows and levees |
| `trench_scarps` | steep fault walls |
| `dropstones` | ice-rafted rocks in fine sediment |
| `ice_scours` | gouges from drifting ice |

The split stays the same as on land: the biome says *what lives there*, the landform suite says
*what shape it makes*. That is what prevents "every ocean floor is sand".

`DECIDED` — biogenic geometry is a landform suite concern, not a biome concern.

---

## The coastal interface

The shoreline is the most important underwater transition, because it is where the player enters.

- Blend across `coast → tidal flat → seagrass → shelf → open water` with the existing soft
  weights. No special transition code.
- **Tide pools** fall out of the fake tide (the sinusoidal `seaRadius` offset described in
  [05](./05-climate-and-hydrosphere.md)): the waterline moves, exposing and flooding the
  intertidal zone. This is free content.
- **Estuaries and lagoons** appear where lakes or outflow paths meet the sea; the sediment and
  turbidity fields should reflect it.
- **Shoreline rendering** needs the water shell and the terrain to agree exactly at the
  waterline (foam, wetness, shallow refraction).

`PROPOSED` — the intertidal zone uses its own variant weights, driven by tide amplitude.

---

## Rendering and visibility

Underwater rendering is mostly about **visibility**, and visibility is gameplay.

- **Fog** — colour and density from zone + turbidity. This is the dominant visual cue.
- **Light attenuation** — red disappears first; depth tints blue-green.
- **Caustics** — near the surface in the photic zone.
- **God rays** — strong in clear shallow water, absent in turbid or deep water.
- **Particulate** — "marine snow", plankton, silt; also a depth cue.
- **Bioluminescence** — deep zone and special volumes; cheap emissive particles.
- **Ice ceiling** — a distinct look: dim, refracted light, inverted topography above.
- **Underwater fog interacts with gameplay** — vehicles, lights, sonar, and predators all depend
  on visibility. Treat it as a mechanic, not a post-process.

Water surface rendering (the shell and lake caps) is defined in
[05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md).

---

## LOD and performance

Underwater is a **performance gift** if the biome distribution is designed well:

- **Fog limits view distance.** Underwater view distance is far shorter than on land, so the
  seafloor can use a much lower LOD at the same physical distance.
- **The water column is nearly free.** No geometry, only fog, particles, and instanced scatter.
- **Deep water is cheap.** Most of an ocean world is deep, dark, and low-detail.
- **Shallow reefs are the expensive part.** Budget for them; they are also the most interesting.
- **Scatter LOD** — anchored scatter culls with the chunk; free-floating scatter uses density
  fields and billboards at distance.
- **Do not render above-water detail while submerged**, and vice versa.

`PROPOSED` — define an underwater visibility distance per biome and use it as the LOD budget.

See [10 — LOD & Streaming](./10-lod-and-streaming.md).

---

## The orbit eye test

Underwater biomes pay off from space, which supports the "orbit must match surface" rule in
[00 — Vision & Pillars](./00-vision.md):

- shallow shelves are turquoise; reefs pale; abyssal water near-black,
- plankton blooms appear as green/turquoise swirls,
- ice worlds are white and fractured, with dark cracks,
- brine or hydrocarbon seas have distinctive colours,
- turbid coastal water reads as brown plumes.

Derive ocean colour from depth, benthic palette, and plankton density. A player should be able to
look at a planet from orbit and know *what kind of ocean* it has before diving.

`DECIDED` — ocean colour is derived from the descriptor, not authored separately.

---

## Descriptor additions

The `hydrosphere` block gains `waterBody`; a new `aquatic` block holds the rest:

```jsonc
"hydrosphere": {
  "waterBody": "marine",          // marine | freshwater | brine | hydrocarbon | ammonia | subsurface
  "waterInventory": 0.62,
  "oceanFraction": 0.48,
  "state": "liquid",
  "seaRadius": 24040.0,
  "salinity": 0.03,
  "lakes": [ /* ... */ ],
  "floodedCaves": ["cave:14"]
},

"aquatic": {
  "photicDepth": 140.0,           // metres; light drops below ~1%
  "maxDepth": 5400.0,
  "turbidity": 0.35,
  "currentField": { "prevailing": [0.4, 0.1, 0.9], "tidalAmplitude": 0.15 },
  "iceCover": 0.0,                // 0..1 seasonal/permanent surface ice
  "iceShellThickness": 0.0,       // > 0 for subglacial oceans
  "benthicBiomePalette": [
    { "id": "sandy_shelf", "weight": 0.4 },
    { "id": "rocky_reef",  "weight": 0.3 },
    { "id": "muddy_basin", "weight": 0.2 },
    { "id": "abyssal_plain", "weight": 0.1 }
  ],
  "pelagicZones": [
    { "id": "photic",   "depthFraction": [0.0, 0.15], "scatterDensity": 0.7 },
    { "id": "twilight", "depthFraction": [0.15, 0.4], "scatterDensity": 0.3 },
    { "id": "deep",     "depthFraction": [0.4, 0.75], "scatterDensity": 0.12 },
    { "id": "abyss",    "depthFraction": [0.75, 1.0], "scatterDensity": 0.05 }
  ],
  "ventSites": [],
  "reefSites": [],
  "brineLayers": [],
  "specialVolumes": []
}
```

Materials gain aquatic entries (sand, silt, coral, ice, basalt, organic mat) and the material
resolve step already receives depth, so wetness and depth shading slot in cleanly.

---

## Gameplay surface

Underwater is a distinct survival loop, not just scenery:

| System | Underwater behaviour |
|---|---|
| Oxygen | limited; tanks, plants, vents |
| Pressure | depth-gated; vehicles and suits |
| Temperature | thermoclines, vents |
| Visibility | fog, lights, sonar |
| Movement | swimming, buoyancy, currents |
| Hazards | predators, riptides, toxins, pressure |
| Resources | fish, kelp, minerals, vent chemistry |
| Bases | anchored habitats, airlocks, flooding risk |

`DECIDED` — underwater is a peer environment to the surface, and a first-class pillar of the
game. It is not a side activity.

### What "first-class" commits us to

- **Survival systems are required**, not optional: oxygen, pressure, temperature, visibility,
  and buoyancy.
- **Vehicles and habitats are in scope**: submarines, diving suits, underwater bases, airlocks.
- **Fauna and flora get behaviour**, not just scatter instances.
- **Underwater resources and progression** are part of the main loop, not collectibles.
- **Its own content pipeline**: biomes, landforms, materials, POIs, and set pieces.
- **Its own vertical slice**: dive from the surface to the seafloor and back, seamlessly.
- **Performance budget parity**: underwater is a different budget from the surface, not a
  smaller one.

---

## Data definitions

Biomes and zones are data, like everything else (see
[12 — Authoring & Tuning](./12-authoring-and-tuning.md)):

```toml
[biome.coral_reef]
display_name  = "Coral Reef"
kind          = "biome"
stratum       = "benthic"
palette       = "reef_tropical"
feature_scale = 1.0
scatter       = ["staghorn_coral", "brain_coral", "reef_fish"]
landform_bias = ["reef_framework", "carbonate_platform"]

[biome.coral_reef.suitability]
depth     = { range = [0, 60], falloff = 20 }
light     = { range = [0.05, 1.0], falloff = 0.05 }
substrate = { any_of = ["rock", "rubble"] }
temp      = { range = [295, 305], falloff = 3 }

[biome.abyssal_plain]
display_name  = "Abyssal Plain"
kind          = "biome"
stratum       = "benthic"
palette       = "abyss_grey"
feature_scale = 0.5
scatter       = ["nodules", "sparse_detritivores"]

[biome.abyssal_plain.suitability]
depth_fraction = { range = [0.7, 1.0], falloff = 0.05 }
light          = { range = [0.0, 0.01], falloff = 0.005 }
substrate      = { any_of = ["mud", "silt"] }

[zone.twilight]
depth_fraction = [0.15, 0.4]
light          = [0.001, 0.05]
scatter        = { plankton = 0.2, fish = 0.15, jellies = 0.4, leviathan = 0.005 }
fog            = { density = 0.04, color = "#0b2b3a" }
```

---

## Scope

Build incrementally, but treat this as a **build order, not a priority ranking**. Underwater
gameplay is in scope from the start; the MVP below is the first playable slice, not the full
ambition. The one thing not to defer is the **volume query abstraction**.

**MVP (first playable slice)**
- stratum + depth/light in the classifier,
- four benthic biomes: `sandy_shelf`, `rocky_reef`, `muddy_basin`, `abyssal_plain`,
- one pelagic scatter layer,
- underwater fog, depth colour, caustics,
- the water-volume query and a swim/dive traversal loop,
- oxygen, pressure, and visibility as real systems,
- one underwater vehicle.

That alone makes oceans feel like a place.

**Later (full pillar)**
- reefs and vents as biogenic geometry,
- brine / hydrocarbon / ammonia worlds,
- subglacial oceans and ice ceilings,
- freshwater biomes,
- bioluminescence, leviathans, special volumes,
- habitats and base building,
- fauna behaviour and ecology,
- underwater POIs, resources, and progression.

---

## Related

- [03 — The Planet Descriptor](./03-planet-descriptor.md) — the `aquatic` block.
- [05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md) — water surface, sea level, lakes.
- [06 — Geology & Landforms](./06-geology-and-landforms.md) — aquatic landform suites.
- [07 — Biomes & Ecology](./07-biomes-and-ecology.md) — classification machinery and strata.
- [09 — Surface Field Stack](./09-surface-field-stack.md) — surface vs volume queries.
- [10 — LOD & Streaming](./10-lod-and-streaming.md) — underwater visibility and LOD.
- [12 — Authoring & Tuning](./12-authoring-and-tuning.md) — biome and zone data.
- [13 - Generator Modules](./13-generator-modules.md) - the `ocean` generator.
- [18 - Collision & Edits](./18-collision-and-edits.md) - the waterline, caves, and the solid model.
