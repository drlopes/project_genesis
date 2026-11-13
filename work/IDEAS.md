# Ideas

Rules and entry format: [work/README.md](README.md). Ideas are triaged at the end of each goal;
they are not acted on directly.

## Open

### I-001 — Bake classification inputs that need extra `h0` evaluations
- **Status:** parked (revisit after E4)
- **Raised:** 2026-11-10, from [E1](../docs/spikes/E1-field-cost.md#open-optimisations-not-needed-yet)
- **What:** store slope and the orographic upwind term on the macro grid at bake, instead of
  evaluating `h0` three more times per sample.
- **Why:** about 500 ns of the ~1.05 µs L0–L2 cost; roughly halves classification-band cost while
  keeping LOD independence.
- **Cost / risk:** one more cache (memory per body); slope resolution drops to the macro grid
  (~150 m on a 24 km body).
- **Decision:** parked until E4 shows whether throughput needs it. **E4 (2026-11-11): not needed**:
  rover speed uses ~10% of generation capacity. Stays parked; it would only raise the low-flight
  speed cap (speed / height ≤ ~6 /s), so revisit if fast low flight becomes a design goal.

### I-002 — Multi-channel noise for classification jitter
- **Status:** parked (revisit after E4)
- **Raised:** 2026-11-10, from [E1](../docs/spikes/E1-field-cost.md#open-optimisations-not-needed-yet)
- **What:** one lattice hash per corner, with different gradient bits per channel, so the four
  jitter axes cost about one noise evaluation instead of four.
- **Why:** ~50 ns per sample; small, but jitter runs on every classified sample.
- **Cost / risk:** new noise function to pin with known answers; low.
- **Decision:** parked at the end of the work-structure goal: E1 is within budget, so this only
  matters if E4 shows a throughput shortfall. Revisit together with I-001. **E4: no shortfall at
  Phase 1 speeds**; stays parked with I-001.

### I-003 — Mirror work items to GitHub Issues when collaborators join
- **Status:** parked
- **Raised:** 2026-11-10, while designing the work structure
- **What:** keep `work/*.md` as the source of truth and mirror open bugs and accepted ideas to
  GitHub Issues for discussion and boards.
- **Why:** Issues have better UI and notifications once more than one person is involved.
- **Cost / risk:** two sources can drift; needs a one-way sync rule or tooling.

### I-004 — View-dependent LOD: less detail outside the view frustum
- **Status:** parked (revisit after E4)
- **Raised:** 2026-11-10, from E3
- **What:** select patches outside the camera frustum with a coarser error threshold (e.g. 4× the
  screen-space error) instead of full detail.
- **Why:** E3's selection ignores view direction, so patches behind the camera are generated and kept
  resident at full detail. Bevy does not draw them, but they cost generation, RAM, and VRAM; roughly
  two thirds of the near-ground patch set is outside a 60° view.
- **Cost / risk:** pop-in when the camera turns quickly; needs a turn-rate test. Interacts with
  collision (E6), which needs geometry around the player regardless of view.
- **Decision:** parked at the end of E3. E4 measures streaming load; decide then whether the
  saving is needed. Note that geomorphing hides pop-in on turns only partly (0.5 s morph).
  **E4: not needed** (throughput margin is large; memory within budget since I-005). Stays parked.

### I-006 — Fewer entities per patch set
- **Status:** parked (revisit after E4)
- **Raised:** 2026-11-10, from E3
- **What:** reduce per-patch main-thread overhead (one entity, one mesh asset, one draw call per
  patch): batch uploads, pool and reuse mesh buffers, or render patches through one custom
  instanced/indirect draw.
- **Why:** E3's grid-33 runs failed on main-thread integration rate, not on worker throughput: the
  per-patch fixed cost, not the vertex count, set the ceiling.
- **Cost / risk:** a custom render path is real engine work; pooling is cheap.
- **Decision:** parked at the end of E3. E4 measures integration rate under sustained movement,
  which is exactly what this would improve; decide with that data. **E4: main-thread terrain
  work stayed ≤ 1.32 ms at p99 in all 16 runs** (budget 2 ms). Stays parked.

### I-007 — Revisit Bevy `dynamic_linking` at the next Bevy upgrade
- **Status:** parked (until the next Bevy upgrade)
- **Raised:** 2026-11-10, from E9
- **What:** check whether `bevy_dylib` matches the Bevy version and measure incremental builds with
  `dynamic_linking` in dev.
- **Why:** it usually cuts incremental link time sharply; on 0.20.1 it is unavailable because
  `bevy_dylib 0.20.1` was never published.
- **Cost / risk:** none to check; dev-only feature, never shipped.
- **Decision:** stays parked at the end of E3; static Bevy with `rust-lld` meets the targets.

### I-008 — Coverage-first generation priority
- **Status:** new
- **Raised:** 2026-11-11, from E4
- **What:** order the generation queue so that a wanted patch with no displayable ancestor (it
  would be a hole) goes before refinements of areas that are already covered, and nearest-first
  only within each class. Equivalently: generate the coarsest missing ancestor first.
- **Why:** at saturation, nearest-first spends the workers on fine patches under the camera and
  leaves uncovered patches waiting. **Corrected by the E4 analysis:** the holes counted in the
  smoke and sweep runs were all invisible far-side patches (see I-009); visible terrain stayed
  covered and only lagged in detail. Coverage-first still matters where nothing is resident yet:
  arrival after fast travel or a teleport (initial fill is 1.3–2.0 s with 4 workers).
- **Cost / risk:** small change to the priority key; must not starve refinement near the camera at
  normal speeds.
- **Decision:** kept open at the end of E4 (no visible problem today); revisit with fast travel or
  arrival design.

### I-009 — Tighter horizon culling
- **Status:** new
- **Raised:** 2026-11-11, from E4
- **What:** cull patches against the horizon with a bound that uses the patch's actual height
  range (min/max from the macro pyramid) rather than its size plus the body's whole relief.
- **Why:** the conservative test admits coarse patches 35–45 km away on the far side of a 24 km
  body. They are never visible but get queued, starve under load, and made E4's hole metric count
  ~35 000 invisible "hole" frames.
- **Cost / risk:** needs per-patch height bounds (cheap from the pyramid); a wrong bound would cull
  visible terrain, so test against mountains on the limb.

### I-010 — Solids the radial field cannot express: arches, caves, floating islands
- **Status:** new
- **Raised:** 2026-11-12, from questions during E7 about arches and floating islands
- **What:** three ways to get a solid that is not one surface per direction. (a) A voxel region
  enclosing the feature: a signed field is enough, so the arch's underside and a free-floating
  island are just sign patterns. (b) A **local solid** (landmark): a small star-shaped field with
  its own centre, LOD and streaming, e.g. a floating island as its own entity. (c) A **union of
  star-shaped blobs** (SDF `min`), which is not star-shaped as a whole and can give rings and
  arches with holes through them.
- **Why:** the radial model (`|p| < radius + height(dir(p))`) has exactly one surface per
  direction, so arches, caves and floating islands are impossible today; an "arch" attempt is a
  hill with a notch. Environment variety is the main product goal, so these are likely wanted.
- **Cost / risk:** (a) memory and chunk allocation scale with the region's vertical extent — the
  current E7 build allocates only the band around the ground, so a floating island would be
  missed until allocation tests solid samples instead of the ground column. An arch must be fully
  enclosed, since the heightfield boundary can only cut single-valued surface. (b) a new world
  entity: streaming, collision, gravity, persistence, and a rule for which LODs it uses. (c) it
  breaks docs/18's "one star-shaped solid" assumption behind the sphere trace and the
  inside/outside test; a union needs each `g` to stay a conservative distance bound, and a
  radius bound per blob for culling.
- **Decision:** captured, not decided. Needs a world-model/content decision before any generator
  or save format depends on it; E7 only measures option (a)'s numbers.

### I-011 — Large edits visible from space (bake vs evaluate, and the terraform boundary)
- **Status:** new
- **Raised:** 2026-11-12, from a question during E7
- **What:** make a large player edit readable at coarse LODs, so a big crater or scar can be seen
  from orbit. Options: evaluate edits as an extra height term at every sample spacing (analytic
  records), or bake them into a cache — the macro pyramid, or a displacement texture with mipmaps
  added as a last layer to the generator.
- **Why:** the coarse mesh samples the height function every 0.5–4000 m depending on distance, so
  an edit is only visible if it spans several samples at the chosen band. That limit is LOD, not
  storage: a texture does not make a 10 m hole visible from orbit, and should not (it would alias).
- **Cost / risk:** a planet-wide texture has a fixed texel size (blocky close up), seams between
  tiles, its own streaming, and must be rebuilt bit-identically from the edit records or two
  clients disagree (E2). Analytic evaluation is exact and cheap for a handful of edits but costs
  per sample for thousands. The macro pyramid is already the "from space" cache; rebaking its
  affected cells is the natural route, with a bake version. Also a policy boundary: a multi-km
  edit is a terraform, and docs/18 already excludes global terraforming from ordinary edits — a
  threshold (edit vs narrative/descriptor event) has to be chosen, especially if edits ever become
  shared between players.
- **Decision:** captured. Measure first (E7 is 0.25–1 m edits, near the player); implement height
  edits as an extra term per LOD, then cache only when edit counts justify it.

### I-012 — Floating debris left by small edits
- **Status:** new
- **Raised:** 2026-11-13, from the user's E7 look test at 0.5 m ("smaller details get lost and
  sometimes leave floating bits")
- **What:** after overlapping small digs, thin slivers of solid only a voxel or two thick can be
  cut off from the ground and are meshed as floating fragments. Options: (a) after each edit,
  flood-fill solid samples within the edit's dirty chunks and remove components that touch neither
  the ground nor the region floor and are below a size threshold; (b) turn such components into
  physics debris that falls; (c) a minimum brush radius of ~2 voxels (1 m) so slivers are rarer.
- **Why:** it reads as a bug, not as digging. Floating solids also collide.
- **Cost / risk:** (a) and (b) must be deterministic and part of applying the record, or replay
  diverges from the live region (E7 checks replay bit for bit). (b) is a gameplay decision.
- **Decision:** captured; decide with the editing tools in Phase 2.

### I-013 — Coarser voxel LOD for distant resident regions
- **Status:** new
- **Raised:** 2026-11-13, from the user's E7 look test ("1 m … only acceptable if viewed from a
  great distance, as a LOD")
- **What:** a base or dig site keeps its voxels resident when the player leaves (D27). Seen from
  far away, mesh it from 1 m (or coarser) samples, the way the heightfield coarsens with distance.
- **Why:** an edited site must keep its shape when seen from a distance, without full-resolution
  meshes for every resident region.
- **Cost / risk:** neighbouring voxel LODs need crack handling (Transvoxel-style transition cells,
  or skirts as the heightfield uses), and a downsampled field must keep thin features from
  vanishing. Interacts with I-011 (edits visible from orbit).
- **Decision:** captured; revisit when bases exist.

### I-014 — Voxelise on first edit
- **Status:** accepted → docs/10 and docs/18 (2026-11-13, the user: "That's the better way")
- **Raised:** 2026-11-13, from E7
- **What:** keep heightfield patches for unedited ground everywhere, including near the player; a
  chunk switches to voxels on its first edit, together with the neighbours whose mesh reads its
  samples.
- **Why:** E7 measured unedited voxels and the heightfield agreeing within 2 cm, so voxels add
  nothing until something is edited. Building a full ±64 m region costs 4.2 s single-threaded on
  arrival and 17 MB; on first edit it would cost ~33 ms per brick, only where needed.
- **Cost / risk:** the heightfield must give up edited chunk footprints at runtime (a hole in the
  patch mesh, or patch-level masking), with the non-radial enclosure rule of docs/18 applied at
  that cut; the collision patches change the same way. Narrows D27 ("voxels near the player").
- **Decision:** accepted 2026-11-13; refines D27 ("voxels where edited, near the player"). The
  user's reason: fast movement near the surface (I-015) must not wait on terrain builds.

### I-015 — Races near the surface: starships and ground/hover vehicles
- **Status:** new
- **Raised:** 2026-11-13, by the user (a gameplay idea for later)
- **What:** races of starships and ground or hovering vehicles close to a planet's surface.
- **Why it matters for the engine now:** terrain must keep up at high speed and low height. E4's
  limit is full detail while **speed ÷ height above ground ≤ ~6 /s** (4 workers, reference
  hardware): a hover racer at 5 m keeps full detail up to ~30 m/s, a ship at 50 m up to ~300 m/s.
  Above that, terrain degrades to coarser detail (never holes), which at racing heights is
  visible and also coarsens collision. Voxels on first edit (I-014) keeps voxel builds out of this
  path; heightfield streaming is the limit that remains.
- **Options:** (a) a race course is known in advance, and generation is deterministic, so the
  corridor along the track can be generated before the race and kept resident (prefetch along a
  path, not around the camera); (b) view-dependent LOD (I-004): racers look forward, so spend less
  behind and to the sides; (c) coverage-first priority (I-008); (d) more workers on bigger
  machines; (e) design speeds and heights to the cap (hover racers fly higher at top speed).
- **Cost / risk:** (a) is cheap for authored or seeded tracks and is the likely answer; free-roam
  racing at low height needs (b)–(d) and new measurements. Physics at racing speed needs swept
  contacts (E6 found fall-through only above ~150 m/s, from body–body impacts).
- **Decision:** captured; revisit when vehicles are designed. Keep it in mind for any choice that
  adds work on the terrain streaming path.

## Closed

### I-005 — Compressed patch vertex format
- **Status:** accepted → done (`eaa0b23`, 2026-11-11; format in docs/10 "Patch vertex format")
- **Raised:** 2026-11-10, from E3
- **What:** store patch vertices compactly: 16-bit positions relative to the patch, octahedral-
  encoded normals, RGBA8 colours, for both the own and the parent (morph) state.
- **Why:** with geomorphing each vertex carries two states (80 bytes), and SSE 16 peaks at ~2.4 GB
  of terrain VRAM against a 2 GB budget. A compact format should bring it to roughly 32 bytes.
- **Cost / risk:** custom vertex layout and shader decoding; precision of 16-bit positions over
  large patches (coarse levels may need 32-bit); must keep the morph contract (a fully morphed
  child equals its parent) within quantisation.
- **Decision:** accepted at the end of E3: the terrain VRAM budget cannot be met at the accepted
  default (SSE 16 with morphing) without it.
