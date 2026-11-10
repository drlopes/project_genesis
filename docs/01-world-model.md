# 01 — World Model: Hierarchy, Scale & Coordinates

This document defines what exists in the world, how things are named and identified, and how
space is measured. It is the spatial foundation for every other document. If you are touching
LOD, physics, or persistence, read this first.

---

## Hierarchy

The world is a strict containment hierarchy. Each level owns its children and derives their
seeds.

```
Galaxy
└── Region / Sector            (spatial chunk of the galaxy, with faction & age overlays)
    └── Star System            (one or more stars + orbiting bodies)
        ├── Star(s)
        ├── Body               (planet, moon, asteroid belt marker, station)
        │   └── Surface Region (a named geographic area, e.g. a basin, highland, reef, or trench)
        │       └── Chunk      (cube-sphere quadtree patch at some LOD)
        │           └── Voxel  (leaf detail, only near the player)
        └── System POI         (stations, derelicts, anomalies between orbits)
```

Not every level is always materialised. Regions exist to organise galaxy-scale generation and
faction overlays; a human player may never see the boundary between two regions. Chunks and
voxels exist only while near the player.

### Identity and the seed chain

Every entity has a stable identity, and every random decision is derived from it:

```
galaxySeed
  └─ regionSeed   = H(galaxySeed,  "region", regionIndex)
      └─ systemSeed = H(regionSeed, "system", systemIndex)
          └─ bodySeed = H(systemSeed, "body", bodyIndex)
              ├─ orbitSeed       = H(bodySeed, "orbit")
              ├─ bulkSeed        = H(bodySeed, "bulk")
              ├─ geologySeed     = H(bodySeed, "geology")
              ├─ hydrosphereSeed = H(bodySeed, "hydrosphere")
              ├─ climateSeed     = H(bodySeed, "climate")
              ├─ contentSeed     = H(bodySeed, "content")
              ├─ chunkSeed       = H(bodySeed, "chunk", face, level, x, y)
              └─ …               // full registry in 11
```

`H` is a counter-based hash, not a stateful RNG stream. This is essential because chunks are
requested **out of order** as the player moves and LOD changes. The complete, append-only list of
purpose tags — and the rules for adding one — lives in
[11 — Determinism & Sampling](./11-determinism-and-sampling.md#the-seed-chain). It is the single
source of truth; this section is only the shape.

Identities are stable strings or 64-bit integers; they are what saves persist. A descriptor is
identified by its seed chain plus a generator version (see [03](./03-planet-descriptor.md)).

---

## Scale model

### Planet radius

Planets are **small**. A body the player can land on will typically be on the order of tens of
kilometres in radius, not thousands. This is not a compromise to be hidden — it is a design
constraint to be embraced:

- Horizon curvature is visible; the world feels like a small moon or asteroid rather than Earth.
- Gravity is low, which suits jetpacks, vehicles, and survival movement.
- Traversal times are playable; a "planet" is a place you can cross in a session, not a
  continent.

`DECIDED` — planets are small. Art direction, movement tuning, and survival mechanics should
assume it. Do not design content that requires Earth-scale continents.

### Interplanetary distance

Orbital distances are **compressed** relative to reality, for two reasons: large neighbours in
the sky look good, and travel time must fit a game session. Because flight is seamless, the
distance the player sees is the distance the player flies — there is no separate "skybox
distance" or conversion layer. Choose the orbital radii themselves to produce the desired
apparent size and travel time.

`DECIDED` — pick aesthetically chosen distances and use them for traversal. Distance is a
design dial: derive it from desired travel time at the ship's cruise speed.

### Travel time as a dial

The meaningful design number is **time to cross a system**, not kilometres. Pick that first
(e.g. minutes), then solve for orbital radii given the ship's speed. This keeps the game's
pacing stable even if the ship gets faster later.

`OPEN` — exact target travel times per transition (surface → orbit, planet → moon,
planet → planet, system → system).

---

## Coordinate spaces

Several spaces are in play. Mixing them up is a common source of precision bugs.

| Space | Origin | Used for | Precision |
|---|---|---|---|
| **Galaxy** | galaxy centre | region and system layout, map UI | 64-bit / double |
| **System** | system barycentre | orbital motion, interplanetary flight | 64-bit / double |
| **Body-local** | body centre | surface generation, terrain, physics near a body | 32-bit float OK if far from limb |
| **Surface-local** | a point on the body | camera, rendering, voxels, character physics | 32-bit float |
| **Camera-relative** | camera | rendering transforms ("floating origin") | single precision |

### Precision rules

- Never express surface positions in system coordinates with single-precision floats. At
  interplanetary distances the error is visible.
- Use a **floating origin** (or camera-relative rendering) so the renderer never handles
  coordinates far from zero. Physics and gameplay may keep world coordinates; rendering must
  not.
- When converting between spaces, always convert *once* at a well-defined boundary and keep the
  result in the local space.

`DECIDED` — floating origin (or equivalent) is required. Choose the exact scheme before writing
the first chunk loader; retrofitting it is painful. See
[10 — LOD & Streaming](./10-lod-and-streaming.md).

---

## Orbital model

### On rails, with local physics

Bodies move on **Keplerian rails** through the system. There is no n-body gravity between
bodies. When the player (or a simulation entity) is near a body, that body's local physics take
over inside its **sphere of influence**.

- Orbital elements are baked into the system descriptor: semi-major axis, eccentricity,
  inclination, argument of periapsis, mean anomaly at epoch, and period.
- Positions are computed analytically each frame from game time. No integration, no drift.
- A body's **sphere of influence** is derived from its mass and its orbit and is the boundary
  between system-scale and local-scale physics.
- Multiple stars: the system has a barycentre, and bodies may orbit the barycentre (P-type) or
  an individual star (S-type). See [02](./02-galaxy-and-systems.md).

This gives close, pretty orbits without requiring them to be dynamically stable. It also makes
time compression trivial.

`DECIDED` — on-rails Keplerian bodies; local physics inside the sphere of influence.

### Time

Game time drives orbital phase. Whether the game supports time acceleration while flying,
and how that interacts with survival needs and physics, is undecided.

`OPEN` — time compression policy.

---

## Frame transitions

The seamless transition is the defining feature, so it is worth naming the phases explicitly.
These are not separate scenes; they are regions of a continuum of LOD and physics.

```
Surface  →  Low Orbit  →  High Orbit  →  Interplanetary  →  Approach  →  Entry  →  Surface
   |            |              |                |               |          |         |
 local        local          body-local      system          body-local  blended  local
 physics      physics        rendering       rendering       rendering   atmosphere
```

Rules that hold across every transition:

1. **One terrain function.** Rendering, collision, and gameplay all sample the same query.
   Collision is not a separate mesh; it is the query at a coarser LOD.
2. **LOD, not scene load.** Transitioning changes which chunks are resident and at what LOD.
   It never swaps worlds.
3. **Continuity of appearance.** A body's colour, atmosphere, biome palette, and silhouette as
   seen from orbit are derived from the same descriptor that generates the surface. Landing
   does not reveal a different world.

Underwater follows the same rule: surface → dive → seafloor is a change in LOD and physics
frame, never a scene load. See [15 — Aquatic Environments](./15-aquatic-environments.md).

Details in [09](./09-surface-field-stack.md) and [10](./10-lod-and-streaming.md).

---

## Persistence

Save games store **descriptors and player state**, never terrain geometry.

- Player position is stored in body-local coordinates plus the body's identity (and, for
  interstellar state, the system and a body-relative frame).
- Persistent modifications (a base, a mined crater, a destroyed structure) are stored as
  **edits** keyed by body and chunk and applied after generation, so the procedural function
  remains the source of truth. See [18 — Collision & Edits](./18-collision-and-edits.md#edits).
- Descriptors carry a **generator version**. A world regenerated by a newer generator may
  differ; migration policy is `OPEN` (see [03](./03-planet-descriptor.md) and
  [11](./11-determinism-and-sampling.md)).

---

## Related

- [02 — Galaxy & Systems](./02-galaxy-and-systems.md) — how the hierarchy above is generated.
- [03 — The Planet Descriptor](./03-planet-descriptor.md) — identity and versioning.
- [10 — LOD & Streaming](./10-lod-and-streaming.md) — coordinates, chunks, and transitions in depth.
- [11 — Determinism & Sampling](./11-determinism-and-sampling.md) — the seed chain and RNG.
