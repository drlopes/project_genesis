# E7 — Voxels and edits

Experiment definition: [21 — Phase 1 Spike Plan](../21-phase-1-spike-plan.md#e7--voxels-and-edits-q2).
Status: **done.** Voxel size **0.5 m** (the coarsest that passed the user's look test), surface
nets, edits persisted as records and replayed.

| | |
|---|---|
| Date | 2026-11-13 |
| Harness | `spikes/voxel` (no engine): `spike_voxel --radius R --voxel V [--edits] [--seam] [--blocky] [--hf S]`, one JSON line per test. Single-threaded throughout |
| Viewer | `spike_app --bin voxel_view` (Bevy): the region meshed at 2 m eye height inside a heightfield ring from the same field. Keys 1/2/3 voxel size, B blocky, E edits, T tint voxel vs heightfield; `--shot` for screenshots |
| Field | the E1 spike field, seed 3, 24 km body, full L0–L4 depth |
| Site | `Site::pick`: a land site above the sea and not steep, deterministic; local frame x east, y north, z up |
| Hardware | Ryzen 7 9800X3D, 32 GB, RTX 4060, Windows 11 |

## Model

- The solid inside a region is the radial field's signed distance, sampled at voxel corners:
  `g(p) = |p| − (radius + height(dir(p)))`, negative inside. Voxels add editability here, not new
  shapes; non-radial shapes need a generator layer (I-010).
- Storage: **bricks of 32³ f32 samples** (128 KB), allocated where the terrain passes through the
  chunk's column, from 8 m below the lowest ground to 2 m above the highest. Samples are global:
  a brick owns `[32c, 32c + 32)` on each axis, so an edit writes exactly one copy.
- An edit that reaches a brick that is not resident **allocates it from the field first**. The
  first version dropped such edits, and a deep dig opened a see-through hole in the region floor.
- Meshing: **surface nets** per chunk (one vertex per sign-changing cell at the mean of its edge
  crossings, one quad per sign-changing edge). Chunk ownership is in
  [docs/18](../18-collision-and-edits.md#the-voxelmesh-boundary); it took two fixes (below).
- Edits: sphere dig/add and cylinder flatten. Every edit is logged as a record; the region is
  rebuilt from the base plus the records to check replay.

## Results

### Region size, memory, meshing, seam (unedited)

Seam: the voxel mesh's top surface against a bilinear heightfield at 0.5 m spacing (the finest
patch spacing), on vertical rays every `max(voxel, 0.5)` m across the whole box; *edge* is the
band along the box edge where the two meshes meet. *Error* is the distance from the meshed point
to the analytic solid. Holes are interior rays that hit nothing.

| Voxel | Region | Box (m) | Bricks | RAM | Build | Triangles | Mesh MB | Mesh all | Chunk p50 / p99 | Seam p99 / max | Edge max | Error p99 | Holes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 m | ±32 m | 64×64×64 | 8 | 1.0 MB | 0.25 s | 8 772 | 0.2 | 32 ms | 2.9 / 5.7 ms | 33 / 49 mm | 42 mm | 43 mm | 0 |
| 0.5 m | ±32 m | 64×64×32 | 32 | 4.2 MB | 1.0 s | 35 420 | 0.8 | 94 ms | 2.9 / 5.5 ms | 10 / 15 mm | 12 mm | 23 mm | 0 |
| 0.25 m | ±32 m | 64×64×24 | 179 | 23.5 MB | 5.8 s | 142 758 | 3.1 | 376 ms | 2.8 / 5.6 ms | 7 / 16 mm | 9 mm | 8 mm | 0 |
| 1 m | ±64 m | 128×128×64 | 32 | 4.2 MB | 1.0 s | 35 332 | 0.8 | 94 ms | 2.8 / 5.6 ms | 33 / 57 mm | 43 mm | 43 mm | 0 |
| **0.5 m** | **±64 m** | 128×128×32 | 127 | **16.6 MB** | 4.2 s | 142 706 | 3.0 | 311 ms | 2.8 / 5.4 ms | **10 / 17 mm** | 14 mm | 23 mm | 0 |
| 1 m | ±128 m | 256×256×64 | 118 | 15.5 MB | 3.9 s | 143 712 | 3.1 | 306 ms | 2.8 / 7.3 ms | 33 / 66 mm | 53 mm | 43 mm | 0 |
| 0.5 m | ±128 m | 256×256×32 | 448 | 58.7 MB | 15.2 s | 579 836 | 11.8 | 1 070 ms | 2.9 / 5.6 ms | 10 / 20 mm | 15 mm | 23 mm | 0 |
| 0.25 m | ±64 m | — | ~720 | ~94 MB | ~23 s | ~570 k | ~12 | — | — | — | — | — | not run |
| 0.25 m | ±128 m | — | ~2 900 | ~375 MB | ~95 s | ~2.3 M | ~48 | — | — | — | — | — | not run |

The 0.25 m rows at ±64 / ±128 m are extrapolated from ±32 m (bricks scale with area); they were
not run because 0.25 m was already over the remesh limit (below).

- **Seam: pass everywhere.** Tolerance is `0.5 · voxelSize` (docs/18): 250 mm at 0.5 m against a
  measured max of 20 mm. The meshes differ by the voxel's own approximation error, not by a step.
- **Cost is set by samples and triangles, not by the region radius.** Meshing is ~2.8 ms per
  brick at every size. Building is ~1.0 µs per sample single-threaded (field evaluation at full
  depth), i.e. ~33 ms per brick.
- **RAM: pass.** docs/21 allows 1.5 GB per body for terrain + scatter + voxels, terrain alone uses
  ~0.43 GB. A ±64 m region at 0.5 m costs 17 MB of samples + 3 MB of mesh.

### Edits and remesh latency

Scripted, at the site centre: flatten (r 8 m), a tunnel (17 overlapping r 1.5 m digs 4 m below
ground), a cliff carve (r 6 m), then 500 deterministic r 1 m digs; then the largest dig whose
edit + remesh fits one 60 Hz frame (16.7 ms), trying r = 1, 2, 3, 4, 6, 8 m.

| Voxel / region | Flatten | Tunnel | Carve | 500 digs | Dirty chunks | Remesh dirty | Largest dig in a frame | Bricks allocated by edits |
|---|---|---|---|---|---|---|---|---|
| 1 m / ±32 | 0.4 ms | 2.0 ms | 0.1 ms | 40 ms | 8 | 32 ms | 8 m (7.9 ms) | 0 |
| 0.5 m / ±32 | 0.4 ms | 2.0 ms | 0.1 ms | 46 ms | 12 | 31 ms | 4 m (4.3 ms) | 0 |
| 0.25 m / ±32 | 1.6 ms | 2.5 ms | 0.7 ms | 60 ms | 64 | **118 ms** | 4 m (15.3 ms) | 6 |
| 1 m / ±64 | 0.4 ms | 2.0 ms | 0.1 ms | 39 ms | 8 | 15 ms | 8 m (3.8 ms) | 0 |
| **0.5 m / ±64** | 0.5 ms | 2.5 ms | 0.1 ms | 45 ms | 12 | **23 ms** | **8 m (15.1 ms)** | 0 |
| 1 m / ±128 | 0.4 ms | 2.0 ms | 0.1 ms | 36 ms | 8 | 15 ms | 8 m (3.7 ms) | 0 |
| 0.5 m / ±128 | 0.4 ms | 2.0 ms | 0.1 ms | 40 ms | 12 | 22 ms | 8 m (15.2 ms) | 0 |

- **Remesh ≤ 50 ms: pass at 0.5 and 1 m, fail at 0.25 m** (118 ms, single-threaded). Chunks remesh
  independently, so 4 workers would bring 0.25 m to ~30 ms; it was not needed.
- The edit itself is cheap (a sphere write); the cost is the remesh of the chunks it dirties. At
  0.5 m a dig up to 4–8 m radius fits one frame: how many chunks the brush straddles decides it
  (two of three runs fit 8 m).
- Edited meshes: still 0 holes and the same seam numbers; the edges of the region are untouched.

### Edit records: storage, replay, garbage collection

A record in a fixed binary layout (id, sequence, createdAt, author, base generator version,
domain + op, chunk key, centre relative to the chunk, radius, one payload value) is **66 bytes**.

| Voxel / region | Records | Record bytes | Replay | Replay = live | No-op records | Changed samples | Changed-voxel delta |
|---|---|---|---|---|---|---|---|
| 0.5 m / ±32 | 525 | 34.7 KB | 50 ms | **bit-identical** | 10 | 82 966 | 332 KB |
| 0.25 m / ±32 | 525 | 34.7 KB | 252 ms | bit-identical | 10 | 460 858 | 1.84 MB |
| 0.5 m / ±64 | 525 | 34.7 KB | 50 ms | bit-identical | 3 | 85 936 | 345 KB |
| 1 m / ±128 | 525 | 34.7 KB | 40 ms | bit-identical | 0 | 17 099 | 69 KB |
| 0.5 m / ±128 | 525 | 34.7 KB | 43 ms | bit-identical | 0 | 87 185 | 352 KB |

The delta is the alternative docs/18 names ("store only the changed voxels"): 4 bytes per changed
sample (u16 index in the brick + f16 value) plus a chunk key per changed brick.

- **Storage per 1 000 edits: 66 KB as records**, independent of voxel size. The same edits as a
  delta cost ~10× more at 0.5 m and ~50× at 0.25 m.
- **Replay reproduces the live region bit for bit**, in the log's order, at every size. Edits are
  a function of the record set, as docs/18 requires. Replay costs ~0.1 ms per record at 0.5 m.
- **GC policy (from these numbers):** drop a record that changes nothing when applied (no-ops,
  0–2 % here; they are known at creation). Records stay the persistent form; a chunk's records are
  compacted into a changed-voxel delta only when they cost more than the delta (bytes or replay
  time). That is the superseded-edit case: many records rewriting the same voxels, as in mining a
  mountain (Q12). Ordinary digging never reaches it.

### Blocky baseline

At 0.5 m, cube faces give half the triangles of surface nets (17 684 vs 35 420 at ±32 m; 71 144 vs
142 706 at ±64 m) but more memory (1.3 vs 0.8 MB; vertices are not shared). Not a candidate for
the look; kept as a reference and a debug view (**B** in the viewer).

## Look test (the user, 2 m eye height, 2026-11-13)

| Voxel | Verdict |
|---|---|
| 0.25 m | "As expected, the most detailed one. The terrain edits are sharp and the spherical dug outs read cleanly." |
| **0.5 m** | "The edit's shape still read although not as clearly as the 0.25 voxel size. Undisturbed terrain has no visible issues. Smaller details get lost and sometimes leave floating bits. **Acceptable overall.**" |
| 1 m | "Too coarse. Makes the terrain read as broken instead of deliberately edited. Only much larger features are readable. Only acceptable if viewed from a great distance, as a LOD." |

**Stop rule: 0.5 m** is the coarsest size that passes. Follow-ups: the floating bits (I-012),
1 m as a distant voxel LOD (I-013).

## Decision

- **Voxel size 0.5 m** (D44). Passes the look test, the seam (20 mm against 250 mm), the remesh
  limit (23–31 ms against 50 ms), and RAM (17 MB per ±64 m region).
- **Mesher: surface nets.** Marching cubes was not measured: surface nets passed the look test and
  the seam, so per the plan's own note ("check which is worth measuring") it was not worth a
  second extractor. If sharp edits ever need crisper edges, the candidate is dual contouring
  (surface nets plus a per-cell feature point), not marching cubes.
- **Region radius: ±64 m** default. ±128 m fits in RAM (59 MB) and the remesh limit, but its full
  build takes 15 s single-threaded; see the proposal below.
- **Edits persist as records** (66 B each); GC as above.
- **Voxelise on first edit** (I-014; accepted by the user 2026-11-13, written into docs/10 and
  docs/18). The unedited voxel surface and the heightfield agree within 2 cm, so a chunk only needs
  voxels once something edits it (plus the neighbours its mesh reads). Arrival then costs nothing,
  and RAM scales with what was edited, not with the radius. Bricks allocated on demand already work
  (the edit path does it); what is new is the heightfield giving up chunk footprints at runtime.
  This refines D27's "voxels near the player" to "voxels where edited". The user's reason: fast
  movement near the surface, such as racing (I-015), must not wait on terrain builds.

## Bugs found and fixed during E7

- **B-005 — the crater term had a vertical wall at every rim** (`spikes/field`, S2). The outside
  branch started at `0.2 · depth` where the bowl ended at 0: a step of up to 60 m. It showed up as a
  12 m "seam outlier" at ±128 m (a 47 m jump in 0.1 m of field). The rim profile is now continuous.
- **Cracks along every chunk boundary** (mesher). Each chunk built only quads whose four cells
  were its own, so the quads straddling a boundary belonged to nobody. Fixed by the ownership rule
  in docs/18. Not caught by the seam metric, which skipped rays that hit nothing; it now counts
  them as holes and the result must be 0.
- **See-through faces at dig rims** (mesher). Winding came from averaged vertex normals, which can
  point the wrong way at a sharp rim, and the face was culled. Winding now comes from the voxel
  signs of the edge.
- **Edits dropped outside resident bricks** (a deep dig opened the region floor), and **dirty sets
  missing the neighbour chunks** that read the edited samples as their apron. Both fixed.
- Blocky face test read out of bounds; fixed.
- One single-ray "hole" was a ray exactly through a shared edge (f32 rounding); the metric retries
  1 mm off before counting a hole.

## Caveats

- Everything is single-threaded. Region build (~33 ms per brick) must run on workers, nearest
  bricks first, or be avoided by voxelising on first edit.
- The region is a band around the ground, so floating shapes would not be allocated (I-010).
- Edits are sphere and cylinder brushes only; materials, height-domain edits and water edits were
  not part of E7.
- The record layout is the spike's; the schema stays `PROPOSED` in docs/18.
