# 16 — Data Contracts

This document defines the rules that **every** schema in the project obeys: identifiers,
references, units, versioning, override semantics, LOD metadata, and validation.

It exists because there are two big schemas — the [planet descriptor](./03-planet-descriptor.md)
and the [content format](./17-biome-and-landform-format.md) — and they must agree on the same
foundations. Without a shared contract, they drift: IDs stop matching, units disagree, and
"extends" means something different in each file.

If a rule applies to more than one data file, it belongs here, not in the schema docs.

---

## Identifiers

### World entities

World entities are the things the game creates: systems, bodies, regions, POIs. Their IDs encode
their place in the hierarchy from [01 — World Model](./01-world-model.md):

```
body   = "gal:0/reg:1/sys:42/body:3"
moon   = "gal:0/reg:1/sys:42/body:3/moon:1"
system = "gal:0/reg:1/sys:42"
poi    = "gal:0/reg:1/sys:42/body:3/poi:7"
chunk  = "gal:0/reg:1/sys:42/body:3/chunk:1/8/3/5"
```

Rules:

- IDs are **stable for the lifetime of the universe seed**. A POI that has been discovered keeps
  its ID across regeneration.
- IDs are **not seeds**. The seed chain is derived separately (see
  [11 — Determinism & Sampling](./11-determinism-and-sampling.md)). Never derive one from the
  other.
- IDs are opaque strings to content and tools; only the owning subsystem parses them.
- Never reuse a retired world ID.

### Content

Content is authored data: biomes, suites, materials, palettes, scatter presets, zones, overlays,
ecotones, system profiles. Its IDs are namespaced:

```
<namespace>:<type>/<name>

core:biome/temperate_forest
core:suite/dune_seas
core:overlay/corrupted_forest
core:ecotone/forest_edge
core:profile/oasis
core:material/basalt
mod.example:biome/glass_forest
```

Rules:

- The default namespace is `core`. Inside a core data file, the namespace and type may be
  omitted: the table `[biome.temperate_forest]` declares `core:biome/temperate_forest`.
- Names are lowercase `snake_case`, ASCII, no spaces. `[A-Za-z0-9_]` plus `-`.
- A reference field may use shorthand **when the expected type is unambiguous**:
  `palette = "green_temperate"` means `core:palette/green_temperate`.
- Renaming a content ID is a breaking change. Declare an alias from the old ID to the new one in
  the pack manifest, and never reuse a retired ID for something else.
- IDs are unique across all loaded packs. Duplicate declarations are a load error.

### Versioned identity

Every descriptor and every content pack carries version information (see
[Versioning](#versioning) below). Identity is `(seed chain, schema version, generator version,
content version)`.

---

## References

- **Reference by ID, never inline.** A biome names a palette; it does not embed a palette.
- **Resolution order** is deterministic: the declaring pack's own entry first, then its declared
  dependencies in order, then `core`. A pack may deliberately shadow a dependency; two independent
  packs defining the same ID without a declared order is an error.
- **Type-checked.** A field that expects a suite ID cannot reference a biome. Validation runs at
  load, not at first use.
- **Dangling references are errors**, not warnings. A biome referencing a missing landform suite
  fails the build.
- **Cross-type collisions are impossible** because the type is part of the ID.

This is what makes content moddable without a linker: a mod can add a biome that references
`core:suite/dune_seas` and a new `mod.example:material/glass`, and both resolve predictably.

---

## Units and frames

Units are fixed at the schema boundary. Mixing them is the most common source of silent,
hard-to-debug errors, so they are stated once here and not re-litigated per document.

| Quantity | Unit in data | Notes |
|---|---|---|
| Length (radius, feature scale, altitude, depth) | metres | metres everywhere, including planet scale |
| Mass | kilograms, **game scale** | not necessarily physical; see below |
| Gravity | g (9.80665 m/s²) | the gameplay-authoritative value |
| Time | seconds (game time) | never wall-clock; see below |
| Temperature | kelvin | convert for UI |
| Pressure | pascals | |
| Angles in data | degrees | converted to radians at the code boundary |
| Angles in code | radians | |
| Colour | sRGB hex (`#rrggbb`) | converted to linear in the renderer |
| Coordinates | body-local metres | origin at the body centre |
| Orbital elements | system frame, game units | conversion documented in the system descriptor |

### Mass is a consistency parameter

Planets are not physically realistic (see [00 — Vision](./00-vision.md)). `mass` exists to make
orbits, spheres of influence, and tidal effects behave consistently. It is **not** a claim about
the real world and is not required to satisfy `g = GM/r²` at the tiny radii the game uses.
`gravity` is the gameplay-authoritative surface value; `density` is derived for display and
composition, not used as a physical constraint. Tune them independently and document any
deviation where it matters.

### Coordinate convention

Body-local space is right-handed with:

- **+Y = north pole**,
- **longitude 0 at +X**, increasing eastward,
- **+Z = 90° east** at the equator.

This is a convention, not a law of nature. What matters is that it is written down and used
everywhere. Latitude is `asin(y / |p|)`, longitude is `atan2(z, x)`.

### Vertical quantities

Two vertical quantities are distinguished, because content and geometry need different origins:

| Quantity | Definition | Used by |
|---|---|---|
| **Elevation** / `height` | `|p| − bulk.radius` (metres above the reference sphere) | rendering, collision, the surface query |
| **Altitude** | `height − seaSurface` (metres above sea level; negative underwater) | climate lapse, the `altitude` biome axis |

`seaRadius` and lake `surfaceRadius` are stored as absolute radii in the descriptor; the query
converts them to `height` once per body. Never compare a content `altitude` against a raw radius.

### Time

All durations are game seconds. An `epoch` is defined per save. Orbital phase is computed from
`(gameTime − epoch) / period`. Changing the time scale must not change generated terrain, only
positions.

---

## Frozen, derived, and cached

This is the data-level encoding of the regeneration policy (decision **D26**). Every descriptor
field belongs to exactly one class:

| Class | Meaning | Persisted? | May change with a new generator? |
|---|---|---|---|
| **Frozen** | A coarse decision. Authoritative once baked. | Yes | No |
| **Derived** | Recomputed from frozen fields + generator. | No | Yes (subject to promotion) |
| **Cache** | Discardable recomputation of frozen/derived data. | Optional | Yes, freely |

### Rules

1. **Never regenerate a frozen field.** A new build reads the old value, even if it would now
   choose differently.
2. **Derived fields may change** — but only if nothing player-facing depends on them yet.
3. **Promote on reference.** If a derived field is referenced by a save, an edit, a base, a quest,
   or a tracked POI, it is written into the descriptor as frozen at that moment. This is the
   escape hatch that keeps saves valid without pinning the whole generator.
4. **Caches are never authoritative.** If a cache is missing or stale, rebuild it from frozen
   fields.

See [03 — The Planet Descriptor](./03-planet-descriptor.md) for which blocks are which.

---

## Versioning

Three numbers, three purposes:

| Field | Scope | Bump when… |
|---|---|---|
| `version` (descriptor) | schema shape | a field is renamed, removed, or retyped |
| `generatorVersion` | generation output | the same seed would produce different frozen/derived values |
| `contentVersion` | resolved content pack set | a biome/suite/material changes meaning or values |

### Change classes

| Class | Example | Action |
|---|---|---|
| **Additive** | new optional field, new content entry | No version bump required; old data still valid |
| **Semantic** | same shape, different meaning or thresholds | Bump `generatorVersion` (descriptor) or `contentVersion` (content) |
| **Breaking** | rename, remove, retype, change ID | Bump the schema `version`; provide migration or alias |

### Unknown fields

- **Authored content**: strict. Unknown keys are errors. A typo must not silently do nothing.
- **Descriptors**: tolerant on read. Unknown fields are preserved on rewrite so a save written
  by a newer build survives a round-trip through an older one.
- **Content packs newer than the code**: refuse to load unless the pack's `schema_version` is
  within the range the engine supports.

### Migration

Breaking changes ship with a migration function `old → new`. Migrations are pure, versioned, and
tested against golden descriptors. See [11](./11-determinism-and-sampling.md).

### Edits and version pinning

An edit is only meaningful against the base terrain it was created on. The first edit in a chunk
records `baseGeneratorVersion`; if a newer generator would move that base, the chunk's base is
promoted to frozen before migration. This is the data-level form of "never regenerate under a
player" (D26). See [18 — Collision & Edits](./18-collision-and-edits.md#promotion-and-version-pinning).

---

## Override and extension semantics

Content composes via `extends` and overlays. The semantics are deliberately simple so merges are
predictable.

### Single inheritance

`extends` names **exactly one** parent. Multiple inheritance makes field precedence ambiguous and
is not worth the expressive power.

```
[biome.alpine_forest]
extends = "temperate_forest"
```

Rules:

- Cycles are a load error.
- Depth is limited (suggest 8) to keep resolution cheap and readable.
- The child's ID is independent of the parent's; renaming the parent does not rename children.

### Merge rules

| Value kind | Default | Explicit variants |
|---|---|---|
| Scalar | child replaces parent | — |
| Map | shallow merge; child keys win | `{ _replace = true }` for a deep replace |
| List | **replace** | `scatter_add`, `scatter_remove` (and equivalents) |
| `null` | clears to the type default | — |

Lists replace by default because "append" is the surprising behaviour when a designer edits a
base entry. Appending and removing are explicit sibling keys (`scatter_add`, `scatter_remove`,
and equivalents for other list fields); they never appear inside the list itself.

### Precedence

Applied in order, later wins per field:

```
base entry  →  variants  →  affinity overlays  →  authored instance overrides
```

An overlay may target one entry (`target = "temperate_forest"`) or a tag/group. Overlays never
mutate the base entry; they are resolved at bake time, and the descriptor records the applied
overlay IDs (`appliedOverlays`) so later content edits cannot change a discovered world. Authored
instance overrides (`authoredOverrides`, see [08](./08-narrative-layer.md)) come last.

### Mods and patches

- Mods declare a namespace and may add new IDs freely.
- Overriding a core ID requires an explicit `patch` entry with a declared load order. Silent
  overrides are not allowed.
- All load order is deterministic and recorded in the save's content manifest.

---

## LOD and feature metadata

Every entry that produces geometry or detail declares the scale at which it operates:

| Field | Meaning | Required |
|---|---|---|
| `feature_scale` | Smallest feature the entry produces, in metres | Yes for suites and biomes; scatter defaults to `scale[0]` |
| `min_lod` / `max_lod` | LOD range in which it is evaluated; `min_lod` = coarsest, `max_lod` = finest | No; `max_lod` defaults to `4`, `min_lod` derives from `feature_scale` |
| `cull_distance` | Distance at which scatter is dropped entirely | No; per scatter rule |

Biome weights are evaluated at every LOD. `min_lod` / `max_lod` constrain where a suite or
scatter rule contributes geometry, so the same content can appear or disappear with distance
without changing the classifier.

Why this is mandatory:

- **Band-limiting.** A suite that makes 2 m features must not be evaluated at a LOD whose sample
  spacing is 40 m. Without `feature_scale`, the streaming layer cannot enforce this.
- **Voxel budget.** The voxel/mesh boundary (Q2) is not settled. `feature_scale` is how content
  declares its minimum resolution, so the boundary can be chosen *after* the spike without
  rewriting content.
- **Validation.** Once the voxel size exists, the validator warns when `feature_scale` is below
  it.

See [09 — Surface Field Stack](./09-surface-field-stack.md) and
[10 — LOD & Streaming](./10-lod-and-streaming.md).

### Deriving LOD bands from feature scale

LOD numbers increase with closeness: `0` is interplanetary, `4` is near the player (see
[10](./10-lod-and-streaming.md#lod-numbering-and-content-windows)). `min_lod` / `max_lod` are
inclusive bounds in that scale.

The default `min_lod` is the **coarsest LOD that can represent the feature without aliasing**:

```text
resolveLod(feature_scale) = smallest band b such that spacing(b) <= feature_scale / 2
min_lod = resolveLod(feature_scale)      // clamped to 4 if the feature is finer than band 4
max_lod = 4                              // full detail is always allowed near the player
```

`spacing(b)` is the band's maximum sample spacing from
[10](./10-lod-and-streaming.md#lod-levels-and-budgets). Bands are **body-independent**, so a
content entry resolves to the same window on every body; the per-body quadtree depth that reaches
each band is a streaming concern (see
[10](./10-lod-and-streaming.md#quadtree-level-versus-lod-band)).

| LOD band | Max sample spacing | Resolvable feature (`>= 2 × spacing`) |
|---|---|---|
| 0 | ~4 km | ≥ ~8 km |
| 1 | ~1 km | ≥ ~2 km |
| 2 | ~50 m | ≥ ~100 m |
| 3 | ~4 m | ≥ ~8 m |
| 4 | ~0.5 m | ≥ ~1 m |

Rules:

- A feature smaller than `2 × spacing(4)` is **near-field only**: `min_lod = max_lod = 4`, with the
  sub-LOD detail handled by LOD 4 micro detail or voxels.
- `max_lod < 4` is a deliberate budget cut. It is safe only if the entry's `feature_scale` is at
  least `2 × spacing(max_lod)`; the validator warns otherwise.
- `min_lod` may be **lowered** (coarser) than the default when the entry has a coarse component as
  well as a fine one. The dune suite in
  [17](./17-biome-and-landform-format.md#landform-suite-schema) is the canonical example: its 4 m
  ripples need LOD 4, but its 120 m dune field is still meaningful at LOD 2.
- Biome **weights** ignore `min_lod` / `max_lod`; they are evaluated at every LOD. The bounds only
  gate geometry and scatter.
- **Scatter is exempt from the Nyquist default.** Scatter instances are meshes placed on the
  surface, not heightfield samples, so their size does not need to be resolvable by the grid. A
  scatter rule's default `min_lod` is the coarsest band whose patches fall inside the rule's
  `cull_distance`; `feature_scale` only selects the instance's mesh LOD. A 2 m tree with
  `min_lod = 3` is therefore valid even though band 3 cannot resolve 2 m *terrain*.

---

## Validation

Validation runs in three places, with different strictness:

| Stage | Strictness | Catches |
|---|---|---|
| **Authoring / CI** | Strict | unknown keys, missing refs, bad enums, cycles, non-normalised weights, `feature_scale` below voxel size, duplicate IDs |
| **Runtime content load** | Strict | the same, plus incompatible pack versions |
| **Runtime descriptor read** | Tolerant | unknown fields preserved; missing required fields are errors |

Minimum checks:

1. Schema shape and types.
2. All references exist and have the expected type.
3. Probability/weight tables are non-negative and normalise (or are declared raw).
4. `extends` has no cycles and stays within the depth limit.
5. `min_lod <= max_lod`.
6. `feature_scale > 0`.
7. IDs are unique.
8. Enum values are from the declared set.
9. Overlay targets exist.

CI must run the full validator on every content pack and on the golden seed set. A content change
that breaks validation does not merge.

---

## File layout and packs

```
data/
  core/
    manifest.toml
    biomes/*.toml
    suites/*.toml
    materials/*.toml
    palettes/*.toml
    profiles/*.toml
    scatter/*.toml
    zones/*.toml
    overlays/*.toml
    ecotones/*.toml
    classification.toml   # tuning constants for 19 (sharpness, budget, jitter, …)
  mods/
    <mod>/
      manifest.toml
      ...
```

A manifest declares:

```toml
id             = "core"
version        = "1.4.0"   # pack version; a descriptor records the resolved set
schema_version = 1
dependencies   = []
aliases        = {}        # old content ID -> current content ID
```

The manifest's `version` is the **pack** version. It is not the descriptor schema version; the
set of loaded pack versions is what a descriptor records in `contentVersion`. The engine declares
the schema range it supports, and a pack whose `schema_version` falls outside it fails to load.

Rules:

- One manifest per pack.
- Files are loaded in deterministic order (manifest-declared, then path-sorted).
- A pack may not depend on a pack that depends on it.
- The save records the resolved manifest set so a world can be reproduced.

---

## Related

- [03 — The Planet Descriptor](./03-planet-descriptor.md) — the world-data schema.
- [17 — Biome & Landform Format](./17-biome-and-landform-format.md) — the content schema.
- [11 — Determinism & Sampling](./11-determinism-and-sampling.md) — seeds, golden tests.
- [12 — Authoring & Tuning](./12-authoring-and-tuning.md) — workflow and tooling.
- [14 — Open Questions & Roadmap](./14-open-questions-and-roadmap.md) — Q1 and Q2 status.
