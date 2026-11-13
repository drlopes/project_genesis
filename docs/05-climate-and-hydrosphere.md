# 05 — Climate & Hydrosphere

This document covers the energy budget, the temperature and moisture fields, sea level, lakes,
and the water query that everything else uses. It is where "the planet is a desert" becomes a
continuous field rather than an enum.

The climate model is **analytic, low-frequency, and cheap**. It is never simulated per frame, and
it never depends on L3/L4 detail. Biome borders must not move when the player changes LOD, and they
must not move with the seasons; both requirements shape the contract below.

---

## Principles

1. **Temperature is a field, not a category.** A world has a temperature at every point.
   Named bands (`temperate`, `frozen`, …) are labels derived from the field for UI and
   flavour. See [04](./04-physical-parameters.md).
2. **Multi-biome planets are not a type.** They are what happens when the climate field varies
   enough across the surface. See [07 — Biomes & Ecology](./07-biomes-and-ecology.md).
3. **Water is a surface at a level.** No fluid simulation, ever. See the water system below.
4. **Climate is cheap.** It is a handful of analytic fields evaluated per sample; it is never
   simulated per frame.
5. **Classification uses the annual mean.** Seasons and day/night are deltas for survival and
   visuals. The climate that chooses biomes is stable across LOD and time.

`DECIDED` — climate is a pure, band-limited field derived from the descriptor; it is never stored
per sample and never simulated.

---

## The climate sample

L1 of the surface stack returns a `ClimateSample` alongside the height and material:

```text
ClimateSample {
  temperature      : float       // K, annual mean at the sample
  diurnalRange     : float       // K, peak-to-peak
  seasonalRange    : float       // K, peak-to-peak
  moisture         : float       // 0..1, annual mean
  relativeHumidity : float       // 0..1, derived from moisture and temperature
  wind             : vec3        // m/s, tangent to the surface
  snowLine         : float | null // metres above sea level, null if none
  climateZone      : string      // derived label, e.g. "temperate"
}
```

This is the structure [09 — Surface Field Stack](./09-surface-field-stack.md) carries as
`SurfaceSample.climate`. `temperature` and `moisture` are the **classification** values; the
ranges are for survival, rendering, and audio.

`temperatureAt(p, time)` is a separate query that adds the seasonal and diurnal deltas:

```text
T(p, t) = climate.temperature
        + seasonalDelta(p, t)
        + diurnalDelta(p, t)
```

`temperatureAt` never feeds classification. It is what the player feels and what the snow/ice
material uses.

---

## Energy budget

Bake computes the body's energy inputs. The exact formula is less important than the fact that it
is **continuous and parameterised**, so the generator can produce a smooth range from infernal to
cryogenic.

| Input | Source | Notes |
|---|---|---|
| **Insolation** | stellar luminosity / orbital distance² | For binaries, sum the flux from each star at the orbit. |
| **Albedo** | surface + cloud estimate | Feedback from hydrosphere/ice; iterate once or twice at bake. |
| **Greenhouse** | atmosphere composition × pressure | A Kelvin offset added to the equilibrium temperature. |
| **Internal heat** | mass × age × composition | Drives volcanism and can warm a world far from its star. |
| **Tidal heat** | moon/star proximity × eccentricity | The physically motivated way to make a warm outer-system moon. |

```text
S_eff  = Σ_stars L_i / (4π d_i²)                     // W/m², game-scaled
T_eq   = 288 K · (S_eff / S_earth)^(1/4) · ((1 − A) / 0.7)^(1/4)
T_base = T_eq + greenhouse + internalWarming + tidalWarming
```

- `288 K` is Earth's mean **surface** temperature and `0.7` is Earth's `1 − albedo`. `T_eq` is an
  Earth-referenced effective temperature, not a pre-greenhouse blackbody value.
- `greenhouse` is the **difference from Earth's baseline greenhouse** (`energy.greenhouse`), which
  is why the reference already includes it. `0` is Earth-like, negative removes the baseline
  (order `−33 K` for no atmosphere), and large positive values are a runaway. This avoids double
  counting.
- `internalWarming = k_int · internalHeat`, with `k_int` a small tuning constant (order 10 K at
  full internal heat). Internal heat is primarily a geology driver; this term only prevents
  "warm volcanism + frozen surface" contradictions.
- `tidalWarming = k_tidal · tidalHeat`, with `k_tidal` order 20 K at full tidal heat. This is how
  a moon far outside the habitable zone stays warm.
- **Albedo feedback:** if the initial pass puts a large fraction of the surface below freezing,
  raise `A` toward the ice albedo and recompute once. Two iterations are enough; this is a bake
  loop, not a simulation.

`DECIDED` — insolation + albedo + greenhouse + internal + tidal heat. No full radiative transfer.

---

## The temperature field

The **annual mean** surface temperature is:

```text
T_mean(p) = T_base
          + latitudeTerm(lat)
          + altitudeLapse(altitude)
          + orographicCooling(p)
          + coastalModeration(p)
```

`altitude` here is metres above sea level, not radius above the reference sphere; see
[16 — Data Contracts](./16-data-contracts.md#vertical-quantities). The field uses the **pre-L3**
height (`h0`), so landform detail never moves a climate boundary. See
[09 — Surface Field Stack](./09-surface-field-stack.md#l1--climate--hydrology).

### Latitude

```text
lat  = asin(clamp(y / |p|, −1, 1))
s    = sin(lat)
T_lat = 3 · A_lat · (1/3 − s²)
```

- At the equator `T_lat = +A_lat`; at the poles `T_lat = −2·A_lat`; the area-weighted mean is
  zero, so `T_base` remains the global mean.
- `A_lat = A_lat0 · (1 − transport)`, where `transport ∈ [0, 1]` is atmospheric heat transport.
- `transport = clamp(clamp(surfacePressure / 1e5, 0, 1) · transportScale, 0, 1)`. A thick atmosphere
  evens out the equator-to-pole difference; a thin or absent atmosphere does not.
- `A_lat0` is a tuning constant per climate preset (order 25–35 K).

### Seasons

```text
seasonPhase = 2π · (gameTime / yearLength) + phaseOffset
T_season(p, t) = A_season · s · sin(seasonPhase)
A_season = A_season0 · sin(obliquity) · (1 − 0.5 · transport)
```

- The seasonal term is zero at the equator and strongest at the poles, which is why obliquity
  produces polar seasons rather than a global temperature swing.
- `yearLength` comes from the orbit; `phaseOffset` is baked from the initial mean anomaly.
- `A_season0` is a tuning constant (order 20 K).
- `seasonalRange = 2 · A_season`, reported in `ClimateSample` as a peak-to-peak value.
- Seasonal delta is **not** part of `T_mean`. It is added by `temperatureAt`.

### Altitude lapse

```text
lapseRate = 0.0065 K/m · (gravity / g_earth) · (max(pressure, 1e3) / 1e5)^0.2
T_lapse   = −lapseRate · min(max(0, altitude), tropopauseAltitude)
```

- `0.0065 K/m` is the Earth-like reference (6.5 K/km).
- Gravity and pressure scale the rate; a thin atmosphere on a low-gravity body has a weaker,
  noisier lapse.
- Above the tropopause (order 10–15 km), the lapse stops. The game rarely reaches it, but the
  clamp prevents absurd temperatures on tall peaks.
- Below sea level (benthic samples), no lapse is applied; water temperature is handled by the
  aquatic layer.

### Orographic cooling and coastal moderation

```text
T_oro   = −A_oro · clamp((h0(p) − h0(p − L_wind · w)) / (L_wind · slope_ref), 0, 1)
T_coast = +A_coast · exp(−d_water / L_coast)
```

- `T_oro` is a small extra cooling on the windward slope (order 1–3 K). Most of the orographic
  temperature effect is already in the lapse term; this term keeps exposed ridges slightly cooler.
  `w` is the prevailing wind direction from the wind field below.
- `d_water` is the distance to the nearest ocean or lake edge, from the baked **water-distance
  field** (see below).
- Coastal moderation mainly reduces the **diurnal and seasonal range**, not the mean. The mean
  offset is small and positive (maritime climates are milder).

### No double counting

Internal and tidal heat are already in `T_base` (see the energy budget). Do not add them again in
the temperature field; if a preset needs a stronger effect on a specific body, increase the
descriptor's `internalHeat`/`tidalHeat`, not a second term.

### Diurnal range

```text
D0 = 12 K                                  // tuning constant
diurnalRange = D0
             · (1 − 0.6 · coastalFactor)
             · (1 − 0.7 · transport)
             · insolationFactor
coastalFactor = exp(−d_water / L_coast)
```

- A thick atmosphere and nearby water both flatten the day/night swing.
- `insolationFactor = clamp(insolation, 0, 1.5)`.
- `localTime = frac(longitude / 2π + gameTime / dayLength) · dayLength`; the diurnal delta is
  `0.5 · diurnalRange · cos(2π · localTime / dayLength)`. It is added by `temperatureAt`, never by
  `T_mean`.
- On a tidally locked body, `diurnalRange = 0`: the substellar model already encodes the permanent
  day/night difference.

### Tidally locked worlds

A tidally locked body has a fixed substellar point. The descriptor stores:

```jsonc
"tidalLockClimate": {
  "substellarDir": [1.0, 0.0, 0.0],   // body-local unit vector
  "dayContrast": 0.35,                // fraction of T_base added at the substellar point
  "nightContrast": 0.55,              // fraction subtracted at the antipode
  "transport": 0.4,                   // atmospheric smoothing, 0..1
  "coldTrap": true                    // nightside can freeze out volatiles
}
```

```text
cosψ = dot(dir(p), substellarDir)
T = T_base
  + altitudeLapse(altitude)
  + dayContrast   · T_base · max(cosψ, 0)^0.25
  − nightContrast · T_base · max(−cosψ, 0)^0.25
```

- The field is continuous at the terminator (`cosψ = 0`).
- `transport` reduces both contrasts; a thick atmosphere shrinks the hot spot and warms the
  nightside.
- `altitudeLapse` still applies: a tidally locked mountain is colder than the same latitude at sea
  level.
- If `coldTrap` is true and the nightside temperature falls below the atmosphere's condensation
  point, the bake may freeze out part of the atmosphere. That is a descriptor decision, not a
  runtime simulation.
- The substellar direction is derived from the orbit and rotation at bake; it is not recomputed
  per sample.
- Tidally locked bodies do not use the latitude term or the diurnal range; the substellar model
  replaces both. Seasons still apply if obliquity is non-zero.

### Thin or no atmosphere

No atmosphere means:

- `transport ≈ 0`, so the latitude contrast is maximal;
- `diurnalRange` is large (no heat storage);
- `greenhouse` is the negative of Earth's baseline (order `−33 K`), not zero;
- the sky is vacuum-black and the surface materials should reflect extreme day/night weathering.

These are consequences of the same equations, not a separate climate type.

### Water temperature at depth

Benthic and pelagic samples use the temperature at depth, not the surface value:

```text
T_water(depth) = T_surface + (T_deep − T_surface) · (1 − exp(−depth / thermoclineDepth))
```

- `T_surface` is `T_mean` at the water surface.
- `T_deep` is the deep-water temperature from the aquatic block; for a frozen world it is clamped
  at the freezing point.
- `thermoclineDepth` is a descriptor value (order 100–1000 m).
- This is an analytic profile, not a simulation. It feeds the benthic `temp` axis and survival.

`DECIDED` — water temperature is an analytic two-layer profile, not a fluid simulation.

---

## Moisture field

Moisture drives biome selection (see [07](./07-biomes-and-ecology.md)) and the `moisture` axis in
[17](./17-biome-and-landform-format.md). It is an annual mean in `[0, 1]`.

```text
m = clamp(
      m_base
    + m_latitude
    + m_water
    + m_orographic
    − m_shadow
    − m_cold,
    0, 1
)
```

### Base and latitude belts

```text
latDeg = degrees(lat)
m_base = 0.15 + 0.25 · noise3(p / S_m + offset_m)       // low-frequency
m_latitude =
    A_itcz  · exp(−(latDeg / σ_itcz)²)
  − A_sub   · exp(−((|latDeg| − 30) / σ_sub)²)
  + A_mid   · exp(−((|latDeg| − 60) / σ_mid)²)
```

- The equator is wet (intertropical convergence), the subtropics are dry (subtropical highs), and
  the mid-latitudes are wet again (storm tracks).
- `S_m` is a large wavelength (order 20–50 km); the noise is band-limited like every other field.
- `A_itcz`, `A_sub`, `A_mid`, and the sigmas are tuning constants in the climate preset.

### Distance to water

```text
m_water = A_water · exp(−d_water / L_water)
```

`d_water` is the geodesic distance to the nearest ocean or lake edge, in metres. It is **not**
computed per sample by a flood fill; it is a baked **water-distance field** on the macro grid,
sampled bilinearly. The field is a cache (see [03](./03-planet-descriptor.md#core-versus-caches))
and is rebuilt from the descriptor if missing.

- `d_water = 0` inside a water body.
- `L_water` is the moisture falloff length (order 5–20 km). A small world feels its coasts
  everywhere; a large one has dry interiors.
- The field also drives coastal moderation and the `distance_to_water` biome axis.

### Orographic lift and rain shadow

This is the single cheapest way to make biomes look designed rather than noisy. Sample the terrain
a short distance upwind and compare:

```text
w        = prevailingWindDir(p)
h_here   = h0(p)
h_upwind = h0(p − L_wind · w)
s_up     = (h_here − h_upwind) / L_wind

m_orographic = A_oro    · clamp( s_up / slope_ref, 0, 1)
m_shadow     = A_shadow · clamp(−s_up / slope_ref, 0, 1)
```

- If the terrain rises into the wind (`s_up > 0`), the air lifts, cools, and rains: wet windward
  slope.
- If the terrain falls away downwind (`s_up < 0`), the air has already lost its moisture: dry
  leeward slope.
- `L_wind` is the upwind sampling distance (order 5–20 km); `slope_ref` is a reference slope (order
  0.05). Both are tuning constants.
- The term is intentionally local. A full rain-shadow integral would be a simulation; this captures
  the visual asymmetry at a fraction of the cost.
- The upwind sample uses the macro field at the same LOD band, so it never introduces detail above
  the cutoff.

### Cold air

```text
coldScale = clamp((T_mean − 240 K) / (273 K − 240 K), 0.25, 1)
m = m · coldScale
```

Cold air holds little water, so polar and high-altitude deserts emerge naturally. The floor of
`0.25` prevents a mathematically dry but visually snowy world.

### Relative humidity

```text
relativeHumidity = clamp(m / saturation(T_mean), 0, 1)
```

`saturation` is a monotonic curve (order: `exp((T − 273) / 30)` clamped). It is used by survival
and weather visuals; biome classification uses `moisture`, not relative humidity.

---

## Wind field

Wind is a coarse direction field, not a simulation. It is used for moisture, dune orientation,
scatter, and visual effects.

```text
up    = normalize(p)
north = normalize(Y − up · dot(up, Y))
east  = cross(north, up)
lat   = asin(clamp(up.y, −1, 1))
band  = tanh(sin(6 · lat) / 0.4)
windDir = normalize(east · (band + 0.15) + north · meridionalNoise(p))
```

- `sin(6 · lat)` gives alternating zonal bands: trades, westerlies, polar easterlies. The sign is a
  convention; what matters is that the bands alternate and are continuous.
- `meridionalNoise` is a small low-frequency component (order 0.2) that keeps the flow from looking
  like a perfect grid.
- `windSpeed` comes from the descriptor's climate parameters and scales with pressure; only the
  direction matters for moisture.
- `terrainDeflection(p)` rotates the wind slightly around steep slopes. It must be small; the wind
  is a climate input, not a local flow solver.
- Near the poles, use a stable fallback tangent (e.g. `+X` projected onto the tangent plane) so the
  basis never divides by zero. Wind direction at the exact pole is arbitrary; pick one and keep it
  continuous.
- On a tidally locked body, replace the band function with flow away from the substellar point:

```text
windDir = normalize(substellarDir − up · dot(substellarDir, up))
```

with a noise deflection near the terminator. This produces the characteristic dayside-to-nightside
circulation without a general circulation model.

`PROPOSED` — wind bands are a latitude function rotated by a per-planet offset. The exact band
function is a tuning constant.

---

## Time: mean versus instantaneous

The climate contract separates three time scales:

| Scale | Field | Used by |
|---|---|---|
| **Annual mean** | `T_mean`, `m` | biome classification, materials, descriptor summaries |
| **Seasonal** | `seasonalDelta(p, t)` | survival, snow cover, visuals, migration |
| **Diurnal** | `diurnalDelta(p, t)` | survival, day/night visuals, nocturnal fauna |

Only the annual mean is allowed to affect classification. If a season could change a biome, the
same world would have different biomes in different months, and every LOD boundary would become a
content bug.

`DECIDED` — classification uses the annual mean at a reference season; time deltas are separate.

---

## Hydrosphere

### Water inventory and state

Bake samples:

- **waterInventory** — how much water the body has (0..1).
- **state** — `liquid`, `ice`, `mixed`, `vapour`, `none`, derived from temperature and pressure.
- **oceanFraction** — the target fraction of the surface covered by liquid (or ice) water.

Water inventory is decided **before** relief. Sea level is decided **after** relief, from the
hypsometric curve. See [03 — The Planet Descriptor](./03-planet-descriptor.md#bake-ordering-within-a-body).

### Sea level from hypsometry

Do not treat sea level as a raw dial. Instead:

1. Generate the **hypsometric curve** — the distribution of surface elevations — from the
   geology layer (see [06](./06-geology-and-landforms.md)). Its shape controls whether the world
   is continental (bimodal: low plains + high plateaus) or monotonous.
2. Solve for the **sea radius** that yields `oceanFraction` of the surface below it.

This single mechanism gives you:

- **Continental worlds** — bimodal hypsometry, large landmasses, distinct shelves.
- **Archipelago worlds** — unimodal, shallow, lots of coastline.
- **Water worlds** — ocean fraction near 1, little or no land.
- **Dry worlds** — ocean fraction near 0, sea level irrelevant.
- **Coastline variety** — falling out of the elevation distribution and the noise, not authored.

`DECIDED` — sea level is derived from ocean fraction + hypsometry, not dialled directly.

The solve as implemented in `genesis_gen` (P1-A, `PROPOSED` numbers):

- Evaluate `h0` at the centres of a 6 × 64 × 64 cube-sphere cell grid, band-limited to the cell
  size, each cell weighted by its solid angle (gnomonic cells are not equal-area:
  `dΩ ≈ Δu Δv / (1 + u² + v²)^{3/2}`).
- Sort by height (stable, so ties keep grid order) and walk the cumulative area to the target;
  the sea surface is midway between that cell and the next one up. `seaRadius = radius + level`.
- The descriptor stores the curve as 21 area quantiles (0 %, 5 %, …, 100 %) and the mean.
- Accuracy: on an independent 96 × 96 grid, the area below the sea matches `oceanFraction` within
  0.001 for the test seeds.
- `oceanFraction` is drawn per body as `clamp(0.55 + 0.25 · N(0, 1), 0, 0.95)` (docs/02's default
  profile `water_fraction`); below 0.02 the body is dry (`state = none`, `seaRadius = null`).
  `waterInventory` and the derived `state` wait for the energy and atmosphere phases; until then
  `state` is `liquid` when there is a sea.

### Ice

Water can be ice rather than liquid. Ice sheets, polar caps, and frozen oceans are just the same
water surface rendered with an ice material, gated by the temperature field. A frozen ocean and
a liquid ocean share all the geometry logic.

`PROPOSED` — ice thickness/visual variation is a shader/material concern, not geometry.

### Water body types

Water is not the only possibility. The hydrosphere stores a `waterBody` type alongside its state
and salinity:

| Type | Density | Palette | Notes |
|---|---|---|---|
| `marine` | normal, saline | blue-green | the default; supports the full aquatic biome set |
| `freshwater` | normal, drinkable | green-brown | lakes, wetlands, ponds; freshwater biomes |
| `brine` | high, density-stratified | white/teal | toxic, anoxic, density layers |
| `hydrocarbon` | low, cryogenic | amber/dark | methane/ethane seas; flammable, toxic |
| `ammonia` | low, cryogenic | pale | extreme cold; exotic solvent |
| `subsurface` | sealed under ice | dark | no surface access; chemosynthesis |

The type changes **palette, buoyancy, scatter, and hazards**. It does not change the geometry of
the water surface: the shell, sea level, and lake machinery are identical. This is a cheap exotic
axis — a methane-lake or ammonia-sea world gets a distinct personality from the same hydrosphere
code.

`DECIDED` — `waterBody` is part of the hydrosphere, not a biome.

See [15 — Aquatic Environments](./15-aquatic-environments.md) for the biological and gameplay
consequences.

---

## Lakes

### Bake-time basin filling

Run a **priority-flood / depression-filling** algorithm on the coarse heightfield:

- Input: the macro height grid (a few hundred thousand samples at most).
- Output: every closed basin, its **spill elevation**, and its **outflow path**.
- Store each basin as `{ id, surfaceRadius, bounds (spherical cap), depthProfile }`.

This is `O(n log n)` on a small grid, runs once per body, and is invisible to the frame budget.

### What you get for free

- **Lakes** — every closed basin below its spill elevation becomes a lake.
- **Endorheic basins** — basins with no outflow (salt flats, closed seas).
- **Dry valleys** — the outflow paths, if you do not want visible rivers. They read as
  dried river beds and are a great landform feature.
- **Wetlands** — shallow basins near sea level.
- **Lake chains** — basins spilling into each other along a path.

### Rivers

Rivers are **explicitly out of scope**. They require flow accumulation, dynamic water volume,
and either fluid simulation or expensive network meshing; in a voxel world they also interact
badly with terrain edits. The outflow paths from depression filling are the closest cheap
approximation and should be used as dry valleys if they are used at all.

`DECIDED` — no rivers, no fluid simulation. Lakes and a sea level only.

---

## The water query

Everything downstream uses a single O(1) query. It is expressed in the same height convention as
[09](./09-surface-field-stack.md#height-and-coordinate-convention) and
[18](./18-collision-and-edits.md#collision):

```text
seaSurface      = (seaRadius ? seaRadius − bulk.radius : −∞)
waterSurface(p) = max(seaSurface, highest lake surface containing p)

if p is inside a flooded cave volume:
    waterSurface(p) = max(waterSurface(p), cave.waterSurface)

depth(p)        = max(0, waterSurface(p) − height(p))
isUnderwater(p) = depth(p) > 0
shoreline(p)    = height(p) − waterSurface(p)      // signed; + is dry
```

- `height` is elevation above `bulk.radius`; `waterSurface` is the same quantity for water.
- `seaRadius` and `lake.surfaceRadius` are stored as absolute radii in the descriptor and converted
  once per body.
- The lake lookup is a broad-phase (bounding cap / spatial hash) followed by a few point-in-cap
  tests. With dozens of lakes per planet, this is trivial.
- A **flooded cave** is a tagged set piece ([15](./15-aquatic-environments.md)); its local water
  surface participates in the same query. The global ocean shell does not fill caves.

`DECIDED` — one water surface scalar for sea, lakes, and flooded caves; no separate water fields.

---

## Rendering water

`DECIDED` — water is rendered as a **surface**, not as voxels:

- **Ocean** — a spherical shell at `seaRadius`, occluded by terrain. At distance it is a single
  mesh; near the player it is tessellated with shader-driven wave displacement.
- **Lakes** — small spherical-cap patches at their `surfaceRadius`.
- **No water voxels.** If the player digs below sea level near the coast, the ocean shell is
  still there and the hole appears flooded. This is correct, free, and requires no fill logic.
- **Caves below sea level stay dry** unless they are tagged as `floodedCaves`, because the shell is
  above them. A flooded cave gets a local water volume.
- **Waves and tides are visual only.** They never enter `waterSurface`, `depth`, or buoyancy;
  see [18](./18-collision-and-edits.md#water-is-not-solid).

Rendering budget: one ocean shell per body (LOD'd), plus N lake caps. Do **not** voxelise water.

### Water appearance

Depth drives colour (shallow turquoise → deep blue/black), plus:

- material (water vs ice vs brine vs liquid hydrocarbon on exotic worlds),
- turbidity/sediment near coasts and river mouths (from the dry-valley paths),
- foam near shorelines,
- subsurface scattering for shallow water,
- caustics underwater,
- fog and light attenuation underwater (gameplay-critical).

---

## Tides

Real tides require the moon's gravity and the planet's rotation. Do not simulate them.

`PROPOSED` — a slow sinusoidal offset applied to the **rendered** water surface, amplitude and
period derived from the dominant moon. It is purely visual (a few centimetres/metres), costs
nothing, and sells the presence of a large moon. Skip it on tidally locked or moonless worlds.
Like waves, it must not enter `waterSurface`.

---

## Climate outputs consumed elsewhere

The climate field feeds:

- **Biome classification** — temperature × moisture × substrate → biome weights
  ([07](./07-biomes-and-ecology.md)).
- **Snow/ice line** — elevation and latitude; shown as material, not geometry. `snowLine` is an
  explicit output so the material resolve and the inspector agree.
- **Weather visuals** — clouds, precipitation, dust storms. Purely visual; see
  [09](./09-surface-field-stack.md) for where they sit in the stack.
- **Space-view appearance** — the colour and cloud cover seen from orbit must match the surface
  climate ([00 — Vision & Pillars](./00-vision.md)).
- **Survival mechanics** — temperature, breathability, water availability, radiation. These use
  `temperatureAt`, not the annual mean.
- **Aquatic environments** — water body type, depth, turbidity, and light feed the benthic and
  pelagic systems ([15](./15-aquatic-environments.md)).

---

## Invariants

These must hold for every climate sample:

1. **Continuity.** `T_mean` and `moisture` are continuous across the surface, including the poles
   and the tidally locked terminator.
2. **Range.** `moisture ∈ [0, 1]` and `relativeHumidity ∈ [0, 1]`; temperature is finite and above
   absolute zero.
3. **LOD independence.** Climate is evaluated from `h0` and low-frequency fields; it is identical
   at every LOD where it is evaluated.
4. **Time independence of classification.** Seasons and day/night never change `biomeWeights`.
5. **Lapse consistency.** `snowLine` is the altitude at which `T_mean = 273.15 K`, computed with
   the same lapse rate used by the field.
6. **Water agreement.** `waterSurface`, `depth`, and `shoreline` use the same `height` scalar as the
   surface query and collision.
7. **Flooded caves are local.** They add a local water surface; they never raise the global sea
   level.
8. **No feedback into generation.** Climate is an input to classification, never read back from
   biomes or materials.
9. **Determinism.** The same descriptor, position, and game time produce the same sample on every
   machine (see [11 — Determinism & Sampling](./11-determinism-and-sampling.md)).

A violation is a bug in the climate model or in content, not a tuning problem.

---

## Worked example

A coastal temperate sample at LOD 3. `bulk.radius = 24 000 m`, `seaRadius = 24 040 m`, so sea level
is `height = +40 m`. The pre-L3 height is `+140 m`, so `altitude = +100 m`. Latitude is `40°`.

| Term | Value |
|---|---|
| `T_base` | `290.0 K` |
| `latitudeTerm(40°)` | `−4.8 K` (`A_lat = 20 K`) |
| `altitudeLapse(100 m)` | `−0.65 K` |
| `orographicCooling` | `+0.5 K` |
| `coastalModeration` | `0 K` (already mild) |
| **`T_mean`** | **`285.05 K`** |
| `diurnalRange` | `8 K` (maritime, thick atmosphere) |
| `seasonalRange` | `12 K` |
| `snowLine` | `100 + (285.05 − 273.15) / 0.0065 ≈ 1 930 m` above sea level |
| `m_base` | `0.20` |
| `m_latitude(40°)` | `+0.10` |
| `m_water` | `+0.30` (coastal) |
| `m_orographic` | `+0.10` (windward) |
| `m_shadow` | `0` |
| **`moisture`** | **`0.70`** |
| `wind` | `(4, 0, 1) m/s` (westerlies) |
| `climateZone` | `temperate` |

The snow line at ~1.9 km means the same body can have a temperate coast, a forest belt, and
snow-capped peaks within a few kilometres. That vertical gradient is one of the cheapest sources of
visual variety on a small planet.

At LOD 2 the same sample uses the same climate values; only the terrain detail differs. At a
different season, `temperatureAt` adds `seasonalDelta`, but `biomeWeights` and `moisture` do not
change.

---

## Related

- [03 — The Planet Descriptor](./03-planet-descriptor.md) — bake order and the climate block.
- [04 — Physical Parameters](./04-physical-parameters.md) — energy/atmosphere/hydrosphere axes.
- [06 — Geology & Landforms](./06-geology-and-landforms.md) — the heightfield that sea level and
  lakes are computed against.
- [07 — Biomes & Ecology](./07-biomes-and-ecology.md) — climate → biomes.
- [09 — Surface Field Stack](./09-surface-field-stack.md) — where climate is evaluated at LOD.
- [10 — LOD & Streaming](./10-lod-and-streaming.md) — how the ocean shell is LOD'd.
- [11 — Determinism & Sampling](./11-determinism-and-sampling.md) — purity and reproducibility.
- [15 — Aquatic Environments](./15-aquatic-environments.md) — everything below the waterline.
- [16 — Data Contracts](./16-data-contracts.md) — altitude versus height.
- [18 — Collision & Edits](./18-collision-and-edits.md) — the waterline and the solid model.
