# 06 — Geology & Landforms

This document covers everything that gives a planet its *shape* below the biome layer:
tectonics, impacts, erosion, geological provinces, and the landform suites that make a barren
world interesting.

Two recurring points:

1. **Geology is what stops barren worlds from looking samey.** Ecology can be flat; geology
   almost never is. See [07 — Biomes & Ecology](./07-biomes-and-ecology.md).
2. **Some things cannot come from noise.** Rivers, craters, and mountain chains are *processes*.
   They are computed at bake time (coarsely) and stored or approximated, not faked with warped
   noise. See [09 — Surface Field Stack](./09-surface-field-stack.md).

---

## Tectonics

### Plate layout

At bake time, partition the sphere into **plates** using a spherical Voronoi diagram:

- Sample `plateCount` seed points (e.g. 5–12) on the sphere.
- Each cell is a plate; boundaries are the Voronoi edges.
- Assign each plate a **type**: continental (thicker, higher) or oceanic (thinner, lower).
- Assign each plate a **motion vector** (tangential drift).

Store the plate seeds and parameters in the descriptor. The full Voronoi is cheap to regenerate
at query time from the seeds, or can be cached at low resolution.

### Boundary types

From the plate types and relative motion, classify boundaries:

- **Convergent (continental/continental)** → mountain belts, high plateaus.
- **Convergent (oceanic/continental)** → subduction, trenches, volcanic arcs.
- **Convergent (oceanic/oceanic)** → island arcs.
- **Divergent** → rifts, mid-ocean ridges, rift valleys.
- **Transform** → strike-slip faults, linear valleys.

These produce the **macro relief**: the top-level shape that the original design called the
first noise pass. It is not noise; it is a structural field derived from the plates.

`PROPOSED` — a simplified plate model is enough. Full plate simulation is out of scope; what
matters is that mountain belts and trenches are *where boundaries are*, not random blobs.

As baked in `genesis_gen` (P1-A, `PROPOSED` numbers, all in `RockyParams`): 5–12 plates with seed
points uniform on the sphere, each continental with probability 0.4, rotating about a random Euler
pole at a speed of 0.5–1. Boundaries come from relative motion at each point, so one boundary can
change from convergent to transform to divergent along its length. Heights, metres, at full
tectonic activity: continental base +500, oceanic −1000; convergent belts 1800
(continental–continental), 1300 (mixed), 900 (oceanic–oceanic); rifts −700. The layout is
domain-warped (3 km amplitude, 15 km wavelength) and blended as a soft Voronoi so the height is
continuous at junctions; the formula is in [09](./09-surface-field-stack.md#analytic-deltas).

### Activity

`tectonicActivity` scales relief amplitude and the freshness of features. Old worlds (Ancient
profile) have eroded, rounded, subdued relief; young worlds have sharp, high relief and active
volcanism.

### Volcanism

- **Hotspots** — a few point sources independent of plate boundaries (shield volcanoes, island
  chains).
- **Arcs** — along subduction zones.
- **Flood basalts** — large provinces of low, dark, smooth terrain (great for the geological
  region map).
- **Cryovolcanism** — on cold worlds, the same machinery with ice instead of rock.

---

## Impacts and cratering

Craters are **not noise**. Use a crater accumulation model:

1. Each geological province has a **surface age** (time since last resurfacing).
2. Sample a **crater size-frequency distribution** over the province for that age.
3. Place craters with a **Poisson-disk** distribution to avoid unnatural overlaps (younger
   craters overwrite older ones; track a simple ordering).
4. Add rims, central peaks, and ejecta as landform features.

This gives the crucial visual cue that **Ancient systems look ancient**: heavily cratered,
degraded surfaces. It also gives young volcanic plains that are almost crater-free. The player
reads planetary age from the surface without being told.

`DECIDED` — cratering is age-driven and province-based. It is baked into the macro field at low
resolution and refined at query time.

`DECIDED` (from [E1](./spikes/E1-field-cost.md)) — the query-time crater term uses a **spatial
index** (cube-sphere cells listing the craters whose influence radius overlaps them). A linear scan
of only 48 craters was already half of `h0`'s cost, and an ancient world has thousands.

As baked in `genesis_gen` (P1-A, `PROPOSED` numbers):

- **Provinces are the plates**: each plate has a surface age (oceanic 0–0.5, continental 0.3–1).
- **Attempts**: 0.04 per km² of surface (290 on a 24 km body), each at a uniform position, with
  rim radius from the cumulative power law `N(> r) ∝ r^−2` on 150 m … min(3 km, radius / 6). An
  attempt is kept with probability equal to the age of the plate it lands on, and rejected if an
  earlier crater of similar size (ratio > 0.5) has its centre closer than `0.6 · (r1 + r2)`; small
  craters inside large ones are allowed. Attempt `i` always draws from `H(geologySeed, "crater",
  i)`, so the list is append-only.
- **Depth** 0.15 × rim radius; profile as in [09](./09-surface-field-stack.md#analytic-deltas).
- **Index**: 6 × 16 × 16 cells, each listing in ascending order the craters whose influence
  (2 × rim radius) can reach it. The indexed sum is bit-identical to the full sum (tested).

`PROPOSED` (P1-A) — craters are **not** in the macro pyramid: the query adds them analytically at
every LOD through the index, and the bake includes them when it evaluates the hypsometry. This
departs from the `DECIDED` line above ("baked into the macro field at low resolution"). A crater
smaller than the pyramid spacing (~375–470 m) could not be baked into it anyway, and one source
per term keeps bake and query additive (docs/09). Needs the user's confirmation.

---

## Erosion regimes

Erosion is parameterised by climate and geology, and **always** coarsely approximated:

| Regime | Driver | Signature |
|---|---|---|
| **Aeolian** | wind, dry climate, loose material | dunes, yardangs, ventifacts, dust mantling |
| **Fluvial** | water (rare, since no rivers) | broad valleys, floodplains, deltas — driven by the outflow paths from lake filling |
| **Glacial** | ice | cirques, U-shaped valleys, moraines, fjords, erratic boulders |
| **Mass wasting** | gravity, steep slopes | talus, landslides, scree slopes, cliffs |
| **Chemical weathering** | water + temperature | karst, sinkholes, solution valleys, laterite |

Each regime is a **landform suite parameter** (see below), not a simulation. The descriptor
stores a per-regime intensity; the suites use it to scale their features.

`PROPOSED` — erosion intensity is `climate-driven × geology-driven`, sampled at bake. No
erosion simulation.

---

## Geological provinces (the region map)

The **geological region map** is the primary source of regional visual variation, especially on
worlds where the ecological layer is flat. It is a low-frequency partition of the surface into
provinces, each with its own age, composition, and landform suite weights.

Province drivers:

- tectonic plates and boundaries
- impact basin age
- volcanic provinces (flood basalts, hotspot chains)
- sedimentary basins
- aeolian provinces (regional wind regimes)
- glacial provinces

Store the province map as a small set of region IDs sampled from a low-frequency field, or as
explicit low-res masks if precise shapes matter.

Example: a barren world might have provinces `cratered_highlands`, `smooth_maria`,
`rifted_plains`, `dune_sea`, `polar_cap`. That is one biome and five very different-looking
regions.

`DECIDED` — the geological region map exists independently of the biome map and is a first-class
descriptor field.

---

## Landform suites

A **landform suite** is a parameterised family of geomorphology that is applied on top of the
base terrain for a given region/biome. This is the main unit of detail variety.

Why suites rather than "biome → terrain":

- Two worlds can share a biome (say, savanna) but have completely different landforms (rolling
  plains vs eroded badlands vs karst plateaus).
- Barren worlds have no biome variety at all, but can have rich landform variety.
- Suites are data, so designers add variety without code.

### Initial catalogue

| Suite | Typical context | Signature features |
|---|---|---|
| `rolling_hills` | temperate, stable geology | rounded hills, shallow valleys |
| `river_valleys` | wet climates | broad valleys, floodplains, dry channels |
| `sea_cliffs` | coastlines | cliffs, stacks, wave-cut platforms |
| `dune_seas` | arid, windy | transverse/longitudinal dunes, slip faces |
| `yardangs` | arid, unidirectional wind | aligned ridges, streamlined forms |
| `canyons` | arid, uplifted | deep incised meanders, mesas, buttes |
| `karst` | wet + soluble rock | sinkholes, towers, caves, poljes |
| `glacial_cirques` | cold, formerly glaciated | cirques, arêtes, U-valleys, tarns |
| `fjords` | cold coastlines | deep drowned valleys |
| `columnar_basalt` | volcanic | hexagonal columns, escarpments |
| `hoodoos` | arid, soft-over-hard strata | eroded pillars |
| `inselbergs` | old, arid plains | isolated resistant hills |
| `salt_pans` | endorheic basins | flat white plains, polygons |
| `chaotic_terrain` | collapsed/icy ground | blocky jumbled topography |
| `maria` | volcanic plains on airless bodies | smooth dark lowlands |
| `cratered_highlands` | ancient airless surfaces | dense overlapping craters |
| `rift_valleys` | divergent boundaries | linear troughs, fault scarps |
| `hydrothermal_fields` | volcanically active | vents, chimneys, mineral terraces |
| `cryovolcanic_plains` | cold, active | ice flows, cryolava lobes, penitentes |
| `reef_framework` | underwater, warm | coral spurs, grooves, reef crest, drop-off |
| `atoll` | underwater, tropical | ring reef with a lagoon; can reach sea level |
| `carbonate_platform` | shallow seas | broad shallow carbonate bank |
| `vent_chimneys` | hydrothermal | mineral towers, black smokers |
| `tube_worm_field` | hydrothermal | dense vertical tubes |
| `microbial_mounds` | shallow / aphotic | stromatolite-like mounds |
| `submerged_karst` | submerged limestone | blue holes, cenotes, drowned caves |
| `sandy_ripples` | shallow, currents | current-aligned bedforms |
| `sediment_dunes` | deep, currents | large bedforms |
| `turbidite_channels` | deep, sediment | channels and levees |
| `trench_scarps` | hadal | steep fault walls |
| `dropstones` | glacial marine | ice-rafted rocks in fine sediment |
| `ice_scours` | glacial marine | gouges from drifting ice |

`PROPOSED` — start with 6–8 suites and grow. Each suite is a module with parameters such as
amplitude, wavelength, anisotropy (wind direction), and a set of feature scatter rules.

### Suite parameters

A suite is more than a name. Its descriptor entry includes sampled parameters:

```jsonc
{ "id": "dune_seas",
  "params": { "orientation": [0.3, 0.0, 1.0], "dune_height": 8.0,
              "dune_spacing": 120.0, "asymmetry": 0.7, "dust_cover": 0.4 } }
```

Parameter names are the module's own `snake_case` schema keys (see
[17](./17-biome-and-landform-format.md#module-parameter-schema)); they are not renamed at bake.

This is what makes two `dune_seas` worlds look different from each other.

The formal schema for suite entries — `module`, `feature_scale`, `context`, typed `params` — is
in [17 — Biome & Landform Format](./17-biome-and-landform-format.md).

### Aquatic and biogenic suites

The catalogue above is terrestrial. Underwater geomorphology uses the same mechanism with a
different catalogue — reefs, atolls, vent chimneys, turbidite channels, dropstones, ice scours —
and a few of them are **biogenic**: biology builds the geometry.

The biome/landform split still holds: the biome says *what lives there*, the suite says *what
shape it makes*. See [15 — Aquatic Environments](./15-aquatic-environments.md) for the aquatic
catalogue and the benthic/pelagic split.

---

## Relief and the macro heightfield

The macro relief (tectonics + impacts + large erosion) is computed at **low resolution** and
cached as the **macro pyramid** (see [10 — LOD & Streaming](./10-lod-and-streaming.md)). It is
used for:

- sea level and lake computation (see [05](./05-climate-and-hydrosphere.md)),
- far-LOD silhouettes and collision,
- cheap queries for POI placement (e.g. "find a flat spot, not underwater"),
- ensuring that detail noise never contradicts the macro shape.

`DECIDED` — macro relief is precomputed per body; detail is evaluated on demand.

---

## Relief statistics worth storing

A few summary numbers make the descriptor useful for gameplay and UI without storing the field:

- min / max / mean elevation
- hypsometric curve parameters (used for sea level)
- terrain roughness (for vehicle handling / walkability)
- slope distribution
- number and size of lakes (from [05](./05-climate-and-hydrosphere.md))

---

## Related

- [04 — Physical Parameters](./04-physical-parameters.md) — geology axis.
- [05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md) — sea level and lakes from relief.
- [07 — Biomes & Ecology](./07-biomes-and-ecology.md) — biome × landform suite.
- [09 — Surface Field Stack](./09-surface-field-stack.md) — where each geological feature is evaluated.
- [10 — LOD & Streaming](./10-lod-and-streaming.md) — macro pyramid and chunking.
- [15 — Aquatic Environments](./15-aquatic-environments.md) — underwater and biogenic landforms.
