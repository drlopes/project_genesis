# 19 — Biome Classification

This document is the single source of truth for **how a sample becomes biome weights**. It is the
implementation contract for layer L2 of the [surface field stack](./09-surface-field-stack.md).

- [07 — Biomes & Ecology](./07-biomes-and-ecology.md) explains *what* biomes are and why they are
  derived rather than painted.
- [17 — Biome & Landform Format](./17-biome-and-landform-format.md) defines the content schema.
- This document defines the algorithm: inputs, scoring, normalisation, variants, ecotones, the
  bake-time half (palette and region index), invariants, and a worked example.

The design target is simple to state: **biome weights are a continuous, LOD-independent,
time-independent function of the descriptor and the position**, and a biome that does not fit a
place gets exactly zero weight there.

---

## Why the earlier sketch was not enough

The first version of L2 was `softmax(score × weight)` over a capped candidate list. It had defects
that would show up as visible bugs:

| Defect | Symptom | Fix in this doc |
|---|---|---|
| `softmax` maps a score of `0` to `e⁰ = 1` | unsuitable biomes leak into every sample | [power normalisation](#9-normalisation) |
| tied biomes split 50/50 across their whole overlap | permanent grey mixes instead of patches | [mosaic prior](#6-the-prior-and-the-mosaic) |
| `substrate` was a hard enum | a straight seam at every substrate boundary | [soft enum membership](#5-axis-scores-and-suitability) |
| hard candidate cap | weights jump where the candidate set changes | [soft budget](#7-the-soft-budget) |
| stratum flips at depth 0 | biome table switches instantly at the waterline | [stratum blending](#3-stratum-blending) |
| variants competed with every biome | adding a variant reshuffles the biome map | [two-stage families](#11-variants-two-stage-families) |
| "fall back to weight alone" when all scores are 0 | a discontinuity at the edge of the habitable range | [ε fallback](#8-the-fallback) |

Every step below is continuous in its inputs. A hard edge can only come from content that asks for
one (`falloff = 0`), and the validator warns about that.

---

## Pipeline

```text
classify(p, lod) -> BiomeWeights:
    x       = axisVector(p)                     // 1. inputs, from h0 and climate only
    x'      = jitter(x, p)                      // 2. irregular borders
    σ       = strataWeights(p)                  // 3. soft stratum membership
    for each stratum s with σ[s] > 0:
        C   = candidates(p, s)                  // 4. palette ∩ region index (conservative)
        for each family F in C (sorted by ID):
            S_F = max over members m of suitability(m, x')    // 5. family envelope
            v_F = prior(F, p) · S_F                           // 6. prior × suitability
        v      = softBudget(v, K)                // 7. continuous cap
        v[fb] += ε                               // 8. fallback, always present
        w_s    = normalise(v, k)                 // 9. power normalisation
        w_s    = injectEcotones(w_s)             // 10. transition biomes
        out   += σ[s] · w_s
    leaves  = distributeWithinFamilies(out, x')  // 11. variants
    return BiomeWeights { families: out, leaves, strata: σ }
```

Each step is specified below. Constants in *italics* are tuning values with defaults in
[Tuning constants](#tuning-constants).

---

## 1. Inputs

Classification reads an **axis vector** whose components are defined by the axis registry in
[17](./17-biome-and-landform-format.md#the-axis-registry). The rule that makes biome borders stable:
**every input comes from `h0` (the pre-L3 height) and from L1 climate**, never from L3/L4 detail or
from resolved materials.

| Axis | Source | Notes |
|---|---|---|
| `temp` | aerial: `climate.temperature`; benthic: `T_water(depth)`; subterranean: annual-mean surface temperature at the cave mouth | annual means only; see [05](./05-climate-and-hydrosphere.md#time-mean-versus-instantaneous) |
| `moisture` | `climate.moisture` | annual mean |
| `humidity` | subterranean: `clamp(0.5 + 0.5 · m_surface + floodedFactor, 0, 1)` | `PROPOSED` placeholder |
| `substrate` | **weight vector** from the material region layer plus the slope rule | never from the resolved material (that would be circular) |
| `altitude` | `h0 − seaSurface` | metres above sea level |
| `slope` | `atan(|∇h0|)` in degrees, band-limited at the classification band | not the final normal |
| `latitude` | `|lat|` in degrees | |
| `depth` | `waterSurface − h0` | benthic only |
| `depth_fraction` | `depth / aquatic.maxDepth` | |
| `light` | `exp(−k · depth)` from [15](./15-aquatic-environments.md#classification-inputs) | |
| `current` | normalised analytic current field | |
| `vent` | proximity field from the baked vent/seep list | |
| `water_body` | one-hot for the water body containing the sample | lakes may differ from the sea |
| `ice` | ice cover from temperature and `aquatic.iceCover` | |

### Substrate is a distribution

`substrate` is the one categorical axis that varies across a continuous surface, so it is carried
as a normalised weight vector, not a single value:

```text
substrate(p) = normalise(
    materialRegionWeights(p)                         // from the region layers in 07
  + rockExposure · smoothstep(slopeRock0, slopeRock1, slope)   // steep → rock
  + iceCover(p) · [ice]                              // frozen surface → ice
)
```

Region weights are themselves blended at region edges, so substrate changes smoothly. A sample near
a sand/soil boundary is `{ sand: 0.5, soil: 0.5 }`, and biomes that need either score accordingly.

`water_body` stays one-hot: two different water bodies are always separated by land, so there is no
surface along which it could jump.

---

## 2. Jitter

Borders that follow climate isolines exactly look like contour maps. The earlier design warped the
*position* before evaluating the fields. That costs a second evaluation of every field and makes
the classification climate disagree with the `ClimateSample` the player feels.

Instead, the continuous axes are **jittered by value**:

```text
temp'     = temp     + A_temp     · n_temp(p)
moisture' = moisture + A_moisture · n_moisture(p)
altitude' = altitude + A_altitude · n_altitude(p)
slope'    = slope    + A_slope    · n_slope(p)
```

- `n_axis` is band-limited noise in `[−1, 1]` with wavelength *S_jitter*, seeded by
  `H(contentSeed, "jitter", axis)` (see [11](./11-determinism-and-sampling.md#feature-level-seeds)).
- Iso-contours of `temp + noise` are irregular in exactly the way a warped border is, at a fraction
  of the cost.
- The jittered values are used **only** for classification. `SurfaceSample.climate` reports the
  unjittered values.
- The jitter amplitude is part of the conservative bounds used by the region index (step 4).

`DECIDED` — value jitter replaces positional domain warp for biome borders.

---

## 3. Stratum blending

A sample belongs to strata softly. Near the waterline, both the aerial and the benthic classifiers
run and their results blend:

```text
d          = h0 − waterSurface            // signed; + is dry
t          = smoothstep(−B_strata, +B_strata, d)
σ[aerial]  = t
σ[benthic] = 1 − t
```

- *B_strata* is a few metres. Beaches, mudflats, and reef flats live inside this band and get
  contributions from both tables.
- Classification uses `h0`. The wet/dry flags and the shoreline shaping in
  [09](./09-surface-field-stack.md#shoreline-shaping) use the final height. Within the band the
  two can disagree by design; both tables contribute, so nothing pops.
- `subterranean` blends with its parent stratum by distance from the cave mouth, over *B_cave*.
- `pelagic` is not classified here; it uses the volume query in
  [15](./15-aquatic-environments.md#pelagic-environments).
- Away from boundaries exactly one stratum has weight 1, and the cost is one classifier.

---

## 4. Candidates

A biome is a candidate at `p` if it passes three filters:

1. **Palette** — it is in the descriptor's `biomePalette` (or `aquatic.benthicBiomePalette`). This
   list is frozen at bake; see [the bake-time half](#the-bake-time-half).
2. **Stratum** — its `stratum` matches.
3. **Region index** — the baked spatial index says it *may* score above zero in this cell.

The region index must be **conservative**: it may include a biome that scores zero, but it must
never exclude one that scores above zero. Each cell stores the biomes whose suitability envelope
(ranges plus falloff) intersects the cell's axis bounds, where the bounds are the min/max of each
axis over the cell, widened by the jitter amplitude and the `h0` residual not captured by the macro
grid. An excluded biome therefore has a provably zero score, and filtering it cannot create a
seam.

`DECIDED` — candidate filtering is an exact optimisation, never an approximation.

---

## 5. Axis scores and suitability

### Axis scores

Each axis expression in a biome's `suitability` map produces a score in `[0, 1]`:

| Form | Score |
|---|---|
| `range = [a, b], falloff = f` | `1` inside `[a, b]`; outside, `1 − smoothstep(0, f, distance)`; `f = 0` is a hard edge (validator warning) |
| `curve = [[x, y], …]` | piecewise-linear in `x`, clamped at the ends |
| `any_of = [...]` | `Σ substrate[s]` for `s` in the set (soft membership) |
| `none_of = [...]` | `1 − Σ substrate[s]` for `s` in the set |

Any form may carry `importance = γ` (default `1`). The axis score is raised to `γ`:

```text
axisScore'(a) = axisScore(a) ^ γ_a
```

- `γ = 0` ignores the axis, `γ < 1` softens it, `γ > 1` sharpens it.
- An exponent is the only modifier that composes correctly with a product: it changes how strongly
  an axis falls off without changing where it is satisfied (`1^γ = 1`).

### Suitability

```text
S(biome) = Π_a axisScore'(a)        // default, mode = "all"
S(biome) = max_a axisScore'(a)      // mode = "any"
```

An axis that the biome does not mention contributes `1`. A biome with an empty suitability map
scores `1` everywhere, so the validator rejects it unless `selectable = false`.

### Families

A **family** is a top-level biome plus its variant tree (see
[17](./17-biome-and-landform-format.md#variants)). The top-level competition uses the family's
**envelope**:

```text
S_F = max over members m of S(m)
```

so a variant whose range extends beyond its parent's (an alpine forest colder than the base
forest) still makes the family appear where only the variant fits.

---

## 6. The prior and the mosaic

Suitability says *where a biome can exist*. The **prior** says *how much this world wants it*:

```text
prior(F, p) = weight_F                 // global bias from content, default 1
            · paletteWeight_F          // per-body draw, frozen in the descriptor
            · mosaic_F(p)

mosaic_F(p) = exp(strength_F · n_F(p))
```

- `n_F` is band-limited noise in `[−1, 1]` with wavelength `mosaic.scale`, seeded by
  `H(contentSeed, "mosaic", familyID)`.
- `mosaic.strength` defaults to *mosaic_strength*; `0` disables it.

The mosaic is what turns a 50/50 overlap into **patches**: where forest and grassland are both fully
suitable, their noise fields decide which one dominates locally, giving groves, meadows, and
clearings instead of a uniform mix. Because the power normalisation in step 9 raises `v` to the
`k`-th power, the effective contrast is `strength · k`; tune them together.

`DECIDED` — the per-family mosaic prior is the mechanism for within-climate patchiness.

---

## 7. The soft budget

Per-sample cost must be bounded, so only the top *K* families per stratum contribute. A hard
top-*K* cut would jump whenever two families swap rank. Instead, subtract the (*K*+1)-th value:

```text
if count(v) > K:
    τ   = (K+1)-th largest v
    v_F = max(0, v_F − τ)
```

The (*K*+1)-th order statistic is a continuous function of the inputs, so the result is
continuous. Families at or below the threshold get exactly zero, and the top *K* lose a small amount
that renormalisation restores.

---

## 8. The fallback

Each stratum has exactly one biome marked `fallback = true` (for `aerial` this is usually
`barren`). It is always a candidate, with a tiny constant added to its value:

```text
v[fallback] += ε
```

When every other family fades out (outside the habitable range, on a world with a thin palette),
the fallback takes over **smoothly** instead of by a branch. `ε` is small enough that it never
shows up next to a real biome: after the power normalisation, `ε^k` is negligible unless everything
else is also near zero.

---

## 9. Normalisation

```text
w_F = v_F^k / Σ_G v_G^k
```

- **Zero stays zero.** A family with `v = 0` gets `w = 0`. This is the property `softmax` lacked.
- `k` is the **sharpness** (*k*). `k = 1` is proportional; large `k` approaches a hard argmax.
  Border width scales roughly with `1/k`.
- Scale-invariant: multiplying every `v` by the same constant changes nothing, so the absolute
  magnitude of the global `weight` is irrelevant; only ratios matter.
- `Σ v > 0` always holds because of the fallback, so the division is safe.
- Use an integer `k` so the power is a few multiplications.

`DECIDED` — power normalisation replaces softmax.

---

## 10. Ecotones

Some transitions need their own look: a forest edge, a marsh margin, a salt crust between desert
and a salt lake. An **ecotone** is a content entry that injects a transition biome where two
families mix:

```toml
[ecotone.forest_edge]
between  = ["temperate_forest", "temperate_grassland"]
biome    = "forest_edge"
strength = 0.6
```

```text
e      = strength · 4 · w_A · w_B          // 1 at a 50/50 border, → 0 as either dominates
w[C]  += e
w      = w / Σ w                           // renormalise
```

- `4 · w_A · w_B` peaks at a balanced border and falls off quadratically, so ecotones stay narrow.
  Border width is still controlled by the sharpness `k`.
- The injected biome `C` normally sets `selectable = false`: it appears only through ecotones and
  never competes on its own.
- Ecotones are evaluated in sorted ID order and injected once (no ecotone of ecotones).

### Ecotones replace adjacency constraints

[07](./07-biomes-and-ecology.md#soft-classification-and-blending) originally proposed a biome graph
that biases which biomes may neighbour which. A per-sample classifier cannot see its neighbours, so
that graph cannot run at query time. It splits into two tools:

- **Runtime:** a pair that should transition through something gets an ecotone.
- **Audit:** a pair that should never touch is listed in `incompatible_with`. At bake, and in the
  diversity audit, the macro grid is scanned for samples where both have significant weight; hits
  are reported in the inspector. The fix is to tune suitability or the climate preset. A forbidden
  border is a content bug, not something to hide at runtime.

`DECIDED` — adjacency is handled by ecotones at runtime and by an incompatibility audit at bake.

---

## 11. Variants: two-stage families

Steps 5–10 produce weights over **families** (top-level biomes). Step 11 distributes each family's
weight among its members:

```text
for each family F with w_F > 0:
    for each member m in F (base entry and variants, sorted by ID):
        u_m = (prior_m · S(m))^k_F
    u = u / Σ u                       // if Σ u = 0, all weight goes to the base entry
    leaves[m] = w_F · u_m
```

- `prior_m` is the member's own `weight` (default `1`) times its mosaic, if it declares one.
- `k_F` defaults to the global sharpness.
- Nested variants recurse: a variant's share is distributed among its own children the same way.

Consequences:

- **Identity is stable.** UI, gameplay, and the ecological layer read `families`, so "you are in a
  temperate forest" does not flicker between `temperate_forest` and `temperate_forest_alpine`.
- **Adding a variant is local.** It changes only the internal split of its family. It changes the
  family's weight only where the variant's suitability exceeds the existing envelope.
- **Visuals use leaves.** Palette, scatter, and suite bias resolve per leaf.

`DECIDED` — variants are distributed within their family; families compete at the top level.

---

## Output

```text
BiomeWeights {
  families : map<contentID, float>   // top-level biomes and injected ecotone biomes; sums to 1
  leaves   : map<contentID, float>   // base entries and variants; sums to 1
  strata   : map<stratum, float>     // soft stratum membership; sums to 1
}
```

`SurfaceSample.biomeWeights` carries `leaves`, and `SurfaceSample.biomeFamilies` carries
`families` (see [09](./09-surface-field-stack.md#the-sample-contract)).

### Dominant biome

The biome name shown to the player is the arg-max of `families`. It needs **hysteresis** (switch
only when the new leader exceeds the current one by a margin, default `0.1`), and hysteresis is
state. It therefore lives in the gameplay layer, never in the query.

### Landform suites

Suite context uses the same soft scoring. In
[17](./17-biome-and-landform-format.md#context-matching), `slope` is a range with falloff,
`substrates` is soft membership, and `biomes` is the sum of the listed families' weights. Suite
borders are therefore as continuous as biome borders.

---

## The bake-time half

Two things are decided once per body and frozen.

### Palette selection

During the content-selection phase of [bake](./03-planet-descriptor.md#bake-ordering-within-a-body):

1. Start from every selectable biome in the loaded content whose `requires_biosphere` level the body
   meets and whose stratum exists on the body.
2. Sample the body's axis ranges on the macro grid (min/max temperature, moisture, altitude…).
   Drop biomes whose suitability envelope misses them entirely.
3. Draw `paletteWeight` per family as lognormal jitter around `1`, seeded by
   `H(contentSeed, "palette", familyID)`. Families whose draw falls below *palette_drop* are
   removed. This is what makes two temperate worlds favour different biomes.
4. Cap the palette at *palette_max* per stratum, keeping the highest `paletteWeight × coverage`.
5. Always include the stratum's fallback.
6. Store the result in `biomePalette` / `aquatic.benthicBiomePalette` with resolved IDs.

The palette is a **per-body draw** in the sense of
[12](./12-authoring-and-tuning.md#variation-budgets-and-per-body-draws). Changing it is a
`generatorVersion` or `contentVersion` change, never a runtime effect.

### Region index

Built as a cache in `bakeCaches` (see [13](./13-generator-modules.md#the-bake-contract)), on the
macro grid:

- per cell, the conservative candidate set from [step 4](#4-candidates);
- per cell, the axis bounds used to compute it, kept for the inspector;
- deterministic: candidate lists are sorted by ID.

A missing index is rebuilt from the core and yields identical weights, because it only ever excludes
zero-score biomes.

---

## Tuning constants

| Constant | Default | Meaning |
|---|---|---|
| *k* (sharpness) | `4` | power normalisation exponent; integer |
| *K* | `4` | families per stratum after the soft budget |
| *ε* | `1e-3` | fallback floor |
| *mosaic_strength* | `0.35` | default `mosaic.strength` |
| mosaic scale | `1500 m` | default `mosaic.scale` |
| *S_jitter* | `2000 m` | jitter wavelength |
| *A_temp* / *A_moisture* / *A_altitude* / *A_slope* | `2 K` / `0.05` / `60 m` / `2°` | jitter amplitudes |
| *B_strata* | `3 m` | aerial/benthic blend half-width |
| *B_cave* | `10 m` | subterranean blend half-width |
| *palette_drop* | `0.4` | per-body draw below which a family is dropped |
| *palette_max* | `12` aerial, `8` benthic | palette cap per stratum |

All of these are data, in the classification preset of the core pack, not code constants.

[E1](./spikes/E1-field-cost.md) measured the classifier at ~265 ns per sample with a palette of 16,
and found a mean of 2.7 active families per sample (maximum 5). *K* = 4 rarely binds, so it stays.
The look-related defaults (*k*, mosaic, jitter) are chosen in E8.

---

## Invariants

1. **Continuity.** If the input fields are continuous, `families` and `leaves` are continuous in
   `p`. The only exception is content with `falloff = 0`, which the validator flags.
2. **Zero stays zero.** A biome whose suitability is `0` at `p` has weight `0` at `p`. The single
   exception is the fallback's `ε`.
3. **Normalisation.** `families`, `leaves`, and `strata` each sum to `1` within floating-point
   tolerance.
4. **LOD independence.** Inputs come from `h0` and L1 only; weights are identical at every LOD band.
5. **Time independence.** Seasons and day/night never change weights.
6. **Conservative filtering.** Removing the region index (scoring every palette entry) changes no
   weight beyond tolerance. This is a test, not just a claim.
7. **Determinism.** Candidates, families, members, and ecotones are processed in sorted ID order.
8. **Family stability.** Adding a variant changes `families` only where its suitability exceeds
   the family's previous envelope.

Each invariant gets a property test in the Phase 1 harness.

---

## Inspector output

For the sample under the cursor, the inspector shows, per candidate family and member:

- each axis value (raw and jittered) and its axis score;
- the **limiting axis**, the arg-min axis score. This is the most useful line for a designer:
  "the forest is missing here because `moisture` scores 0.1";
- prior, palette weight, and mosaic factor;
- `v` before and after the soft budget, and the final weight;
- ecotone injections and the stratum blend.

It also shows the region-index cell's candidate list and the incompatibility-audit hits for the
body.

---

## Worked example

A temperate aerial sample. After jitter: `temp = 283 K`, `moisture = 0.58`,
`substrate = { soil: 0.8, rock: 0.2 }`, `altitude = 100 m`, `slope = 8°`. Defaults: `k = 4`,
`K = 4`, `ε = 1e-3`.

| Family | Limiting axis | `S` | `paletteWeight` | mosaic | `v` |
|---|---|---|---|---|---|
| `temperate_forest` | substrate (`soil`/`sand` → 0.8) | `0.80` | `0.42` | `exp(0.35·0.2) = 1.073` | `0.361` |
| `temperate_grassland` | substrate (0.8) | `0.80` | `0.28` | `exp(0.35·−0.5) = 0.840` | `0.188` |
| `boreal_forest` | temp (`[250, 280]`, falloff 6; 3 K out → 0.5) × substrate 0.8 | `0.40` | `0.18` | `1.0` | `0.072` |
| `barren` (fallback) | — | — | — | — | `0.001` |

The `temperate_forest` family's alpine variant (`temp [250, 275]`) scores `0` here, so the family
envelope is the base entry's `0.80`.

Four values and `K = 4`, so the soft budget does nothing. Power normalisation with `k = 4`:

| Family | `v⁴` | weight |
|---|---|---|
| `temperate_forest` | `0.01691` | `0.930` |
| `temperate_grassland` | `0.00125` | `0.069` |
| `boreal_forest` | `0.0000269` | `0.0015` |
| `barren` | `1e-12` | `≈ 0` |

With `k = 2` the same inputs give `0.762 / 0.207 / 0.030`: a wider, softer border.

Ecotone `forest_edge` (`strength = 0.6`):
`e = 0.6 · 4 · 0.930 · 0.069 = 0.154`. After renormalising by `1.154`, the families are
`temperate_forest 0.806`, `forest_edge 0.133`, `temperate_grassland 0.060`,
`boreal_forest 0.0013`.

Stage 2: in the forest family, the base entry scores `0.80` and the alpine variant `0`, so all
`0.806` goes to the base entry. The dominant biome is `temperate_forest`.

The sample sits just inside a forest near a meadow. Moving towards the grassland's mosaic peak, the
weights cross smoothly through a band of `forest_edge` scatter into grassland, with no seam and no
step.

---

## Related

- [07 — Biomes & Ecology](./07-biomes-and-ecology.md) — the concepts this algorithm implements.
- [09 — Surface Field Stack](./09-surface-field-stack.md) — where L2 sits in the evaluation order.
- [05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md) — temperature and moisture inputs.
- [11 — Determinism & Sampling](./11-determinism-and-sampling.md) — seeds for jitter, mosaic, and
  palette.
- [12 — Authoring & Tuning](./12-authoring-and-tuning.md) — per-body draws and the inspector.
- [15 — Aquatic Environments](./15-aquatic-environments.md) — benthic inputs and the pelagic volume.
- [16 — Data Contracts](./16-data-contracts.md) — content IDs and validation.
- [17 — Biome & Landform Format](./17-biome-and-landform-format.md) — the schema.
