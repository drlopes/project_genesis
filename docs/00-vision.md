# 00 — Vision & Pillars

## The game, in one sentence

A seamless space exploration and survival game in which the player can take off from one
planet's surface, fly into space, land on another planet, and dive to the bottom of its oceans —
all **without a loading screen** — across many star systems and galaxies. Surface and underwater
are peer environments; neither is a side activity.

## The design problem this folder solves

The setting is enormous. A single galaxy may contain thousands of star systems, and a single
system may contain a dozen landable bodies. If those worlds look and feel the same, the game
collapses into a palette swap. **Environment variation is the product.** Everything in these
documents is subordinate to that goal.

The naive approach — a linear list of terrain passes, driven by a flat enumeration of planet
types — produces contradiction-prone content and a combinatorial explosion of hand-authored
cases. This architecture replaces enumeration with **composition of independent axes**, and
replaces "generate everything up front" with **bake a descriptor, then query it at any LOD**.

---

## Pillars

### 1. Seamless continuity

There is one continuous world from the seafloor to interstellar space. No loading screens, no
scene swaps, no "enter atmosphere" button. This is the hardest constraint in the project and it
shapes almost every other decision:

- The same terrain function must serve rendering, collision, and physics, at different LODs —
  including underwater.
- Distances and coordinate precision must work at planetary and interplanetary scale.
- Atmosphere, ocean, seafloor, and sky must cross the boundaries without popping.

See [01 — World Model](./01-world-model.md) and [10 — LOD & Streaming](./10-lod-and-streaming.md).

### 2. Aesthetic-first, causality-second, realism-third

The goal is **not** an accurate space simulation. It is a game that looks good, is interesting,
and supports engaging gameplay. Planets may orbit closer than reality; moons may be implausibly
large in the sky; a system may be arranged purely because it frames beautifully.

But: **players forgive unrealistic scale instantly and inconsistencies never.** The causal
chain must stay coherent even when the constants are wrong:

- What you see from orbit must match what you find on the surface.
- A planet close to its star should be hot — unless something in the world explains why not
  (tidal locking, geothermal activity, greenhouse runaway, artificial origin).
- A huge moon implies tides. Either fake them plausibly or do not draw attention to them.

Rare physical contradictions are allowed when they are **content**: tag them as anomalies and
let lore explain them. See [08 — Narrative Layer](./08-narrative-layer.md).

### 3. Composition over enumeration

Physical, ecological, geological, and narrative properties are independent axes. Named
"planet types" are **derived labels** computed from those axes, not inputs that gate one
another. This is what makes thousands of worlds affordable to author and impossible to
contradict.

See [04 — Physical Parameters](./04-physical-parameters.md).

### 4. Determinism

The same seed produces the same world, on every machine, in every session, in any order of
exploration. Generation is a pure function of identity, never of call order. This is required
for saves, multiplayer, streaming, and debugging.

See [11 — Determinism & Sampling](./11-determinism-and-sampling.md).

### 5. Bounded cost at every distance

Every query has a cost appropriate to the LOD that requested it. A body seen from orbit must be
cheap; a body under the player's feet may be expensive. Detail is generated on demand and
cached, never simulated globally.

See [09 — Surface Field Stack](./09-surface-field-stack.md) and [10 — LOD & Streaming](./10-lod-and-streaming.md).

### 6. Contrast and rarity

Not every planet should be interesting. A featureless ice moon makes the next ocean world land
harder. Boring worlds are valuable as negative space — but they still need a reason to exist
(a POI, a resource, a view). Deliberate rarity tiers and occasional anomalies are what make
exploration feel like discovery rather than sampling.

See [08 — Narrative Layer](./08-narrative-layer.md) and [11 — Determinism & Sampling](./11-determinism-and-sampling.md).

### 7. Surface and underwater are peers

The ocean floor is a second exploration space, not a backdrop. Underwater gets the same depth of
generation, gameplay, and content as the surface: its own biomes, landforms, resources, hazards,
vehicles, bases, and reasons to return. A planet is not finished when its land is explored.

This is a scope commitment, not an aspiration. It shapes the descriptor (the `aquatic` block),
the field stack (the volume query), the LOD budget, and the content pipeline.

See [15 — Aquatic Environments](./15-aquatic-environments.md).

---

## Non-goals

Explicitly **not** goals, so that effort is not spent on them:

- Accurate astrophysics or n-body simulation. Bodies are on rails; local physics only near
  the player.
- Fluid simulation. Oceans and lakes are surfaces at a level; no flow, no flooding, no
  currents.
- Underwater as a shallow visual backdrop. It is a first-class environment (Pillar 7).
- Earth-scale planets. Voxel worlds are tens of kilometres in radius by necessity.
- Runtime tectonic or erosional simulation. These are baked coarsely, once, or approximated.
- Realistic orbital distances and travel times. Distances are design dials chosen for
  aesthetics and pacing.
- Photoreal rendering of every body. Stylised, readable, varied.

---

## Success criteria

The architecture is working when:

1. **The diversity audit is healthy.** A sample of 10,000 descriptors shows broad, stratified
   coverage of the parameter space — no giant clusters of near-identical worlds, no empty
   regions. (See [11](./11-determinism-and-sampling.md).)
2. **The eye test holds.** From orbit, a planet's colour, atmosphere, and biome palette predict
   the surface. Landing does not break the illusion.
3. **Flight is continuous.** A player can go surface → space → another surface, and dive to the
   seafloor and back, with no visible seam, pop, or load.
4. **Weird worlds exist.** At least a small fraction of worlds are genuinely surprising, and
   they are explained well enough to feel intentional.
5. **Content is data.** Adding a biome, landform suite, system profile, or affinity does not
   require touching generation code.
6. **Underwater is as deep as the surface.** A player can spend a full session underwater and
   still find new biomes, resources, and reasons to dive.

---

## Related

- [01 — World Model](./01-world-model.md) — what a "world" is spatially.
- [03 — The Planet Descriptor](./03-planet-descriptor.md) — the central data artifact.
- [04 — Physical Parameters](./04-physical-parameters.md) — the axes behind the labels.
- [14 — Open Questions & Roadmap](./14-open-questions-and-roadmap.md) — what is still undecided.
