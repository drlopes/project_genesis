# 03 — The Planet Descriptor

The descriptor is the most important artifact in the architecture. Everything the game knows
about a world that is not geometry lives here. If you are adding a new property to planets,
this is where it goes.

---

## The two phases

```
                    BAKE (once per body, deterministic)
seed chain  ──────────────────────────────────────────────►  Descriptor
                                                              (a few KB, JSON-like)
                                                                    │
                                                                    │  QUERY (per chunk, pure, any order)
                                                                    ▼
                                          (descriptor, position, LOD) ──► Height / Material / Climate / Scatter
```

### Bake

Runs the first time a body is encountered (entering the system, or loading a save that
references it). It takes the body's seed chain and produces a **descriptor**: all coarse,
discrete decisions about the world. Bake is allowed to be relatively expensive (tens of
milliseconds) because it happens once and can be amortised.

Bake must be:

- **Deterministic** — same seed chain and generator version ⇒ byte-identical descriptor.
- **Order-independent** — it must not depend on what else has been generated.
- **Self-contained** — the descriptor plus the generator version must be sufficient to
  reproduce the surface; no reference to other bodies except by ID.

### Query

Runs constantly, per chunk, per frame. It takes the descriptor, a position, and an LOD, and
returns samples. There are two sample types: **surface** (height, material, biome weights) and
**volume** (water-column zone, light, current, scatter, hazards). It must be:

- **Pure** — no side effects, no dependence on call order.
- **Stateless** — no accumulated history between calls.
- **Band-limited** — it must not return detail above the requested LOD (see
  [09 — Surface Field Stack](./09-surface-field-stack.md)).

Query never mutates the descriptor. Player modifications are applied as separate **edits**
after generation.

---

## Why this split exists

| Problem | Solved by |
|---|---|
| Save files containing gigabytes of terrain | Save descriptors (KB) + player edits only |
| Memory for thousands of planets | Descriptors are cheap; geometry is transient |
| Chunks requested out of order while flying | Query is pure and positional; no RNG stream ordering |
| Multiplayer / shared seeds | Descriptor is a deterministic function of a seed string |
| Changing a shader invalidates a world | Materials are data in the descriptor; only generator changes matter |
| Debugging "why does this planet look like that" | Inspect the descriptor, not the mesh |
| Modding and data-driven content | Descriptors reference data-defined biome/landform/material IDs |

---

## Schema layers

The descriptor is a layered record, not a flat bag. The layers matter because they have different
change policies.

| Layer | Contents | Change policy |
|---|---|---|
| Identity & core | `id`, `seedChain`, versions, `generator` | additive only |
| Physical | orbit, bulk, rotation, energy, atmosphere, hydrosphere, geology, climate | frozen; semantic changes bump `generatorVersion` |
| Content references | regions, biome/landform/material IDs, biosphere, aquatic | frozen; validated against [17](./17-biome-and-landform-format.md) |
| Narrative | affinity, anomalies, POIs, naming | frozen |
| Derived | `derivedLabels`, summaries | recomputed; promote on reference |
| Caches | macro pyramid | discardable |

Adding a field inside a layer is non-breaking. Renaming or removing one is a schema break. See
[Schema evolution](#schema-evolution).

---

## Schema

The exact format is an implementation detail (JSON, MessagePack, binary struct). The important
part is the **shape**. A representative schema:

```jsonc
{
  "version": 1,                  // descriptor schema version
  "generator": "rocky",          // body class / generator module key
  "generatorVersion": 7,         // bump when generation output changes
  "contentVersion": "core@1.4.0",// resolved content pack set at bake

  "id": "gal:0/reg:1/sys:42/body:3",
  "seedChain": { "galaxy": "...", "region": "...", "system": "...", "body": "..." },

  "orbit": {
    "parent": "sys:42/star:0",   // or barycentre
    "semiMajorAxis": 0.42,       // game units
    "eccentricity": 0.03,
    "inclination": 1.2,          // degrees
    "argumentOfPeriapsis": 12.0,
    "meanAnomalyAtEpoch": 88.0,
    "period": 3600.0,            // seconds of game time
    "tidallyLocked": false
  },

  "bulk": {
    "radius": 24000.0,           // metres; reference sphere for elevation
    "mass": 3.1e21,              // kg, game scale (orbits/tides; not physical at this radius)
    "gravity": 0.36,             // g at surface; the gameplay-authoritative value
  },

  "rotation": {
    "dayLength": 512.0,          // seconds of game time
    "obliquity": 18.0,           // degrees
    "tidalLock": { "locked": false }
  },

  "energy": {
    "insolation": 0.87,          // relative to Earth
    "albedo": 0.31,
    "greenhouse": 14.0,          // K offset from Earth's baseline (see 05)
    "internalHeat": 0.4,         // 0..1 normalized
    "tidalHeat": 0.0
  },

  "atmosphere": {
    "present": true,
    "surfacePressure": 61000.0,  // Pa
    "composition": { "n2": 0.78, "o2": 0.21, "ar": 0.01 },
    "scaleHeight": 8500.0,
    "hazeColor": "#9db4c0",
    "magnetosphere": 0.7
  },

  "hydrosphere": {
    "waterInventory": 0.62,      // fraction of surface able to hold water
    "oceanFraction": 0.48,       // target fraction covered by water
    "state": "liquid",           // liquid | ice | mixed | vapor | none
    "waterBody": "marine",       // marine | freshwater | brine | hydrocarbon | ammonia | subsurface
    "seaRadius": 24040.0,        // absolute radius of sea level (metres from centre)
    "salinity": 0.03,
    "lakes": [                   // from bake-time depression filling
      { "id": "L0", "surfaceRadius": 24062.0, "bounds": [/* spherical cap */], "outflow": null }
    ],
    "floodedCaves": ["cave:14"]
  },

  "climate": {
    "meanSurfaceTemp": 289.0,    // K
    "tempBands": ["temperate"],  // derived labels for UI
    "axes": { "temperature": 0.62, "moisture": 0.48 }, // summary of the continuous fields
    "tidalLockClimate": null     // populated when tidally locked
  },

  "geology": {
    "plateCount": 7,
    "tectonicActivity": 0.55,
    "volcanism": 0.3,
    "craterAgeProvinces": [ { "age": 0.2, "coverage": 0.4 }, { "age": 0.9, "coverage": 0.6 } ],
    "erosion": { "aeolian": 0.4, "fluvial": 0.2, "glacial": 0.0, "massWasting": 0.3 },
    "macroPyramid": "res://gen/42/body3_macro.bin" // optional cached low-res grid
  },

  "regions": {
    "ecological": { "count": 5, "dominant": "temperate_forest" },
    "geological": [ "cratered_highlands", "smooth_maria", "rifted_plains" ],
    "material":   [ "basalt", "iron_oxide", "sedimentary" ],
    "narrative":  []
  },

  "biomePalette": [
    { "id": "temperate_forest", "weight": 0.42 },
    { "id": "temperate_grassland", "weight": 0.28 },
    { "id": "boreal_forest", "weight": 0.18 },
    { "id": "alpine", "weight": 0.12 }
  ],
  "landformSuites": [
    { "id": "rolling_hills", "params": { "amplitude": 40.0, "wavelength": 900.0 } },
    { "id": "dune_seas",     "params": { "orientation": [0.3, 0.0, 1.0],
                                         "dune_height": 8.0, "dune_spacing": 120.0,
                                         "asymmetry": 0.7, "dust_cover": 0.4 } }   // fully resolved
  ],

  "materials": {
    "surface": "regolith_basaltic",
    "subsurface": "basalt",
    "liquid": "water",
    "accent": "iron_oxide"
  },

  "biosphere": {
    "level": "complex",          // none | microbial | complex | exotic
    "density": 0.7,
    "palette": "carbon_oxygen"
  },

  "aquatic": {                   // see docs/15-aquatic-environments.md
    "photicDepth": 140.0,        // metres; light drops below ~1% of surface
    "maxDepth": 5400.0,
    "turbidity": 0.35,
    "currentField": { "prevailing": [0.4, 0.1, 0.9], "tidalAmplitude": 0.15 },
    "iceCover": 0.0,             // 0..1 seasonal/permanent surface ice
    "iceShellThickness": 0.0,    // > 0 for subglacial oceans
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
    "ventSites": [], "reefSites": [], "brineLayers": [], "specialVolumes": []
  },

  "affinity": "corrupted",       // narrative overlay: mechanical | infected | corrupted | wild | gaia | ...
  "appliedOverlays": ["corrupted_forest", "corrupted_sky"], // resolved at bake
  "anomalies": [],

  "derivedLabels": ["temperate", "ocean_world", "habitable", "ringed"],
  "pois": [
    { "id": "poi:0", "type": "ruin", "position": [ /* body-local */ ], "poiSeed": "..." }
  ],

  "naming": { "culture": "sol_common", "designation": "Kestrel IV", "localName": null },

  "contentFlags": { "hasRuins": false, "contested": false, "hazardLevel": 2 }
}
```

Notes on the schema:

- **Everything discrete lives here.** If a decision is made once and reused, it belongs in the
  descriptor, not in a per-chunk query.
- **Continuous fields are summarised, not stored.** Temperature and moisture are functions
  evaluated from parameters; only their axes/summary and derived labels are stored. The macro
  pyramid is an optional cache, not the source of truth.
- **IDs, not references.** `landformSuites`, `biomePalette`, and `materials` reference
  data-defined entries (see [17](./17-biome-and-landform-format.md)). Bake stores the chosen IDs
  and their sampled parameters; for suites the per-body `params` are what make two worlds using
  the same suite look different (see [06](./06-geology-and-landforms.md#suite-parameters)).
  Within a typed block the shorthand ID (`temperate_forest`) is sufficient; the block name
  supplies the type (see [16](./16-data-contracts.md#references)).
- **Labels are outputs.** `derivedLabels` are computed during bake from the physical axes and
  are used for UI, weighting, and dispatch. They are never read back into generation. See
  [04](./04-physical-parameters.md).
- **Surface and volume.** The descriptor feeds two query types: the surface stack
  (height/material) and the volume query (water column). The `aquatic` block is what makes the
  volume query possible. See [15](./15-aquatic-environments.md).
- **Narrative resolution.** `affinity` selects overlays; `appliedOverlays` records the resolved
  IDs so later content edits cannot change a discovered world. See
  [08](./08-narrative-layer.md) and [17](./17-biome-and-landform-format.md#overlays).
- **Versioning is mandatory.** See below.

---

## Units and frames

All quantities use the units fixed in [16 — Data Contracts](./16-data-contracts.md): metres,
kilograms (game scale), seconds, kelvin, pascals, degrees at the data boundary, and body-local
metres for coordinates. Angles and colours are converted at the code boundary.

This is stated once, centrally, because a schema with mixed units silently corrupts every
downstream calculation. If a field needs a unit, it uses the contract's unit; there are no local
exceptions.

---

## Frozen, derived, and cached

Every block in the schema belongs to one of the three classes defined in
[16](./16-data-contracts.md#frozen-derived-and-cached). The split is the data-level form of the
regeneration policy (D26).

| Block | Class | Why |
|---|---|---|
| `id`, `seedChain`, `version`, `generator`, `generatorVersion`, `contentVersion` | frozen | identity |
| `orbit`, `bulk`, `rotation` | frozen | astronomical identity |
| `energy`, `atmosphere`, `hydrosphere`, `geology` (coarse), `climate` parameters | frozen | physical identity |
| `regions`, `biomePalette`, `landformSuites`, `materials`, `biosphere`, `aquatic` | frozen | content identity; POIs depend on them |
| `lakes` | frozen | computed at bake; referenced by gameplay |
| `affinity`, `appliedOverlays`, `anomalies`, `pois`, `naming` | frozen | narrative identity |
| `derivedLabels`, `tempBands`, `contentFlags`, summary statistics | derived | recomputed for UI and weighting |
| `macroPyramid` | cache | discardable |

Rules:

- Frozen fields are persisted and never regenerated.
- Derived fields are recomputed, but are **promoted to frozen** if a save, edit, base, quest, or
  tracked POI references them.
- Caches are rebuilt on demand and never authoritative.

---

## Schema evolution

Changes fall into three classes (see [16](./16-data-contracts.md#versioning)):

| Class | Example | Action |
|---|---|---|
| Additive | new optional field | no schema bump |
| Semantic | same fields, different thresholds or meaning | bump `generatorVersion` |
| Breaking | rename, remove, retype | bump `version` + migration |

Descriptor readers are tolerant of unknown fields and preserve them on rewrite; content readers
are strict. That asymmetry is deliberate: saves must survive a newer build, content must not hide
typos.

Semantic changes to **generation parameters** bump `generatorVersion`; changes to **content
meaning** bump `contentVersion`. The schema version only moves for shape changes.

---

## Versioning and regeneration

Three version numbers matter:

- `version` — the descriptor **schema** version. Changing field names or shapes requires
  migration.
- `generatorVersion` — bumped whenever generation **output** changes for the same seed. It does
  not change the schema, but an old save and a new build may describe the world differently.
- `contentVersion` — the content pack set the descriptor was baked against. Changing a biome's
  palette, a suite's parameters, or a material's response bumps it.

`DECIDED` — descriptors persist all three.

`DECIDED` — regeneration policy (D26):

1. **Freeze coarse decisions.** Orbit, bulk, biome palette, landform suite IDs, material palette,
   POIs, and naming are authoritative once baked.
2. **Migrate detail.** Fine terrain, micro-scatter, and other derived fields may improve with a
   new generator.
3. **Never regenerate under a player.** If a derived field is referenced by an edit, base, quest,
   or tracked POI, promote it to frozen at that moment.

This keeps saves valid without pinning the entire generator, and it lets the detail layer improve
between releases.

`OPEN` — the mechanics of migration: when to run it, how to detect affected worlds, and how to
present it to the player. The policy is settled; the procedure is Q1 in
[14 — Open Questions & Roadmap](./14-open-questions-and-roadmap.md).

---

## What does *not* belong in the descriptor

- Mesh data, heightmaps (beyond the optional macro pyramid cache), textures.
- Per-chunk edits (those are separate, keyed by chunk).
- Anything derivable from the seed in O(1) that is cheaper to recompute than to store.
- Live simulation state (positions, health, inventories of NPCs) — that is world state, not
  descriptor.

---

## Bake ordering (within a body)

Bake is a small pipeline. The order is a **dependency order**, not a preference: later phases read
the outputs of earlier ones.

1. **Bulk and rotation** — mass, radius, day length, obliquity, tidal lock.
2. **Energy budget** — insolation at the orbit, albedo, internal heat, tidal heat.
3. **Atmosphere** — retention from escape velocity and temperature; composition, pressure,
   greenhouse.
4. **Water inventory and state** — how much water exists, its state (liquid/ice/vapour), and the
   water-body type. **No sea level yet**: it depends on relief.
5. **Geology** — plate layout (coarse), activity, volcanism, crater provinces, erosion regime.
6. **Coarse relief** — evaluate the L0 macro field on a coarse grid. This produces the hypsometric
   input and the L0 samples the macro-pyramid cache is built from; it is the only heightfield the
   core needs.
7. **Hydrosphere completion** — solve `seaRadius` from `oceanFraction` and the hypsometric curve;
   fill depressions to find lakes. This reuses the same coarse heightfield. See
   [05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md#sea-level-from-hypsometry).
8. **Climate fields** — temperature and moisture parameters (full fields are evaluated at
   query time, but their controlling parameters are baked here).
9. **Region layers** — sample ecological/geological/material region parameters.
10. **Biosphere** — level and density, from climate and hydrosphere.
11. **Biome palette and landform suites** — chosen from the axes, region layers, and biosphere
    level, with suite parameters sampled per body.
12. **Aquatic layer** — water body type, photic depth, turbidity, currents, benthic biome
    palette, pelagic zones, vent/reef sites, ice cover. Depends on hydrosphere, climate, geology,
    and biosphere. See [15](./15-aquatic-environments.md).
13. **Narrative** — affinity, resolved overlays, anomalies, POIs, naming.
14. **Derived labels** — computed last, from everything above.

### Core versus caches

The macro pyramid is a **cache** (see [16](./16-data-contracts.md#frozen-derived-and-cached)),
not part of the frozen identity. Bake therefore has two outputs:

```text
bakeCore(seed, class, systemProfile, params, content) -> DescriptorCore
bakeCaches(core)                                      -> Caches
```

This split avoids the bootstrap cycle "the pyramid needs the descriptor, the descriptor needs the
pyramid": the core is built first, then the caches sample it. The descriptor hash covers the core
only. A missing cache is rebuilt from the core; it never changes identity. The water-distance field
is one of these caches: it is computed from the sea/lake mask and sampled by L1.

`DECIDED` — core and caches are separate outputs; only the core is hashed and saved as identity.

---

## Related

- [01 — World Model](./01-world-model.md) — identity and seed chain.
- [04 — Physical Parameters](./04-physical-parameters.md) — the values the descriptor stores.
- [09 — Surface Field Stack](./09-surface-field-stack.md) — how query uses the descriptor.
- [11 — Determinism & Sampling](./11-determinism-and-sampling.md) — how bake stays deterministic.
- [12 — Authoring & Tuning](./12-authoring-and-tuning.md) — the data IDs the descriptor references.
- [13 — Generator Modules](./13-generator-modules.md) — the `generator` field and its contract.
- [16 — Data Contracts](./16-data-contracts.md) — units, IDs, versioning, frozen/derived.
- [17 — Biome & Landform Format](./17-biome-and-landform-format.md) — the content schema.
