# 09 — Surface Field Stack

This document defines how a surface sample is produced: the **data a sample carries**, the
**order of evaluation**, the **kind of operation** at each layer, and the **band-limiting rules**
that keep every LOD cheap and consistent.

It is the reframing of the original "five terrain passes". The passes were the right instinct,
but they mixed three different kinds of operation (analytic fields, process simulations, and
detail noise) and treated biomes as something painted. This document separates those concerns and
turns the layer model into an implementation contract.

---

## The three kinds of operation

Every contribution to the surface is one of these. Mixing them up is the most common source of
bugs and of "the terrain looks fake".

| Kind | What it is | Examples | How it is evaluated |
|---|---|---|---|
| **Analytic field** | a closed-form or low-frequency function defined everywhere | base relief, temperature, moisture, wind, macro elevation | cheap at any point; band-limited by construction |
| **Process simulation** | the result of an algorithm that cannot be sampled directly | plate boundaries, crater accumulation, basin filling | computed once at bake on a coarse grid; sampled or approximated afterwards |
| **Detail noise** | band-limited high-frequency variation | rocks, dunes, pillars, grass-scale bumps | evaluated on demand, gated by biome/landform/LOD |

**Rule:** if something has structure that depends on *history* or *accumulation* — rivers,
craters, mountain belts, erosion — it is a process simulation, not noise. Faking it with warped
noise produces patterns that players read as wrong even if they cannot say why.

---

## The two query types

The stack below describes the **surface query**: it returns height, material, and classification
weights for a point on ground. That covers land and the seafloor (benthic).

The **water column** has no height field, so it uses a separate **volume query**. Keeping these as
two query types, rather than faking the water column as terrain, is what keeps the ocean cheap and
the seafloor consistent.

### The sample contract

```text
SurfaceSample {
  position       : vec3        // body-local metres, origin at body centre
  normal         : vec3        // unit, outward
  height         : float       // elevation above bulk.radius (reference sphere)
  altitude       : float       // metres above sea level; negative underwater
  climate        : ClimateSample
  material       : MaterialSample
  biomeWeights   : map<contentID, float>   // leaves (biomes + variants), sums to 1
  biomeFamilies  : map<contentID, float>   // top-level families, sums to 1; for UI/gameplay
  suiteWeights   : map<contentID, float>   // normalised, may be empty
  water          : WaterSample | null
  flags          : { submerged, shoreline, ice, edited }
}

MaterialSample {
  materialID     : contentID
  blend          : map<contentID, float>   // normalised
  albedo         : color
  roughness      : float
  wetness        : float
}

WaterSample {
  body           : contentID | null        // marine, freshwater, brine, ...
  surface        : float                   // metres above bulk.radius
  depth          : float                   // 0 when above the surface
  shoreline      : float                   // height − surface, metres
}

VolumeSample {
  zone           : contentID
  depth          : float
  light          : float
  turbidity      : float
  current        : vec3
  scatterDensity : map<contentID, float>
  hazards        : map<string, float>
}
```

`SurfaceSample` deliberately does **not** contain scatter. Scatter is generated per chunk from the
sample's weights and the rules in [17 — Biome & Landform Format](./17-biome-and-landform-format.md);
a single sample cannot know what the rest of the chunk contains.

`ClimateSample` (annual-mean temperature, moisture, wind, ranges, snow line) is defined in
[05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md#the-climate-sample). It is carried on
the sample because survival, materials, and the inspector all need it, but it is **not** part of
the classification inputs: classification reads the same annual-mean fields.

### Stratum selection

Which query applies is determined by the sample's **stratum** (`aerial`, `benthic`, `pelagic`,
`subglacial`, `subterranean`). Benthic samples use the surface query with a water-aware classifier
(depth and light instead of altitude and insolation). Pelagic samples use the volume query. See
[15 — Aquatic Environments](./15-aquatic-environments.md).

For a benthic sample, `SurfaceSample.climate` keeps the **surface** climate (it is the same
`ClimateSample` the water surface sees), but classification's `temp` axis is the **water
temperature at depth**:

```text
tempAxis = T_water(depth) = T_surface + (T_deep − T_surface) · (1 − exp(−depth / thermoclineDepth))
```

`T_surface = climate.temperature`, and `T_deep`/`thermoclineDepth` come from the body's aquatic
block. The classifier never uses `climate.temperature` directly for a benthic sample. See
[05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md#water-temperature-at-depth).

---

## Height and coordinate convention

Body-local space is right-handed, with **+Y = north pole**, **longitude 0 at +X**, and **+Z = 90°
east** at the equator. These are the conventions fixed in
[16 — Data Contracts](./16-data-contracts.md#coordinate-convention).

Height is defined as **elevation above the reference sphere**, not above sea level:

```
height(p)      = |p| − bulk.radius
waterSurface   = seaRadius − bulk.radius          // ocean
lakeSurface    = lake.surfaceRadius − bulk.radius // lakes
altitude(p)    = height − (seaRadius − bulk.radius) // above global sea level
depth          = max(0, waterSurface − height)    // local water body
shoreline      = height − waterSurface            // local signed distance
```

`height` is the geometry/collision quantity. `altitude` is the climate and biome quantity; it is
the `altitude` axis in [17](./17-biome-and-landform-format.md#the-axis-registry), and it is
negative underwater. `shoreline` equals `altitude` for the ocean and differs for a lake.

Why this convention:

- **Body size and terrain stay separate.** A moon with a 4 km radius and a planet with a 24 km
  radius use the same elevation numbers; only the reference sphere changes.
- **Water and collision share one scalar.** Sea level, lake surfaces, shoreline shaping, and
  collision all compare against `height`, so they cannot disagree.
- **LOD morphing is stable.** The macro silhouette is a property of `height`, not of absolute
  radius, so morphing between LODs does not move the planet.

`seaRadius` and `lakes[].surfaceRadius` are stored as absolute radii in the descriptor (see
[03](./03-planet-descriptor.md)); the query converts them once per body.

---

## Layer model

Layers are **frequency bands**, not code passes. Each layer is evaluated only as far as the
requested LOD needs it. The feature-scale column is a starting point, not a hard rule; content
declares its own `feature_scale` (see [16](./16-data-contracts.md#lod-and-feature-metadata)).

| Layer | Purpose | Typical feature scale | Evaluation |
|---|---|---|---|
| **L0** | macro structure: plates, basins, large erosion | 10–100 km | baked → macro pyramid + analytic deltas |
| **L1** | climate and hydrology: temperature, moisture, wind, water surface | 5–50 km | analytic |
| **L2** | classification: biome, suite, and material weights | 1–10 km | analytic, soft weights |
| **L3** | landform detail: suites, shoreline, material resolve | 1–500 m | modules + band-limited noise |
| **L4** | micro detail: rocks, cracks, grass, cleanup | below 1 m | near-field only |

---

## L0 — Macro structure

The planet's large-scale shape. It is **not** noise; it is derived from:

- tectonic plate layout and boundary types (see [06](./06-geology-and-landforms.md)),
- impact basins and large craters (age-driven),
- large-scale erosion and sediment fill,
- the hypsometric character (bimodal continental vs unimodal oceanic).

### The macro pyramid

The smooth part of L0 is baked on a cube-sphere grid and cached as the **macro pyramid**. The query
samples it with bilinear interpolation at LOD 0–2 and bicubic interpolation at LOD 3–4. The
pyramid is the cached representation used for the far silhouette and cheap collision; the
definition remains the descriptor plus the generator (see
[10](./10-lod-and-streaming.md#the-macro-pyramid)). Higher LODs add detail below its Nyquist limit
but never change its shape.

```text
macroHeight(p, lod) = pyramid.sample(p, interpolation(lod))
```

The pyramid holds `macroHeight` only: the plate base elevations (soft-blended) plus the macro
noise, band-limited to the node spacing. Plate boundaries and craters are not in it; they are the
analytic deltas below. `PROPOSED` (P1-A): 6 × 129 × 129 corner-aligned nodes (390 KB; node
spacing at a face centre 375 m on a 24 km body, 470 m on 30 km), bilinear. Corner-aligned nodes repeat along shared face edges with the
same value, so bilinear sampling is continuous across faces. Pyramid + deltas track the analytic
field within 2.4–4.8 m p99 (`genesis_gen` test `cached_h0_tracks_the_analytic_field`).

### Analytic deltas

The coarse grid cannot represent a crisp ridge or trench, so the unresolved part of L0 is added
analytically at query time. The bake computes the same terms from the same seeds, so the pyramid
and the deltas are consistent.

```text
plateDelta(p)   = Σ_{i<j} activity · amp_ij(p) · 4 · w_i(p) · w_j(p)   // soft Voronoi, see below
craterDelta(p)  = Σ_k profile_k(|p − c_k|)    // from the crater list, via the crater index
erosionDelta(p) = lowFrequencyErosion(p)      // climate-driven, from 05 and 06; 0 until climate exists
```

`DECIDED` (P1-A, confirmed by the user 2026-11-13; replaces the earlier `Σ_b amplitude_b · type_b ·
smoothstep(1 − d_b / width_b)`): **plates are a soft Voronoi.** The numbers stay `PROPOSED`. A "nearest boundary" sum switches abruptly
wherever the nearest boundary changes, which makes vertical walls along lines from every triple
junction (the [B-005](../work/BUGS.md) lesson). Instead:

- `w_i(p) ∝ exp(−(d_i − d_min) / softness)`, normalised, where `d_i` is the geodesic distance from
  the domain-warped `p` to plate `i`'s seed point. The weights are equal exactly on the Voronoi
  bisectors, so plates and boundaries are where [06](./06-geology-and-landforms.md#plate-layout) puts
  them; the warp keeps boundaries from being great-circle arcs.
- `4 w_i w_j` is 1 on a two-plate boundary and fades away from it and at junctions, so the
  boundary width is set by `softness` (half-width ≈ 1.8 × softness).
- `amp_ij(p) = c · A(types)` for convergence `c ∈ (0, 1]` and `c · rift` for `c < 0`, where
  `c = (v_i − v_j) · t̂ / |v_i − v_j|` is the relative surface velocity from the plates' Euler-pole
  rotations along the normal `t̂` from `i` towards `j`. Transform motion gives `c ≈ 0`: no height.
- The same weights (with a wider softness) blend the continental and oceanic base elevations in
  `macroHeight`, which is what the pyramid caches.

The result is continuous everywhere: `genesis_gen`'s test walks great circles in 10 m steps and
finds at most 7.9 m per step; a discontinuous crater rim fails the same test at 68 m.
Craters use the continuous profile from B-005: bowl `−depth · (1 − t²)` for `t = dist / r < 1`,
rim `0.8 · depth · u · (1 − u)` for `u = t − 1 ∈ [0, 1)`, 0 beyond `2r`.

Craters are an accumulation model over age provinces, not per-sample noise; the crater field is
described in [06](./06-geology-and-landforms.md). Erosion is a low-frequency modulation, not a
per-sample simulation.

```text
h0(p) = macroHeight(p, lod) + plateDelta(p) + craterDelta(p) + erosionDelta(p)
```

The footprint of L0 is deliberately additive: bake and query must produce the same sum. If a term
is baked into the pyramid, it must not also be added at query time.

---

## L1 — Climate & hydrology

Analytic fields evaluated from the descriptor's climate parameters. They are low frequency, cheap,
and never stored per sample. L1 returns a `ClimateSample`:

```text
climate(p, lod) -> ClimateSample:
    T_mean = T_base
           + latitudeTerm(lat)
           + altitudeLapse(altitude)
           + orographicCooling(p, wind)
           + coastalModeration(p)

    m      = clamp(m_base + m_latitude + m_water
                   + m_orographic − m_shadow − m_cold, 0, 1)

    wind   = windBands(lat, descriptor) + terrainDeflection(p)

    diurnalRange, seasonalRange = f(transport, coastalFactor, insolation)
    snowLine = altitude + (T_mean − 273.15) / lapseRate   // null when below the surface

waterSurface(p) = max(seaSurface, highest lake surface containing p)
```

The full equations, tuning constants, tidally locked model, and the baked water-distance field live
in [05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md#the-temperature-field). Three rules
matter here:

- **Climate uses `h0`, not the final `h`.** If climate depended on L3 detail, it would change with
  LOD and biome borders would swim. The lapse term uses `altitude = h0 − seaSurface`, not raw
  elevation.
- **Classification uses the annual mean.** `temperatureAt(p, t)` adds seasonal and diurnal deltas
  for survival and visuals, but never feeds L2.
- **Wind is a coarse field, not a simulation.** A latitude-band function rotated by a per-planet
  offset plus terrain deflection is enough; see 05.

L1 outputs feed L2 classification and L3 (dune orientation, snow line, moisture-dependent
materials). The `ClimateSample` is carried on `SurfaceSample` for survival, materials, and the
inspector.

---

## L2 — Classification

L2 turns continuous fields into **weights**, never a single painted biome. The algorithm is
specified in full in [19 — Biome Classification](./19-biome-classification.md); this section is
the summary the field stack depends on.

```text
classify(p, lod) -> BiomeWeights { families, leaves, strata }
```

1. **Inputs** come from `h0` and L1 only: annual-mean climate, `altitude = h0 − seaSurface`, slope
   of `h0`, and substrate as a **weight vector** from the material region layer. Landform detail
   cannot move biome borders. The final `SurfaceSample.altitude` includes L3 and is for UI/debug
   only.
2. **Jitter** perturbs axis values with band-limited noise, which makes borders irregular without
   re-evaluating fields at a warped position.
3. **Strata** blend softly across the waterline and cave mouths.
4. **Candidates** are the frozen palette filtered by a conservative region index (an exact
   optimisation, never an approximation).
5. **Suitability** is a product of smoothstep-edged axis scores; families use the max over their
   variants.
6. **Prior** is the global weight × per-body palette weight × a per-family mosaic noise, which
   gives patches instead of uniform mixes.
7. **Soft budget** keeps the top *K* families continuously.
8. **Fallback** adds `ε` to the stratum's fallback biome, so the barren take-over is smooth.
9. **Normalisation** is `v^k / Σ v^k`: zero stays zero.
10. **Ecotones** inject transition biomes where two families mix.
11. **Variants** split each family's weight among its members.

Rules the rest of the stack relies on:

- Evaluate everything in **sorted content-ID order** so floating-point accumulation is
  deterministic.
- Suite weights use the same soft scoring for their context; see
  [17](./17-biome-and-landform-format.md#context-matching).
- Material weights come from composition and weathering (see
  [06](./06-geology-and-landforms.md) and [07](./07-biomes-and-ecology.md)).

This is where the "blend pass" of the original design disappears: blending is a property of the
weights, not a post-process.

---

## L3 — Landform detail

L3 adds the parameterised features of each landform suite, plus shoreline shaping and material
resolution. It is the layer that makes two worlds with the same biome palette look different.

### Suite composition

Each suite is a module (data + code) that contributes a **delta relative to the base terrain**, not
an absolute height:

```text
h3(p) = Σ_j suiteWeights[j] · suiteHeight_j(p, lod)
```

Rules:

- A suite must return `0` outside its context, or fade to zero at the context edge. It must not
  bias the whole planet.
- A suite's amplitude comes from its per-body `params` (see [03](./03-planet-descriptor.md) and
  [17](./17-biome-and-landform-format.md)).
- A suite is evaluated only if its `feature_scale` passes the band limit for the current LOD.
- Suites blend; they do not replace each other.

### Shoreline shaping

The waterline is the one place where height, material, and gameplay must agree exactly. The query
computes a signed distance to the water surface and shapes a narrow band:

```text
d = h − waterSurface
if |d| < shoreBand and slope < cliffSlope:
    h = waterSurface + beachProfile(d / shoreBand) · shoreBand
    flags.shoreline = true
else if |d| < shoreBand and slope >= cliffSlope:
    flags.shoreline = true                 // cliff: keep the steep profile
```

`shoreBand` is a tuning value (order of metres) from the descriptor/climate; `beachProfile` is a
monotonic, slope-limited curve. This produces beaches, berms, and coastal flats without a fluid
simulation.

### Material resolve

Material is the last surface step, after height and water:

```text
blend = normalize(
    paletteBlend(biomeWeights)          // base look from 17
  + rockExposure(slope)                // steep faces expose rock
  + wetness(depth)                     // below water, blend to wet variants
  + snowIce(c, altitude)               // cold/high surfaces; uses snowLine
)
```

- `snowIce` gates on `altitude >= c.snowLine` rather than on a raw temperature threshold, so the
  material and the reported `snowLine` cannot disagree.
- The **instantaneous** `temperatureAt(p, t)` may add a temporary frost/dusting, but it must not
  move the permanent snowline. Permanent snow is the annual-mean material; seasonal frost is a
  shader/coverage overlay.
- A frozen water surface uses the ice variant of the water body's material. It is selected by the
  same temperature gate; it is never a separate mesh. See
  [05](./05-climate-and-hydrosphere.md#ice).

The output `MaterialSample` carries the dominant `materialID`, the full blend, albedo, roughness,
and wetness. The renderer may use the blend; collision only needs height.

### Scatter

Scatter is evaluated per chunk from the sample's weights and the scatter rules in
[17](./17-biome-and-landform-format.md#scatter-rules). It must use the **final** height and water
surface, so nothing floats or sits inside the ground. Scatter is deterministic from the chunk seed
(see [11](./11-determinism-and-sampling.md)).

---

## L4 — Micro detail

The smallest changes: pebbles, cracks, grain, small props, and cleanup. Only evaluated near the
player (LOD 4, and optionally LOD 3 for nearby patches).

- Adds height/normal perturbation and micro-props, not new biomes.
- Fades out over one octave before the LOD cutoff, so detail does not pop.
- Never changes the L0 silhouette or the L2 weights.
- Performs cleanup: remove scatter below ground or inside water, snap props to the final height,
  weld chunk seams.

L4 is where you can afford to be expensive, but it must still be band-limited and deterministic.

---

## Evaluation order (per sample)

```text
sampleSurface(p, lod):
    # 1. Macro
    h = macroHeight(p, lod)
    h += plateDelta(p) + craterDelta(p) + erosionDelta(p)

    # 2. Climate (computes h0 internally; never uses the final height)
    c = climateSample(p, lod)

    # 3. Classification
    w = classify(p, h, c, descriptor)          # biomeWeights + suiteWeights

    # 4. Landform detail
    h += Σ suiteHeight(s, p, lod) · w.suiteWeights[s]

    # 5. Micro detail (near field only)
    if lod >= NEAR_LOD:          # NEAR_LOD = 4 in the LOD scheme from 10
        h += microHeight(p, lod, w)

    # 6. Water and shoreline
    water = waterAt(p, h)
    h = shoreShape(h, water, c)

    # 7. Material and normal
    m = resolveMaterial(p, h, c, w, water)
    n = normal(p, lod, h)

    return SurfaceSample {
        position: p, normal: n, height: h,
        altitude: h − seaSurface,
        climate: c, material: m,
        biomeWeights: w.leaves, biomeFamilies: w.families,
        suiteWeights: w.suiteWeights,
        water: water, flags: flags
    }
```

Notes:

- `waterAt` uses the pre-shaping height to decide which water body contains `p`; `shoreShape` then
  shapes the band around that surface.
- `flags` is assembled by the caller: `submerged` from `water`, `shoreline` from shaping, `ice`
  from L1, and `edited` from the streaming layer.
- Player **edits** are applied after this function by the streaming layer. The field stack is pure
  and does not read saves. The edit record, its ordering, and how it composes with
  `sampleSurface` are defined in [18 — Collision & Edits](./18-collision-and-edits.md#edits).
- For **benthic** samples, step 2 also evaluates depth and light, and step 3 uses the water-aware
  classifier. **Pelagic** samples bypass this order and use the volume query.

---

## Band-limiting and LOD

Every layer has a characteristic frequency. A patch only evaluates features whose feature size is
resolvable at its sample spacing. Two quantities are involved (see
[10 — LOD & Streaming](./10-lod-and-streaming.md#quadtree-level-versus-lod-band)):

```text
s(level) = faceArc / (2^level · intervalsPerPatch)   // the patch's actual spacing
band     = band(level, R)                            // the LOD band, 0..4
```

- The **band** gates content: an entry runs only when `min_lod <= band <= max_lod`.
- The **actual spacing** drives the fade. Inside an active window, a feature with `feature_scale`
  contributes with weight:

```text
r      = feature_scale / (2 · s(level))       // >= 1 means resolvable (Nyquist)
weight = smoothstep(0.5, 1.0, r)
```

Because `s(level)` halves with every level, detail fades in smoothly as the quadtree deepens, not
in a step at a band boundary. The `lod` argument of `sampleSurface(p, lod)` carries both the level
and the band.

| LOD | Layers | What is added |
|---|---|---|
| **0** | L0 | macro silhouette from the pyramid |
| **1** | L0 + coarse L1 | broad climate and water surface |
| **2** | L0–L2 + coarse L3 | biome/suite weights and large landforms |
| **3** | L0–L3 | full landform detail and shoreline |
| **4** | L0–L4 | micro detail and cleanup |

Rules:

- A layer must not return detail above the cutoff for the current LOD. This prevents aliasing and
  popping.
- When LOD increases, new detail appears as if it was always there; the macro shape must not
  change. This is why L0 is baked and shared.
- LOD transitions morph vertex positions between adjacent levels rather than re-evaluating with a
  different classifier.

See [10 — LOD & Streaming](./10-lod-and-streaming.md).

---

## Domain warping

Domain warping is the main tool for making procedural fields look natural rather than
mathematical:

- warp the landform suite coordinates, so dune fields and karst boundaries meander;
- warp the macro field slightly at mid frequencies, to avoid perfectly smooth plate edges.

```text
warp(p, S, A) = p + A · vec3(
    noise3(p / S + offsetX),
    noise3(p / S + offsetY),
    noise3(p / S + offsetZ)
)
```

Warping is done with a low-frequency noise field whose amplitude is proportional to the feature
scale it is applied to. It must itself be band-limited, or it reintroduces aliasing. Use separate
offsets per purpose (suite, macro) so the warps are decorrelated.

Biome borders do **not** use positional warp. They use value jitter on the classification axes,
which looks equivalent and avoids evaluating climate twice; see
[19](./19-biome-classification.md#2-jitter).

---

## Normals and derivatives

The height field is radial: `r = bulk.radius + h`. The outward normal is the radial direction
corrected by the tangential gradient of `h`:

```text
p_dir = normalize(p)
g     = (h(p + ε·e_x) − h(p − ε·e_x)) / (2ε) · e_x
      + (h(p + ε·e_y) − h(p − ε·e_y)) / (2ε) · e_y
n     = normalize(p_dir − g)
```

Rules:

- `(e_x, e_y)` is a local tangent basis at `p`.
- `ε = max(s(lod), minFeatureScale(lod))`, where `minFeatureScale` is the smallest feature
  evaluated at that LOD; sampling with a smaller epsilon than the band causes aliasing.
- Normals must be computed from the **same** height function used for collision, or the player will
  visibly float or sink.
- At chunk borders, sample across the border so normals are continuous.

---

## Determinism of evaluation

The query function must be pure:

- It takes the descriptor and a position; it returns a sample.
- It does not depend on what was queried before.
- It uses counter-based RNG keyed by identity and purpose when it needs randomness (e.g. scatter),
  never a shared stream (see [11](./11-determinism-and-sampling.md)).

Additional rules that matter at this layer:

- **Quantise positions before hashing.** Use a grid coarser than float precision (order 1 cm at
  planet scale) or chunk-local coordinates, so tiny floating-point differences do not change
  hashes.
- **Iterate content in sorted ID order.** Softmax and weighted sums must accumulate in a fixed
  order; otherwise the same sample can differ across machines.
- **No shared mutable caches** inside the query. The macro pyramid, region index, and water-distance
  field are read-only.
- **Same math mode.** Avoid compiler fast-math that changes results between builds.

This is what allows chunks to be generated out of order, on another thread, or on another machine,
and still match.

---

## Performance budget

Measured in [E1](./spikes/E1-field-cost.md), cost is dominated by **noise evaluations and
repeated `h0` evaluations**, not by content count: going from 8 to 32 biomes adds about 30%. One
`gradient3` costs ~14 ns; `h0` costs ~166 ns and a full sample evaluates it four times (itself,
the orographic upwind sample, and two slope differences).

| Stage | Work per sample | Measured (E1, 1 thread) | Guard |
|---|---|---|---|
| L0 | 1 pyramid fetch + up to 3 deltas | ~168 ns | skip zero terms; **spatial index for craters** |
| L1 | ~4 scalar fields + 1 upwind `h0` | ~242 ns | low frequency; reuse across neighbouring samples |
| L2 | 2 slope `h0` + 5 noise + classifier | ~644 ns | region index; soft budget *K* |
| L3 | active suites only | ~77 ns (4 suites) | `feature_scale` and context gate |
| L4 | near field only | ~51 ns | distance and LOD gate |
| **Total** | | **~1.2 µs** (budget 4 µs) | L0–L2 ~1.05 µs (budget 1 µs) |

Rules:

- The soft budget in [19](./19-biome-classification.md#7-the-soft-budget) bounds L2 continuously.
  The inspector reports where it is active; it is not an error.
- Cache the macro pyramid, lake list, region index, and water-distance field. Never cache L1–L4
  sample values globally; they are cheap to recompute and caching them explodes memory.
- Any new per-sample `h0` evaluation or noise field is a cost decision: it is the dominant term.
  `PROPOSED` (from E1): bake the classification inputs that need extra `h0` evaluations (slope,
  orographic upwind term) on the macro grid if E4 needs the throughput.

---

## Invariants

These must hold for every sample, at every LOD, on every body with a surface:

1. **Height is continuous** across chunk and face borders. No cracks, no steps.
2. **Normals match the height derivative** at the same LOD and epsilon.
3. **Water agreement:** `water.depth > 0` if and only if `height < water.surface`, within the same
   water body.
4. **Scatter agreement:** no instance floats above the ground or intersects it after cleanup.
5. **Weights normalise:** `biomeWeights` and `suiteWeights` sum to 1 (or are empty when there are
   no candidates).
6. **No detail above cutoff:** a layer never returns features its LOD cannot resolve.
7. **Order independence:** the same sample from any order, thread, or machine is identical.
8. **LOD monotonicity:** increasing LOD adds detail without changing the L0 silhouette.
9. **Sea-level consistency:** the water surface comes from the same `seaRadius`/lake data used by
   bake and collision.
10. **Edits are separate and idempotent:** applying the same edit twice has the same result, the
    pure query never observes it, and the final surface is produced exactly once by the edit
    layer in [18](./18-collision-and-edits.md#edits).

A violation is a bug in the field stack or in content, not a tuning problem.

---

## Worked example

A coastal temperate sample at LOD 3, on a body with `bulk.radius = 24 000 m` and
`seaRadius = 24 040 m` (sea level at `height = +40 m`).

| Step | Result |
|---|---|
| L0 macro | `+120 m` |
| Plate delta | `+35 m` |
| Crater delta | `−5 m` |
| Erosion delta | `−10 m` |
| **L0 height** | **`+140 m`** |
| L1 altitude | `+100 m` above sea level |
| L1 temperature | `285 K` |
| L1 moisture | `0.70` |
| L1 wind | `(4, 0, 1) m/s` |
| L2 biome weights | forest `0.55`, grassland `0.25`, wetland `0.12`, alpine `0.08` |
| L2 suite weights | rolling_hills `0.70`, river_valleys `0.30` |
| L3 suite delta | `+18 m` |
| **Final height** | **`+158 m`** |
| **Final altitude** | **`+118 m`** |
| Water | sea surface `+40 m`, depth `0`, shoreline `+118 m` |
| Material | soil/grass, ~15 % rock exposure on the local slope, wetness `0` |
| Normal | computed from the LOD-3 height band |

The sample is dry land, 118 m above the waterline, so no shoreline shaping applies. At LOD 2, the
same sample would use only coarse suite detail and return a slightly smoother height; the L0
silhouette and the biome weights would still agree.

---

## What is cached, and where

| Data | Where | Lifetime |
|---|---|---|
| Macro pyramid (L0) | descriptor cache / on disk | permanent per body |
| Region index | descriptor cache / on disk | permanent per body |
| Water-distance field | descriptor cache / on disk | permanent per body |
| Lake list | descriptor | permanent |
| Chunk meshes | streaming cache | while in range |
| Collision patches | streaming cache | while in range |
| Scatter instances | chunk generation | regenerated per chunk |
| Edits | save file, applied after generation | permanent |

Do not cache L1–L4 sample values at global scale; they are cheap to recompute and caching them
explodes memory.

---

## Mapping the original five passes

| Original pass | What it was trying to do | Where it lives now |
|---|---|---|
| 1. Top-level noise: continents, oceans, mountains, trenches | macro relief | **L0** — plate/impact-driven macro field, not noise |
| 2. Biome definition dispersed by temperature | regional variation | **L2** — soft classification of L1 climate fields (derived, not painted) |
| 3. Smaller features: pillars, rocks, ravines, hills, dunes | landform detail | **L3** — landform suite modules |
| 4. Most detailed changes, cleanup | micro detail | **L4** — near-field only |
| 5. Blending biome transitions | seamless transitions | **removed** — blending is inherent in L2 weights + axis jitter |
| (rivers) | water features | **removed** — lakes only; outflow paths optionally used as dry valleys |

Two explicit corrections carried from the design discussions:

- **Rivers are out of scope.** Water is a level, not a flow. See
  [05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md).
- **Craters are not noise.** They are an accumulation model over provinces. See
  [06 — Geology & Landforms](./06-geology-and-landforms.md).

---

## Related

- [03 — The Planet Descriptor](./03-planet-descriptor.md) — the input to every query.
- [05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md) — L1 fields in depth.
- [06 — Geology & Landforms](./06-geology-and-landforms.md) — L0 and L3 content.
- [07 — Biomes & Ecology](./07-biomes-and-ecology.md) — L2 classification and region layers.
- [10 — LOD & Streaming](./10-lod-and-streaming.md) — how layers map to LOD and chunks.
- [11 — Determinism & Sampling](./11-determinism-and-sampling.md) — purity, hashing, and RNG.
- [16 — Data Contracts](./16-data-contracts.md) — feature scale, units, and frozen data.
- [17 — Biome & Landform Format](./17-biome-and-landform-format.md) — suitability, suites, and scatter.
- [18 — Collision & Edits](./18-collision-and-edits.md) — collision and the edit overlay on this field.
- [19 — Biome Classification](./19-biome-classification.md) — the L2 algorithm in full.
