# 02 — Galaxy & Systems

This document covers everything above the individual planet: how the galaxy is laid out, how
star systems get their character, how stars and orbits are generated, and how moons and rings
fit in.

The guiding rule: **system profiles weight and decorate physical generation; they do not gate
it.** A physically beautiful water world can exist in a Lifeless system — it simply has no life
and no narrative layer. Physical variety and narrative flavour are separate concerns.

---

## Galaxy layer

### Layout

A galaxy is a set of **regions** (sectors) arranged spatially. Each region carries:

- a **density** of systems
- an **age / metallicity gradient** (older, metal-poor regions toward the core or halo;
  younger, metal-rich regions in the arms — topology is a design choice)
- **faction / political overlays** (who claims it, what they build)
- **narrative overlays** (war, plague, quarantine, uncharted)
- a **system profile distribution** (which profiles are common here)

The galaxy seed plus region index deterministically produces the region's system list. See
[11 — Determinism & Sampling](./11-determinism-and-sampling.md).

`PROPOSED` — exact galaxy topology (spiral arms, ring, cluster). Not required for vertical
slice; a single region is enough to start.

### Why regions exist

They are the natural unit for:

- **stratified sampling** of worlds so the galaxy is not a uniform smear (see
  [11](./11-determinism-and-sampling.md)),
- faction and story overlays without touching individual systems,
- streaming and map UI at scale,
- modding and DLC boundaries.

---

## System profiles

A **system profile** is the character of a whole system. It has two effects:

1. **Weighting** — it biases the probability distributions of physical parameters (insolation,
   water, tectonics, biosphere, anomalies).
2. **Narrative** — it adds a layer of lore-appropriate content (ruins, factions, derelicts,
   hazards, naming culture).

It is **not** a hard filter. It never removes a physically plausible planet from the
distribution entirely; it makes some outcomes common and others rare.

### Initial profile set

| Profile | Weighting bias | Narrative layer | Notes |
|---|---|---|---|
| **Ancient** | older stars, more cratered/degraded surfaces, more tidal locking | ruins, precursor remnants, archaeology | Age shows up in geology, not just props |
| **Untouched** | biosphere weighted up, no artificial structures | wildlife, pristine ecosystems | The "no one has been here" feeling |
| **Lifeless** | biosphere ~0, often smaller/dimmer systems; rarer | none; occasional anomaly / eldritch tag | Physical variety remains: a lifeless ocean world is allowed |
| **Oasis** | habitable-zone weighted, water and resources up, biosphere up | settlements, conflict, contested claims | High POI density, high risk |
| **Frontier** | mixed, resource-rich, low development | outposts, prospectors, pirates | The default "playable" space |
| **War-torn** | normal physics, heavy artificial debris | wreckage, fortifications, minefields | Surface damage as overlays, not terrain rewrite |
| **Anomalous** | rare; deliberately breaks plausibility in one dimension | reality-bending content, unique encounters | The outlier generator; use sparingly |

`PROPOSED` — this list is a starting point, not final. Profiles should be **data**, not code
(see [12 — Authoring & Tuning](./12-authoring-and-tuning.md)). Blended profiles
(`Ancient + Oasis` = ruined paradise) should be supported and are excellent POI magnets.

### Profiles are distributions, not switches

Concretely, a profile is a bundle of per-axis weight tables:

```toml
[profile.oasis]
system_count    = { min = 3, max = 9, distribution = "poisson" }
biosphere       = { none = 0.10, microbial = 0.20, complex = 0.60, exotic = 0.10 }
water_fraction  = { median = 0.55, spread = 0.25 }
tectonics       = { median = 0.6, spread = 0.2 }
anomaly_rate    = 0.05
poi_density     = "high"
narrative_tags  = ["settled", "contested", "resource_rich"]
```

The generator samples each axis from the profile's table, then lets physics and the derived
labels do the rest. See [04 — Physical Parameters](./04-physical-parameters.md).

---

## Stars

### Generation order

1. **Star count** — sample 1–3 (rarely more) from the profile.
2. **Mass / spectral class** — sample mass first, then derive spectral class, luminosity,
   colour, and lifetime. Mass is the primary variable.
3. **Age** — from region age plus a spread; profile can bias it.
4. **Multiplicity configuration** — for binaries and triples, decide whether planets orbit the
   barycentre (P-type, circumbinary) or an individual star (S-type, circumstellar). This
   choice heavily affects where the habitable zone is and how the sky looks.
5. **Luminosity** — derived from mass and age; drives the habitable zone.

`PROPOSED` — spectral classes use a simplified main-sequence model plus post-main-sequence
states (red giant, white dwarf) for Ancient systems. Exotic remnants (neutron star, black hole)
are profile-gated and rare.

### Why mass first

Everything downstream depends on stellar luminosity and lifetime. Sampling a "class" and then
guessing mass produces inconsistent systems. Sample the physical quantity, derive the label —
the same principle as planets.

### Habitable zone

Compute the habitable zone from luminosity (and, for binaries, from the combined flux at the
orbit). Store it in the system descriptor so planet generation and UI can use it.

`DECIDED` — habitable zone is computed and stored, but it **biases** temperature outcomes; it
does not forbid a temperate world outside it (geothermal, greenhouse, artificial explanations
are allowed and are content).

---

## Orbits and bodies

### Mass budget and disk model

Do not roll "number of planets" from a flat dice. Sample:

1. a **protoplanetary disk mass** from the star's mass and the system profile,
2. a **body count** consistent with that mass and the system's age,
3. **orbital slots** via Hill-radius packing and resonance chains.

This produces systems where the spacing, sizes, and types are relationally plausible, which is
most of what makes a system read as "real" to a player.

`PROPOSED` — full disk model can be approximated by a few distributions; the important part is
that *counts, masses, and spacing are correlated*, not independent rolls.

### Orbital slots

- Use **Hill radius packing** with a minimum separation so bodies do not overlap.
- Prefer **resonance chains** (2:1, 3:2) for adjacent bodies where possible; they look
  intentional and are stable-looking.
- Place **asteroid belts** where the disk model predicts failed accretion (often near the frost
  line or in resonances).
- Apply the **frost line**: beyond it, volatiles condense, so ice-rich and gas-rich bodies
  become more likely. This is a soft rule, and a good one — it makes systems legible.

### Body classes at this stage

Every body gets a **body class** (the dispatch key for its generator, see
[13 — Generator Modules](./13-generator-modules.md)):

| Class | Typical location | Has surface? |
|---|---|---|
| `rocky` | inner system | yes |
| `ocean` | habitable zone with high water | yes (ocean shell + seafloor) |
| `gas_giant` | outer system, massive | no |
| `ice_giant` | outer system, volatile-rich | no |
| `barren` is **not** a class | — | `barren` is a *derived label* of a rocky body |
| `artificial` | profile-gated | usually yes |
| `belt` | resonance/frost-line slots | no (rendered as instances) |

Note the important correction: "barren", "volcanic", "desert world" are **derived labels**
computed from the physical axes after sampling — they are not classes and do not select a
generator. Only the bulk/composition structure selects a generator. See
[04](./04-physical-parameters.md).

### Moons

- Moons are generated as smaller bodies around planets, with their own descriptors.
- Moon count is sampled from the planet's mass, its Hill radius, and the system profile.
- **Tidal locking** is computed from moon distance and planet mass; it is common and is a
  feature (one face always facing the planet — great for sky composition and for asymmetric
  terrain heating).
- Moons mostly use the same generators as planets, at smaller scale. Most are single-biome by
  consequence, not by special case. See [07 — Biomes & Ecology](./07-biomes-and-ecology.md).
- **Tidal heating** from a close massive moon can keep a small body warm far from the star —
  a legitimate, physically motivated source of surprising worlds.

### Rings

- Rings are a probability biased by the **absence of inner moons** (gameplay/visual rationale:
  rings read better and are more stable without shepherds inside them), but not a hard
  dependency.
- Ring extent should respect the **Roche limit**.
- Rings are visible from the surface as a band in the sky — this is a major aesthetic payoff
  and should be treated as a first-class feature, not a decoration.
- Ring composition (ice, rock, dust) feeds the surface palette (lighting tint at night) and the
  descriptor.

---

## System POIs

Besides bodies, a system has points of interest between orbits: stations, derelicts,
anomalies, navigation hazards, and faction assets. These are placed at system bake time with
stable IDs and are rendered at system scale. Their placement should respect the narrative layer
but not the physical generation of any planet.

See [08 — Narrative Layer](./08-narrative-layer.md).

---

## System descriptor

The bake output for a system is a compact record containing:

- identity and seed chain
- profile and narrative tags
- star list (mass, class, luminosity, age, multiplicity config)
- habitable zone(s)
- body list (class, orbit elements, mass, radius, rotation) — each pointing to a body
  descriptor
- system POI list with stable IDs
- naming/culture reference

It is small (kilobytes) and is the unit saved when a system is discovered.

---

## Related

- [01 — World Model](./01-world-model.md) — hierarchy and seeds.
- [03 — The Planet Descriptor](./03-planet-descriptor.md) — per-body bake output.
- [04 — Physical Parameters](./04-physical-parameters.md) — the axes profiles bias.
- [08 — Narrative Layer](./08-narrative-layer.md) — profiles' narrative side.
- [11 — Determinism & Sampling](./11-determinism-and-sampling.md) — stratified sampling and RNG.
