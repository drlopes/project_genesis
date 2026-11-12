# E6 — Collision and physics

Experiment definition: [21 — Phase 1 Spike Plan](../21-phase-1-spike-plan.md#e6--collision-and-physics).
Status: **in progress.** Avian measured; Rapier and the walking character not yet.

| | |
|---|---|
| Date | 2026-11-12 |
| Harness | `spikes/physics` (headless Bevy 0.20.1, `MinimalPlugins`, fixed 60 Hz, one update = one step). `spike_physics drops\|rays\|rest [options]`; engine by cargo feature (`avian` default, `rapier`) |
| Terrain | the E1 field; collision patches at the finest level (grid 65, 0.29 m spacing), a 5 × 5 block (~90 m) around each test site, built by the same code as the visual patches (`spike_app::patch`), skirts excluded |
| Frames | each site has its own physics frame (origin on the surface, +Y up, f64 positions differenced then cast, per E5); `--frame naive` puts body-local f32 positions into the engine (control) |
| Hardware | Ryzen 7 9800X3D, 32 GB, Windows 11 |

## Candidates and the collider question

| | Avian 0.8 | Rapier (`bevy_rapier3d` 0.37) |
|---|---|---|
| Bevy 0.20 | yes | yes |
| 64-bit positions | `f64` feature | no (f32 only in the Bevy plugin) |
| Custom field collider (option b) | `AnyCollider` exists, but it replaces the collider type for the whole world and contacts are only generated between two colliders of that type: we would own contact generation for every shape pair, and spatial queries need their own path | no custom contact dispatcher exposed |
| Per-patch meshes (option a) | `Collider::trimesh_with_config` | trimesh / heightfield colliders |

Neither accepts a custom field collider cleanly, so per the stop rule E6 uses **option (a):
per-patch triangle meshes** built from the visual patch vertices. That matches docs/18's collision
LOD ("equal to the visual LOD within the interaction radius"). Triangle meshes rather than
heightfields, because cube-sphere patches are not planar grids. Raycasts are also measured with our
own sphere trace on the field (docs/18), which needs no engine.

## Avian 0.8 results

### Drops: 1 000 capsules (r 0.4 m, 1.8 m tall) at 0–300 m/s, 0–45° from vertical

Each capsule is aimed at a point within ±8 m of a site and starts ~0.1 s (6 steps) before impact.
Fall-through = the capsule centre more than 0.5 m below the field surface while it was over the
inner 3 × 3 patches the step before, and still below at the end (none recovered in any run).

| Variant | Fall-throughs (per 1 000) | Notes |
|---|---|---|
| Swept CCD off (control) | **375–377** | from 50 m/s upward |
| Avian default (swept CCD on for fast bodies) | 6 | all > 150 m/s, at impact |
| + speculative CCD | 6 | |
| + linear sweep mode | 1 | |
| + CCD threshold 0.1 | 6 | |
| + 12 substeps | 3 | |
| + collision margin 2 cm / 5 cm on the terrain | 0–5 / 0–4 | depends on site layout |
| **Capsules collide only with the terrain** (`--solo`), margin 0 / 2 / 5 cm, 4 site layouts each | **0 in all 12 runs (12 000 drops)** | |

**Terrain collision never failed on its own.** Every fall-through with CCD on came from capsules
hitting each other: a body struck at up to 300 m/s while sitting on the ground is driven through it
in one step (Avian sweeps fast bodies against static and kinematic bodies by default, not against
other dynamic bodies).

Step time (20 capsules per site, 25 terrain patches): p50 0.20–0.28 ms; p99 3–7 ms during
high-speed impacts with many bodies.

### Rays: 5 × 10 000 rays from 1–50 m above the ground, 10–90° below the horizon

| | ms per 10 000 rays |
|---|---|
| Avian `SpatialQuery::cast_ray` on the patch meshes | **4.8–6.5** |
| Our sphere trace on the field (docs/18) | **483–517** (≈ 48 field samples per ray) |

Over the collision block (39 169 rays whose field hit lies on it), mesh and field agree to
**1.6 mm (p50) and 1.2 cm (p99)**. Six rays (0.015%), all 40–60 m out on shallow rays, hit 3–6.6 m
farther on the mesh than on the field (two missed the 5 × 5 block entirely; with a 7 × 7 block they
hit farther). Interpretation, not proven point by point: rays grazing convex crests, where the
mesh's straight chords lie a few millimetres below the true surface.

### Rest: two 1 m boxes per site, 4 sites, 10 simulated minutes after settling

| Body, frame | Box allowed to sleep | Box never sleeping |
|---|---|---|
| 24 km, local frame | **0.0 mm** on all sites | creeps 0.41–1.02 m (2–16° slopes) |
| Earth radius (6 371 km), local frame | **0.0 mm** | creeps 0.15–0.50 m (3.5–7.4°) |
| Earth radius, naive body-local f32 (control) | 3.6–18.6 m | 6–13 m; one box fell 1 784 km through the planet |

Controls for the creep, never-sleeping box on a flat native box collider: level 0.04 mm; tilted
2° 0.34 m, 8° 1.42 m, 16° 3.18 m. **Creep is the solver's static friction slowly yielding on slopes**
for bodies that never sleep, not a terrain-mesh effect. With sleeping, bodies stay exactly put.

## Harness lessons

1. **Bodies spawned within one step of the ground pass straight through** (no contact pair or
   previous position yet). The first harness version did this at 250–300 m/s. Fast spawns need a
   sweep at spawn.
2. **Avian sweeps fast bodies by default.** Adding `SweptCcd::default()` changes nothing; turning
   CCD off needs `CcdFilter::NONE`. The first "no CCD" control was not one.
3. **Measure fall-through against the area that has collision.** A radius test counted bodies
   sliding off the edge of the patch block; the harness now tests the patch cells themselves.
4. **The field sphere trace must bracket the crossing before bisecting** (B-004, docs/18).

## Findings so far

1. Per-patch triangle meshes from the visual vertices are a sound terrain collider for Avian:
   zero fall-through in 12 000 terrain-only drops up to 300 m/s with default CCD.
2. Fast dynamic–dynamic impacts near the ground can push bodies through (6 per 1 000 in the crowded
   test). Options for later: sweeping fast bodies against dynamic ones (`CcdFilter::ALL`), a small
   terrain collision margin, or a depenetration safety net against the field.
3. Bulk raycasts must use the engine's meshes (~85× faster); the field sphere trace is for rare,
   long rays beyond the collision area.
4. A local physics frame works at Earth radius exactly as on a 24 km body; body-local f32 does not.
5. Never-sleeping bodies creep on slopes; anything that must hold still while awake needs a
   kinematic or anti-creep treatment.

## Still to do

- Rapier: the same three tests.
- Character walking and jumping across patch seams and LOD changes (Avian `MoveAndSlide`), physics
  ms per frame in that test (pass ≤ 1 ms), and the user's feel check.
- Decisions and write-back to docs/18 and docs/20.
