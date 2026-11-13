# 18 — Collision & Edits

[09 — Surface Field Stack](./09-surface-field-stack.md) defines a **pure** surface query: it
describes an untouched world. Two things sit on top of it:

- **Collision** — how the player physically meets that world.
- **Edits** — how the world remembers what the player changed.

They share one rule: the generated field stays authoritative. Collision samples it; edits decorate
it. Neither is allowed to become a second source of terrain truth.

This document is the contract for both, plus the voxel/mesh boundary where they meet.

---

## The authority model

```text
final(p) = applyEdits( generate(p), editsNear(p) )

generate(p)   = descriptor + generatorVersion        (pure, from 09)
editsNear(p)  = saved EditRecords whose bounds contain p
final(p)      = what rendering, collision, and gameplay all consume
```

Consequences:

- Rendering and physics must read the **same** `final`. A visual mesh built from `generate` while
  collision reads `final` is a bug, not a tuning problem.
- Edits never mutate the descriptor.
- Edits never re-run classification. `biomeWeights` and `suiteWeights` are properties of the
  generated world; a dug pit is still in a forest.
- The descriptor remains the save format. Edits are a separate, sparse record stream keyed by
  body and chunk.

`DECIDED` — edits are a layered record stream applied after generation (extends D18).

---

## Collision

### The solid model

A surface body is a **star-shaped solid**: along any direction from the body centre, everything
below the surface is solid and everything above is empty.

```text
dir(p)           = p / |p|
surfaceRadius(p) = bulk.radius + height(dir(p))
isSolid(p)       = |p| <= surfaceRadius(p)
```

That is sufficient for the open surface and it is why heightfield collision is cheap and
seam-free: adjacent patches evaluate the same function, so they cannot crack apart.

Caves, overhangs, and player-built volumes are **not** star-shaped. They use a volume
representation (voxels) instead of the heightfield. Base solidity is therefore:

```text
baseSolid(p) = isSolid(p) && !caveCarve(p)
```

- `caveCarve` is the union of cave volumes for the body, sampled from their `cave:<id>` features
  (see [15 — Aquatic Environments](./15-aquatic-environments.md#strata)). It is non-zero only
  inside cave bounds, so the normal surface path never pays for it.
- Cave IDs and parameters are frozen at bake; the volume is sampled from them, not stored as a
  grid. `hydrosphere.floodedCaves` lists the caves that contain water.
- Runtime edits may add or remove volume, but the cave layout itself is descriptor data.

`DECIDED` — heightfield solid for the open surface, sparse volume for caves and edits.

### The query contract

```text
sampleCollision(p, lod) -> CollisionSample | null
raycast(origin, dir, maxDist, mask, lod) -> Hit | null
overlapAABB(bounds, mask, lod) -> bool
```

```text
CollisionSample {
  position   : vec3        // body-local metres
  normal     : vec3        // unit, outward
  solid      : bool        // false inside a carved or edited void
  materialID : contentID   // physics material, not the render blend
  medium     : vacuum | air | water | ice | lava | ...
  water      : WaterSample | null
  flags      : { submerged, shoreline, ice, edited }
}

Hit {
  t          : float       // distance along dir
  point      : vec3
  normal     : vec3
  materialID : contentID
  medium     : string
  kind       : solid | water | trigger
}
```

`mask` selects which kinds a query reports. Gameplay asks different questions:

- "what does this bullet hit?" → `solid | trigger`
- "where is the waterline?" → `water`
- "can I stand here?" → `solid` with `solid == true`

A body with no solid surface (gas giant, ice giant) returns `null` from `sampleCollision` and
never reports a `solid` hit. Callers must handle `null`; see
[13 — Generator Modules](./13-generator-modules.md#the-contract).

### Collision LOD

Collision is the same query at a coarser LOD, but the relationship to the visual LOD is
constrained:

| Situation | Collision LOD | Why |
|---|---|---|
| Within `interactionRadius` of an active physics body (player, vehicle, projectile) | equal to the visual LOD | the player can see and touch the same surface |
| Between `interactionRadius` and `collisionRadius` | at most one LOD coarser | prevents falling through visible geometry |
| Beyond `collisionRadius` | none | nothing can reach it |
| No solid surface | none | nothing to collide with |

- `interactionRadius` and `collisionRadius` are per-body tuning values, not constants.
- Collision tolerance is `tol = max(0.02 m, 0.5 · sampleSpacing(lod))`.
- A collision patch is never generated **finer** than the visual patch it belongs to: that would
  cost more than the frame can show.

`DECIDED` — collision LOD is derived from the visual LOD and reachability, never independent.

### Raycasting a star-shaped solid

Because the solid is star-shaped and the height field is band-limited, a conservative sphere trace
is valid:

```text
g(p)   = |p| − surfaceRadius(dir(p))   // > 0 above the surface, < 0 inside
slope  = localSlopeBound(p, lod)       // |∇h| upper bound
step   = clamp(g(p) / (1 + slope), minStep, maxStep)
```

- Step by `step` while `g > tol`. The final segment may stop short of the surface: extend it in
  small steps until `g` changes sign, then bisect between those two points. A bisection whose
  endpoints have the same sign silently returns a point `tol` short of the ground.
- `slope` may be estimated from the local samples or bounded by the generator; either way it must
  be conservative, or the trace can step through a ridge.
- Stop after a fixed iteration cap. If the cap is hit, fall back to the macro pyramid and record a
  counter in the inspector. Silent failure is a bug.
- `maxStep` must be smaller than the thinnest solid feature at that LOD, derived from
  `feature_scale` (see [16](./16-data-contracts.md#lod-and-feature-metadata)). Otherwise fast
  projectiles tunnel through walls.
- For very fast movers, sweep the whole segment against the macro field and the voxel AABBs rather
  than relying on a long ray.

Water hits are analytic, not traced:

```text
t = solve(o + t·d = seaRadius)              // ocean
t = solve(o + t·d = lake.surfaceRadius)     // lake caps
```

Choose the smallest positive root. If the origin is already inside the water body, the nearest
water surface is behind the ray, so the query reports the exit point (or nothing, when the mask
asks only for entry hits). Never sphere-trace the shell.

### Physics materials

`materialID` resolves to a physics row — friction, restitution, density, hardness — from the
material content table in [17](./17-biome-and-landform-format.md). Physics reads that row, not the
renderer's material blend.

If a sample has a blend, the **dominant** `materialID` is authoritative for physics. Blending is a
visual convenience; it must never make two clients disagree about whether a slope is slippery.

### Water is not solid

This follows from D8 and D9: water is a level, not a fluid and not a mesh collider.

- The ocean is a **shell** at `seaRadius`; lakes are caps at their `surfaceRadius`.
- Water reports a `Hit` of kind `water` only when the mask requests it. It is never a `solid` hit.
- Buoyancy, swimming, and splash use `WaterSample` from the same query that renders the waterline.
- Waves are **visual displacement only**, clamped to `waveAmplitude`. They never enter
  `waterSurface`, `depth`, or buoyancy. The gameplay waterline stays level.

This is what keeps "water is a level" honest: the pretty surface and the physical surface are the
same plane, with cosmetic motion on top.

### Walking on a heightfield

- The character is a kinematic capsule moved by a slide query against the collision mesh at the
  visual LOD. Contact is kept by a **ground snap** — a downward shape cast, capped at 0.5 m — not
  by commanding downward velocity: a downward velocity projected onto a slope becomes a slide that
  never stops, even with no input.
- **With no input the requested motion is exactly zero and the movement solver is not run at all.**
  On walkable ground the character cannot move. (Letting the solver run with zero velocity still
  crept about 5 mm per 3 s of standing, from contact depenetration against the triangle mesh.)
- **Walkable** is a surface normal within 50° of the local up. Steeper ground gets no snap: the
  character falls or slides, deliberately.
- The capsule stands about 1 cm above the collision surface (the snap offset), inside the collision
  tolerance. Walking downhill faster than gravity can follow is what the snap is for.
- A collision LOD change under a standing character moves it only along the local up, never
  sideways.
- Colliders are added before the old ones are removed, and only a few per frame: rebuilding a block
  of collision patches in one frame cost 16–39 ms in both engines (E6).

### Seams and normals

- Heightfield collision is sampled, not meshed, so there is no crack to stitch.
- Voxel collision must overlap the mesh region by at least one voxel (see below).
- Normals use the same epsilon rule as [09](./09-surface-field-stack.md#normals-and-derivatives).
  A collision normal computed with a different epsilon than the visual normal will make the player
  visibly float or sink.

---

## Edits

### The record

```text
EditRecord {
  id                   : string          // stable; dedupe key
  world                : bodyID
  chunk                : chunkID | null  // null for cross-chunk edits
  domain               : height | volume | material | scatter | water
  op                   : string          // domain-specific
  sequence             : uint64          // per-author monotonic
  author               : string          // player, faction, authored
  createdAt            : float           // game time, not wall clock
  baseGeneratorVersion : uint32          // pinned when the first edit lands
  bounds               : sphere { center, radius } | aabb
  payload              : object
}
```

`bounds` is the broad-phase. It must contain **every** point the edit can affect, including
falloff. An undersized bound is a correctness bug, not an optimisation.

### Operations

| Domain | Ops | Payload |
|---|---|---|
| `height` | `raise`, `lower`, `flatten`, `smooth` | `delta`, `radius`, `falloff`, `target` |
| `volume` | `add`, `remove` | brush shape (sphere / box / capsule), occupancy |
| `material` | `paint` | `materialID`, `strength`, `radius` |
| `scatter` | `place`, `remove` | `presetID` + transform, or instance ID |
| `water` | `fill`, `drain` | `surfaceRadius`, local water-body ID |

Falloff curves are data (`smoothstep`, `linear`, `constant`) so authored and player edits share
one implementation.

Semantics by domain:

- **Height** edits: `raise` and `lower` are additive and commutative — `h += Σ delta_i` in sorted
  order. `flatten` and `smooth` are non-linear and therefore order-dependent; they are applied at
  their position in the total order. All height edits change `height`, `normal`, `altitude`, and
  `flags.edited`.
- **Volume** edits are **occupancy overrides**: inside their bounds, `isSolid` returns the override
  instead of `baseSolid`. Multiple overlapping overrides resolve last-writer-wins in sorted order.
- **Material** edits blend the material palette in sorted order. They do not change height.
- **Scatter** edits are add/remove sets keyed by instance ID, applied after chunk scatter is
  generated.
- **Water** edits change a **local** water body's surface. They never change the global
  `seaRadius`; that is frozen (D10).

`PROPOSED` — the exact payload schemas above. The domains and ordering rules are `DECIDED`.

### Ordering and idempotence

Records are applied in a total order:

```text
sort by (createdAt, author, sequence, id)
```

- The same record applied twice is a no-op. Dedupe by `id` **before** applying.
- `createdAt` is game time. For multiplayer, the server sequence is authoritative; clients never
  invent ordering.
- All accumulation uses the same fixed order on every machine, so replays and clients converge
  (see [11 — Determinism & Sampling](./11-determinism-and-sampling.md)).

This makes edits a deterministic function of the record set, not of arrival order.

### Application order

```text
editedSample(p) = applyEdits(sampleSurface(p, lod), editsNear(p))

applyEdits:
  1. height edits     // changes height, normal, altitude
  2. material edits   // changes material blend, not height
  3. water edits      // changes local WaterSample
  4. scatter cleanup  // per chunk, after all samples
```

Volume edits are not part of `editedSample`: they are consulted by `isSolid` and by collision, not
by the surface sample. This keeps the two representations separate:

- `height` describes the walkable surface where no volume override exists.
- `volume` describes solids and voids that the heightfield cannot represent.

Rules:

- Height edits do **not** re-run classification. `biomeWeights` / `suiteWeights` stay as generated.
- The `altitude` **axis** used by classification remains the pre-edit value; the `altitude` field
  returned in the edited sample is recomputed for UI/debug. This mirrors the pre-L3 classification
  rule in [09 — Surface Field Stack](./09-surface-field-stack.md).
- Water edits update `surface` and `depth`, but not the descriptor's `seaRadius`.

### Promotion and version pinning

This is the mechanism behind D26's "never regenerate under a player".

1. The first edit in a chunk records `baseGeneratorVersion`.
2. That chunk is generated with the pinned version, or migrated explicitly.
3. If a generator change would move the terrain under the edit, the chunk's base height is
   **promoted to frozen**: a sparse base cache is stored instead of regenerated.
4. Promotion is per chunk and per referenced feature. It is not a whole-planet freeze.

An `EditRecord` is therefore valid only against the base it was created on. Loading without the
pinned version is a migration event, not a silent reinterpretation.

### Persistence and garbage collection

- Edit records persist permanently. They are small.
- Derived data (meshes, collision patches, voxel meshes) is evicted with the chunk.
- **Height and material edits** are cheap to keep and cheap to replay.
- **Volume edits persist as records, not as voxels.** `DECIDED` from
  [E7](./spikes/E7-voxels-and-edits.md#edit-records-storage-replay-garbage-collection): a record is
  ~66 bytes (66 KB per 1 000 edits) at any voxel size; the same 525 edits stored as changed voxels
  cost ~10× more at 0.5 m. Replaying the records over the base reproduces the edited region bit
  for bit, at ~0.1 ms per record.
- **Garbage collection.** A record that changes nothing when applied is dropped at creation. A
  chunk's records are compacted into a changed-voxel delta (sparse indices + quantised values)
  only when they cost more than that delta, in bytes or replay time: the superseded-edit case,
  where many records rewrite the same voxels. `PROPOSED` — the exact thresholds; part of Q12 in
  [14 — Open Questions & Roadmap](./14-open-questions-and-roadmap.md).

### Large-scale changes

- **Draining a lake** is a `water` edit on that lake, not a re-bake and not a new sea level.
- **Mining a mountain** is a sequence of volume edits. The save grows with the removed volume;
  this is the Q12 problem, and it is deliberately not solved by regenerating terrain.
- **Destroying a structure** is a scatter/volume edit on the structure's IDs, not a terrain edit.
- **Global terraforming** (raising the sea, changing the atmosphere) is out of scope for edits.
  It would be a new baked descriptor or a narrative event, not a per-chunk record.

---

## The voxel/mesh boundary

The strategy is decided in [10 — LOD & Streaming](./10-lod-and-streaming.md) (D27); this is the
contract that makes it seam-free.

- A voxel region covers an explicit box. Inside it, the signed distance `g` (negative inside) is
  sampled at voxel corners and meshed with **surface nets**.
- Outside it, the mesh samples the same `isSolid`.
- The voxel region extends **at least one voxel beyond** the mesh boundary, so the two overlap.
  Do not rely on exact vertex matching; overlap is what hides the cut.
- `voxelSize ≤ min(feature_scale) / 2` for every feature active in the region. Features that fail
  the Nyquist test are band-limited out at LOD 4, exactly as in [09](./09-surface-field-stack.md).
- Seam tolerance is `0.5 · voxelSize`. A larger visible step is a bug, not a LOD artefact.
- The voxel region stays resident while the player is within `interactionRadius`, or while a base,
  edit, or building inside it needs to persist.
- **Non-radial solidity must be enclosed by a voxel region.** An arch, a cave mouth, or an overhang
  cannot cross the region boundary, because the heightfield side can only continue a single-valued
  (radial) surface; a tunnel reaching the box edge would be visibly capped by the heightfield. This
  is a content and layout rule, not a meshing bug (see I-010).

`DECIDED` — the numbers, from [E7](./spikes/E7-voxels-and-edits.md) (D44):

| | Value | Why |
|---|---|---|
| Voxel size | **0.5 m** | coarsest size that passed the user's look test from 2 m; 1 m reads as broken terrain |
| Region | **±64 m** around the player (128 × 128 m, band from 8 m below the ground to 2 m above): the extent edits can reach, built only where edited | 17 MB samples + 3 MB mesh if fully built; ±128 m fits in RAM but takes 15 s to build single-threaded |
| Storage | bricks of 32³ f32 samples (128 KB), one per chunk; samples are global, stored once | |
| Seam | max 20 mm measured against 250 mm tolerance | |
| Remesh | ~2.8 ms per chunk; a dig dirties 8–12 chunks, 23–31 ms single-threaded (limit 50 ms) | |

Rules that came out of E7:

- **Chunk ownership** (surface nets). A chunk owns the lattice edges whose start node is local
  `0..32` along the edge's own axis and `1..=32` on the other two. Each owned edge's quad joins the
  four cells around it, which lie in local `0..=32`, so a chunk computes vertices for `33³` cells
  from `34³` nodes: its own samples plus two apron layers read from the neighbouring brick (or the
  field, if that brick is not resident). Owning only quads whose cells are all local leaves a crack
  along every chunk boundary.
- **Winding comes from the voxel signs**: a quad faces from the solid end of its edge to the empty
  end. Averaged vertex normals can disagree at a sharp rim, and the face is then culled.
- **An edit never drops samples.** If it reaches a brick inside the region that is not resident,
  that brick is filled from the field first.
- **An edit dirties every chunk that reads the samples it changed**, including the neighbours below
  on each axis whose apron covers them (local indices 0 and 1).
- **Voxels exist only where edited** (`DECIDED`, I-014): a chunk switches from heightfield to
  voxels on its first edit, with the neighbours that read its samples; see
  [10](./10-lod-and-streaming.md#voxels-vs-heightfield).

---

## Authoring versus player edits

- Deterministic world features (a crashed ship placed by the seed, a hand-shaped valley baked into
  the descriptor) are content, not edits.
- Modifications that must layer over generated terrain can ship as **authored edit records** with
  `author: "authored"`.
- Authored records sort before player records because their `createdAt` precedes the save's epoch.
  They are frozen and cannot be removed by the player unless the content says so.
- `OPEN` — Q10 in [14](./14-open-questions-and-roadmap.md) covers how authored records merge with
  generated and player content.

---

## Invariants

These must hold for every body, at every LOD, with any edit set:

1. **One final surface.** Rendering, collision, and gameplay read the same `editedSample` and
   `isSolid`; no path regenerates terrain independently.
2. **Waterline agreement.** `WaterSample.surface`, the rendered shell, and buoyancy agree within
   tolerance; waves never move the gameplay waterline.
3. **Collision LOD is explicit.** No patch collides finer than it renders inside
   `interactionRadius`, and nothing collides beyond `collisionRadius`.
4. **No tunnelling.** `maxStep` is below the thinnest solid feature, or a swept test is used.
5. **Edits are idempotent and total-ordered.** Applying the same record twice is a no-op; the
   record set has one deterministic order.
6. **Edits never mutate the descriptor and never re-run classification.**
7. **Edited chunks pin their base.** `baseGeneratorVersion` is recorded, and promotion freezes the
   base if regeneration would move it.
8. **Voxel agreement.** Voxel solidity equals `baseSolid` outside edit bounds.
9. **Volume overrides are bounded.** A volume edit changes solidity only inside its `bounds`.
10. **Authored and player edits share one type and one order.**
11. **No movement without input.** A character standing on walkable ground does not move, not even
    by contact depenetration. Contact is kept by a ground snap, never by a commanded downward
    velocity.

A violation is a bug in collision, edits, or content — not a tuning problem.

---

## Worked example

A player digs a pit on a beach. The body has `bulk.radius = 24 000 m`, `seaRadius = 24 010 m`
(sea level at `height = +10 m`). The sample point is at `height = +12 m`.

| Step | Result |
|---|---|
| `sampleSurface` | height `+12 m`, biome `beach`, material `sand`, dry |
| Height edit | `lower`, radius `3 m`, `delta −3 m`, smoothstep falloff |
| Edited height at centre | `+9 m` |
| `altitude` at centre | `−1 m` |
| `WaterSample.depth` at centre | `1 m` (the pit is below the waterline) |
| `flags.edited` | `true` |
| `biomeWeights` | unchanged: `beach` |
| `materialID` | unchanged: `sand` |
| Solid-only raycast down | hits the pit floor at `height +9 m` |
| Water-only raycast down | hits the water surface at `height +10 m` |

The pit floods because `depth = max(0, waterSurface − height)` and the edited height went below the
waterline. Nothing simulated the water; the level did the work.

Because water is a level (D8), any point below the local water surface is wet, whether or not a
channel connects it to the ocean. That is the intended simplification. A pit stays dry only if its
floor remains above the waterline; making it dry below sea level would require flow, which is out
of scope.

Now the player builds a small overhang one metre above the sand:

| Step | Result |
|---|---|
| Volume edit | `add`, box brush above the surface |
| `isSolid` inside the brush | `true`, overriding the heightfield |
| `isSolid` outside the brush | unchanged |
| Surface sample | unchanged height, `edited` flag set |
| Raycast down | hits the overhang first; a second ray from beneath it hits the sand |

The heightfield cannot represent the overhang; the volume override can. Both paths still agree
about the sand underneath.

---

## Related

- [01 — World Model](./01-world-model.md) — persistence and the one-terrain-function rule.
- [03 — The Planet Descriptor](./03-planet-descriptor.md) — frozen/derived data and version pinning.
- [05 — Climate & Hydrosphere](./05-climate-and-hydrosphere.md) — the water surface edits may change.
- [09 — Surface Field Stack](./09-surface-field-stack.md) — the pure query edits decorate.
- [10 — LOD & Streaming](./10-lod-and-streaming.md) — LOD budgets, voxel strategy, streaming.
- [11 — Determinism & Sampling](./11-determinism-and-sampling.md) — ordering, RNG, replay.
- [13 — Generator Modules](./13-generator-modules.md) — the collision methods a generator provides.
- [14 — Open Questions & Roadmap](./14-open-questions-and-roadmap.md) — Q2, Q10, Q12.
- [15 — Aquatic Environments](./15-aquatic-environments.md) — caves and the water column.
- [16 — Data Contracts](./16-data-contracts.md) — `feature_scale` and versioning.
- [17 — Biome & Landform Format](./17-biome-and-landform-format.md) — physics material rows.
