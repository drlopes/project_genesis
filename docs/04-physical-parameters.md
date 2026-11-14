# 04 — Physical Parameters & Derived Labels

This is the document that prevents the system from collapsing back into a flat list of planet
types. It defines the **independent axes** a body is sampled along, and how human-readable
**labels** are computed from them.

Read this before adding any new category. Most "new planet types" are a combination of existing
axes, or a new derived label, or a narrative overlay — not a new enum.

---

## The core rule

> **Axes are inputs. Labels are outputs.**
>
> Generation samples physical axes. Physics and classification then produce labels
> (`barren`, `volcanic`, `ocean_world`, …). Labels are used for UI, weighting, and generator
> dispatch — **never** as gates on other axes.

Why this matters:

- It removes contradictions ("volcanic + frozen" is fine: cryovolcanism).
- It collapses the combinatorial explosion: instead of authoring `N temperatures × M terrain
  types × K affinities`, you sample ~8 independent distributions.
- It lets new labels emerge from combinations you did not anticipate, which is exactly the
  variety the game needs.

---

## The axes

| Axis | Parameters | Range / units | Drives |
|---|---|---|---|
| **Bulk** | radius, mass, composition (density is derived) | SI / game units | gravity, escape velocity, atmosphere retention, whether it can be a gas giant |
| **Energy budget** | insolation, albedo, internal heat, tidal heat | normalised | surface temperature |
| **Atmosphere** | presence, surface pressure, composition, greenhouse, scale height, magnetosphere | Pa, fraction, K offset | climate, sky, habitability, erosion |
| **Hydrosphere** | water inventory, state (liquid/ice/vapour), ocean fraction, salinity | fraction | sea level, coastlines, moisture, albedo, biome |
| **Geology** | plate count, tectonic activity, volcanism, crater age provinces, erosion regime | normalised | landforms, relief, material exposure |
| **Rotation & orbit** | day length, obliquity, tidal lock, eccentricity, semi-major axis | s, deg, AU-equivalent | climate symmetry, seasons, sky |
| **Biosphere** | level (none/microbial/complex/exotic), density, palette | enum + fraction | ecology, atmosphere, biome palette |
| **Narrative overlay** | affinity, anomalies, faction tags, POIs | tags | appearance modifiers, content |

Bulk and composition are special: they determine the **body class** and therefore which
generator runs (see [13](./13-generator-modules.md)). Everything else tunes the output of that
generator.

`PROPOSED` (P1-A, the `rocky` class defaults in `genesis_gen::RockyParams`; they move to content
data with `genesis_content`). Uniform draws unless noted, each from the `bulk` seed:

| Parameter | Range | Note |
|---|---|---|
| radius | 16–32 km | Q4 still open; E3 ran 8 / 24 / 40 km |
| gravity | 0.2–0.6 g | authoritative (docs/16) |
| mass | `gravity · g₀ · radius² / G` | a placeholder until orbits exist; may then be tuned for orbits and tides |
| day length | 300–1800 s | game seconds |
| obliquity | 0–40° | |
| tidal lock | `null` | needs the orbit phase |

Per-region stratified sampling (docs/11) waits for the system bake: a body baked alone draws
independently.

`PROPOSED` (P1-C1, the new phases in `genesis_gen::RockyParams::climate`; stand-ins until stars and
orbits exist — full formulas in [05](./05-climate-and-hydrosphere.md#as-implemented-energy-and-atmosphere-p1-c1)):

| Parameter | Draw | Note |
|---|---|---|
| insolation | `exp(0.3 · N(0,1))`, clamped 0.25–4 | relative to Earth; lognormal until a star gives it |
| albedo | uniform 0.2–0.45, then the ice feedback | raises toward 0.6 as the sea freezes |
| internal heat | uniform 0–1 | ×10 K surface warming |
| tidal heat | 0 | needs moons and orbits |
| atmosphere | present with probability 0.85 | |
| surface pressure | lognormal, median 60 kPa, `σ_ln` 0.8, clamped 1 kPa–1 MPa | 0 when absent |
| greenhouse | `−33 K + 33 K·√(min(p/1 bar, 3)) + N(0, 8 K)`; exactly `−33 K` when absent | offset from Earth's baseline |
| biosphere level | weights none 0.1 / microbial 0.2 / complex 0.6 / exotic 0.1 | only with liquid water and 240–330 K |
| biosphere density | 0.1–0.4 microbial, 0.3–1.0 otherwise | 0 with no life |

The ice feedback's two iterations make the albedo itself a result, not a dial: a cold ocean gets
brighter and colder, which is how frozen worlds stay frozen.

---

## Body class vs derived label

This is the distinction that resolves the "terrain type" confusion in the original design.

**Body class** answers: *what kind of object is this, structurally?* It selects a generator.

- `rocky` — solid surface, heightfield-based generation
- `ocean` — global ocean shell over a seafloor
- `gas_giant` — no surface; volumetric banded atmosphere
- `ice_giant` — no surface; volatile-rich atmosphere
- `artificial` — constructed body, special generator
- `belt` — asteroid belt, not a single body

**Derived label** answers: *what does this particular object look like / play like?* It is
computed after sampling.

- `barren` — no biosphere, thin atmosphere, little/no surface water
- `volcanic` — high internal heat + high tectonic activity
- `ocean_world` — `ocean` class, or a `rocky` body with ocean fraction above a threshold
- `desert` — low moisture, high temperature
- `ice_world` — water inventory present but frozen; low insolation
- `ringed` — has a ring system
- `habitable` — surface pressure and temperature in a breathable/livable range
- `tidally_locked` — rotation matches orbit

`barren`, `volcanic`, `desert`, and `ice_world` are **labels on a `rocky` body**, not separate
generators. A `water_world` is either an `ocean` class (global ocean) or a `rocky` body with a
high ocean fraction — decide once and be consistent.

`DECIDED` — labels are computed by a pure function `classify(parameters) → labels` during bake,
and stored in the descriptor for UI and weighting.

---

## Derived label rules (initial)

These are proposed thresholds, intended to be tuned in data (see
[12](./12-authoring-and-tuning.md)). They are deliberately simple; the point is that they are
*computed*, not authored.

| Label | Rule of thumb |
|---|---|
| `barren` | `biosphere == none` ∧ `atmosphere.pressure < 0.1 bar` ∧ `oceanFraction < 0.02` |
| `volcanic` | `geology.volcanism > 0.6` ∧ `energy.internalHeat > 0.5` |
| `ocean_world` | `hydrosphere.oceanFraction > 0.85` |
| `desert` | `climate.moisture < 0.2` ∧ `meanSurfaceTemp > 280 K` |
| `ice_world` | `hydrosphere.state == ice` ∧ `oceanFraction > 0.1` |
| `habitable` | `0.5 bar < pressure < 3 bar` ∧ `260 K < meanTemp < 320 K` ∧ `biosphere != none` |
| `tidally_locked` | `rotation.locked == true` |
| `hot_jupiter` | `class ∈ {gas_giant}` ∧ `orbit.semiMajorAxis` small |
| `temperate` | `meanSurfaceTemp ∈ [270, 310] K` |

`PROPOSED` — these thresholds should live in a config file, not in code. The classification
function should be data-driven so designers can retune "what counts as habitable" without a
build.

### Temperature bands

The original design listed `Hellish / Scorched / Volcanic / Arid / Temperate / Chilled /
Frozen / Barren / Perma-Frost`. That list mixes temperature with geology and biosphere.
Replace it with:

- a **continuous temperature field** (see [05](./05-climate-and-hydrosphere.md)),
- a set of **named bands** for UI/flavour, e.g. `infernal` (> 700 K), `scorched`,
  `hot`, `temperate`, `cool`, `cold`, `frozen`, `cryogenic`,
- separate labels for `volcanic` (geology) and `barren` (biosphere/hydrosphere).

Bands are presentation. They never gate generation.

`PROPOSED` — band boundaries and names are content; tune freely.

---

## Plausibility as soft constraints

Physics should bias sampling, not forbid outcomes. Implement the important relationships as
**soft constraints** (reject-or-resample with a small probability of acceptance, or weighted
sampling) so that rare "impossible" worlds survive as anomalies:

| Relationship | Constraint | Relaxation |
|---|---|---|
| Atmosphere retention | escape velocity must exceed thermal velocity of the gas | magnetosphere and cold temperatures help; heavy gases retained on small bodies |
| Frost line | volatiles condense beyond a radius | migration and scattering can move ice-rich bodies inward |
| Habitable zone | liquid water is favoured | geothermal, tidal, greenhouse, or artificial warming allow outliers |
| Tidal locking | close orbits lock | young systems may not have locked yet |
| Tectonics | needs mass and internal heat | small bodies can be geologically active via tidal heating |
| Rings | inside the Roche limit | can be young, shepherded, or artificial |

`DECIDED` — constraints are soft. A world that violates one must be **tagged as an anomaly**
and given a narrative explanation (see [08](./08-narrative-layer.md)), not silently generated.

This is how you get both plausibility *and* wonder: the rules are legible, and the exceptions
are deliberate.

---

## Affinity is not an axis here

Affinity (`mechanical`, `infected`, `corrupted`, `wild`, `gaia`, …) is a **narrative overlay**,
not a physical axis. It is applied after physical and ecological generation as a modifier stack.
That keeps the combinatorial space manageable and lets the same physical world host different
story flavours.

See [08 — Narrative Layer](./08-narrative-layer.md).

---

## Worked example

A temperate ocean world with a large moon:

1. Sample bulk → `rocky`, radius 22 km, mass 2.8e21 kg, gravity 0.38 g (game scale;
   `gravity` is authoritative, mass is tuned for orbits and tides).
2. Sample orbit inside the habitable zone → insolation 0.95, mild eccentricity.
3. Sample atmosphere → 0.9 bar, N₂/O₂/Ar, greenhouse +12 K.
4. Sample water inventory → 0.7, ocean fraction 0.62, state liquid, marine water body.
5. Sample geology → 6 plates, activity 0.6, volcanism 0.25, two crater-age provinces.
6. Evaluate coarse relief → derive sea radius from the hypsometric curve.
7. Sample climate parameters → mean 288 K, moisture 0.55.
8. Sample regions → ecological: 6 regions; geological: highlands, plains, rift; material:
   basalt, sediment.
9. Sample biosphere → complex, density 0.7.
10. Choose biome palette → forest, grassland, wetland, alpine, coastal.
11. Choose landform suites → rolling hills, river valleys, sea cliffs.
12. Narrative → no affinity; one ruin POI; naming culture `sol_common`.
13. Derive labels → `temperate`, `ocean_world`, `habitable`, `ringed?`, `tidally_locked?`.

Nothing above required a "temperate ocean world" enum. The label was the *result*.

---

## Related

- [02 — Galaxy & Systems](./02-galaxy-and-systems.md) — where axes are sampled from profiles.
- [03 — The Planet Descriptor](./03-planet-descriptor.md) — where the sampled values are stored.
- [05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md) — energy, temperature, water.
- [06 — Geology & Landforms](./06-geology-and-landforms.md) — tectonics and landform suites.
- [07 — Biomes & Ecology](./07-biomes-and-ecology.md) — how axes become biomes.
- [08 — Narrative Layer](./08-narrative-layer.md) — affinities and anomalies.
- [13 — Generator Modules](./13-generator-modules.md) — body classes and their contract.
