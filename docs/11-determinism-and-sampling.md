# 11 — Determinism & Sampling

Two distinct but related concerns:

1. **Determinism** — the same seed must produce the same world, everywhere, in any order.
2. **Sampling** — pure randomness produces clumps and sameness across thousands of worlds;
   the distributions must be shaped deliberately.

Both are required for the game to work at scale.

---

## Determinism

### Counter-based RNG, not streams

Never use a shared, stateful random number generator during generation. Instead, every random
decision is a **pure function of an identity and a purpose**:

```
rand(identity, purpose, index) → value
```

Implementation: a counter-based generator (PCG, Philox, SplitMix64, or a hash of the key tuple).
The key is derived from the seed chain (see [01 — World Model](./01-world-model.md)) plus a
string or integer **purpose tag**.

Why this matters:

- Chunks are requested **out of order** while flying and changing LOD.
- Generation may run on multiple threads in any order.
- Saves and multiplayer require identical results across machines.
- Debugging requires reproducing a specific decision without replaying the whole game.

A stateful stream fails all four because the *n*-th call depends on how many calls happened
before.

### The seed chain

The chain is a strict hierarchy. Each level derives its children with a named **purpose tag**:

```text
galaxySeed
└── regionSeed   = H(galaxySeed,  "region", regionIndex)
    └── systemSeed = H(regionSeed, "system", systemIndex)
        └── bodySeed = H(systemSeed, "body", bodyIndex)
```

At body level, every subsystem gets its own derived seed. The tag is part of the key, so two
subsystems can never share a random stream:

| Purpose tag | Owns | Used at |
|---|---|---|
| `orbit` | orbital elements, epoch, mean anomaly | bake, system view |
| `bulk` | radius, mass, rotation, tidal lock | bake |
| `energy` | stellar flux jitter, albedo seed | bake |
| `atmosphere` | composition and pressure jitter | bake |
| `hydrosphere` | water inventory, ocean fraction, salinity | bake |
| `geology` | plates, provinces, craters, erosion | bake, L0 deltas |
| `climate` | climate preset jitter, wind offset, seasonal phase | bake, L1 |
| `regions` | ecological/geological/material region layout | bake |
| `biosphere` | biosphere level and palette draw | bake |
| `content` | biome, suite, and material selection | bake |
| `aquatic` | photic depth, currents, vents, reef sites | bake, volume query |
| `narrative` | affinity, anomalies, naming | bake |
| `poi` | points of interest | bake |
| `chunk` | per-chunk detail and scatter | query |
| `scatter` | per-chunk scatter placement | query |

```text
subsystemSeed = H(bodySeed, tag)                       // tag from the table
chunkSeed     = H(bodySeed, "chunk", face, level, x, y)
scatterSeed   = H(chunkSeed, "scatter", presetID)
```

### Feature-level seeds

A feature generated as a list (plates, craters, lakes, vents, POIs) derives its seed from the
subsystem seed plus a **stable index**, never from iteration order:

```text
plateSeed       = H(geologySeed,     "plate",  i)
craterSeed      = H(geologySeed,     "crater", i)
lakeSeed        = H(hydrosphereSeed, "lake",   i)
ventSeed        = H(aquaticSeed,     "vent",   i)
poiInstanceSeed = H(poiSeed,         "poi",    i)
```

The index is baked into the descriptor (`plates[i]`, `craters[i]`, `lakes[i]`), so the same entry
always reproduces the same feature. Feature lists are therefore **append-only within a generator
version**; removing or reordering an entry is a generator-version change.

Content-keyed draws use the content ID instead of an index, so adding content never moves an
existing draw:

```text
paletteSeed = H(contentSeed, "palette", familyID)       // per-body palette weight
mosaicSeed  = H(contentSeed, "mosaic",  familyID)       // per-family patch noise
jitterSeed  = H(contentSeed, "jitter",  axisName)       // classification axis jitter
```

See [19 — Biome Classification](./19-biome-classification.md).

### Tag rules

1. **Tags are lowercase ASCII, stable, and append-only.** A tag is never renamed or reused for a
   different decision.
2. **One tag, one subsystem.** A module draws only from its own seed. If it needs a value owned by
   another subsystem, that value is baked into the descriptor and read, not re-derived.
3. **Adding a decision never shifts existing values.** Each decision has its own key, so new tags do
   not perturb old worlds. This is the point of counter-based RNG.
4. **Chunk identity is positional.** `(face, level, x, y)` is the whole key. Chunks can be requested
   in any order, on any thread, without a sequence number.
5. **Siblings share inputs, never a stream.** A cross-chunk feature (a mountain range, a lake) is
   derived from the body-level subsystem seed, not from a neighbouring chunk's seed.
6. **Content is addressed by ID.** Scatter includes the preset ID in its key, so adding a new preset
   does not move existing instances.

`DECIDED` — hierarchical, purpose-tagged seed derivation with an append-only tag registry.

### Purity rules for query

The runtime query (see [09](./09-surface-field-stack.md)) must be:

- free of global mutable state,
- independent of call order,
- free of floating-point non-determinism across platforms where it matters (be careful with
  `sin`/`cos` and fast-math; if cross-platform determinism is required, use deterministic
  approximations or quantise).

`DECIDED` — query is pure. If cross-platform bit-identical terrain is required (e.g. shared
seeds between players), floating-point determinism must be validated early.

---

## Sampling

### The problem with iid randomness

Independent uniform sampling over a high-dimensional parameter space **clusters**. With
thousands of worlds you get:

- many near-identical temperate rockballs,
- empty regions of parameter space (almost no ice worlds, almost no ocean worlds),
- a distribution that feels random but not *varied*.

Players perceive clusters as sameness, even though the generator is "random".

### Stratified and low-discrepancy sampling

Sample the parameter space so it is **covered**, not just randomly populated:

- **Stratified sampling** — divide each axis into bins and sample within bins, guaranteeing
  coverage.
- **Low-discrepancy sequences** (Sobol, Halton) — points that fill the space evenly; use a
  scrambled sequence so it does not look like a grid.
- **Quota sampling** per region — e.g. each region must contain at least one ocean world, one
  ice world, one gas giant, one anomaly.

This is the single most effective anti-sameness measure.

`DECIDED` — use stratified/low-discrepancy sampling for body parameters, per region.

### Weighted tables with explicit masses

Any categorical choice (system profile, body class, biosphere level, biome) uses a weighted
table with **explicit probability masses** in data. No implicit fallthroughs, no "pick a random
item from a list". This makes distributions auditable and tunable.

```toml
[biosphere_weights.oasis]
none      = 0.10
microbial = 0.20
complex   = 0.60
exotic    = 0.10
```

### Rejection and resampling

Soft physical constraints (see [04 — Physical Parameters](./04-physical-parameters.md)) are
implemented as rejection sampling with a small acceptance probability, so rare "impossible"
worlds survive as anomalies. Cap the number of attempts to guarantee termination.

---

## The diversity audit

This is the tool that answers "does it feel samey?" before shipping.

### Feature vector

For each body, build a fixed-length feature vector summarising its identity:

- class (one-hot),
- temperature, moisture, ocean fraction, gravity, atmosphere pressure,
- tectonic activity, volcanism, erosion mix,
- biome palette distribution,
- landform suite distribution,
- biosphere level, affinity,
- ringed / moon count / tidally locked flags.

Normalise each dimension.

### Metrics

Sample 10,000 descriptors and report:

- **Per-axis histograms** — are there empty bins? spikes?
- **Pairwise distances** — distribution of nearest-neighbour distances; too many near-zero
  distances means clumping.
- **Cluster counts** — k-means or similar; too few clusters means low variety.
- **Entropy per axis** — low entropy means an axis is barely used.
- **Coverage of rare labels** — anomalies, ocean worlds, ice worlds, ringed worlds.

### Use

Run the audit:

- after changing any weight table,
- after adding a biome or landform suite,
- before each milestone.

Treat it as a regression test: store baseline metrics and fail if variety drops below
thresholds.

`PROPOSED` — the audit should be a command-line tool that reads the same data files as the game
(see [12 — Authoring & Tuning](./12-authoring-and-tuning.md)).

---

## Golden seeds

Maintain a small set of **golden seeds** with expected descriptor hashes:

- a handful of representative worlds (temperate ocean, barren, gas giant, tidally locked,
  anomaly),
- assert that bake produces the same descriptor hash across code changes,
- update intentionally when `generatorVersion` changes.

This catches accidental drift in generation, which is otherwise invisible until a player reports
that their base is now underwater.

Golden hashes are checked in CI on **Windows and Linux**, in release and debug builds, on every push.
[E2](./spikes/E2-determinism.md) established the pattern and confirmed that generation is
bit-identical across threads, evaluation order, build profile, and OS.

In production (P1-A): `crates/genesis_gen/tests/golden.rs` pins the descriptor hash of body 0 for
E2's ten seeds. Only rocky bodies exist so far, so the set is not yet the representative list
above; worlds of the other kinds join it as their generators arrive. The test also asserts
`GENERATOR_VERSION`, so a version bump forces a deliberate re-pin. Bake phases draw from the
registered subsystem tags `bulk`, `hydrosphere`, `geology`, `energy`, `atmosphere`, `climate` and
`biosphere`, and the feature seeds `H(geologySeed, "plate", i)` and
`H(geologySeed, "crater", i)`. Purposes used inside them are listed in `genesis_gen::bake` and
`genesis_gen::physical`.

Version history of the golden set: **1** — P1-A, the rocky bake. **2** — P1-C1, the physical
phases (energy, atmosphere, water state, climate summary, biosphere); the ten hashes were re-pinned
deliberately in that change, and that is the only reason they may differ from 1.

---

## Save and version policy

- Descriptors store `version`, `generatorVersion`, and `contentVersion` (see
  [03 — The Planet Descriptor](./03-planet-descriptor.md)).
- Golden-seed tests pin old outputs.
- When the generator changes, freeze coarse decisions, migrate detail, and promote any derived
  field a save references (D26).
- Never silently regenerate the surface of a world the player has a base on.

The policy is settled; only the mechanics are open. See
[03](./03-planet-descriptor.md#versioning-and-regeneration) and
[16 — Data Contracts](./16-data-contracts.md#frozen-derived-and-cached).

---

## Related

- [01 — World Model](./01-world-model.md) — seed chain and identity.
- [03 — The Planet Descriptor](./03-planet-descriptor.md) — versioning.
- [04 — Physical Parameters](./04-physical-parameters.md) — soft constraints and rejection.
- [09 — Surface Field Stack](./09-surface-field-stack.md) — purity of query.
- [12 — Authoring & Tuning](./12-authoring-and-tuning.md) — weight tables as data.
