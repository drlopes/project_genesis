# E5 — Coordinate precision

Experiment definition: [21 — Phase 1 Spike Plan](../21-phase-1-spike-plan.md#e5--coordinate-precision).

| | |
|---|---|
| Date | 2026-11-12 |
| Harness | `spikes/app` (Bevy 0.20.1): `--system-distance <m> --frame relative\|naive`; emulation in `spikes/app/src/precision.rs` |
| Hardware | Ryzen 7 9800X3D, 32 GB, RTX 4060 8 GB, Windows 11; window 2560 × 1440 |
| Raw data | `spikes/app/results/e5-*.json`; screenshots not committed |

## Candidates

| Candidate | Status for Bevy 0.20 (checked 2026-11-12) |
|---|---|
| `big_space` | only `0.14.0-rc.0`, built against Bevy 0.20.0-rc.2; no stable release a month after Bevy 0.20 |
| Our own camera-relative frame | ~50 lines in the spike: positions in f64, render translations `(entity − camera)` computed in f64 then cast to f32, camera at the origin |
| Naive (control) | system positions cast to f32, which is Bevy's default use of `Transform` |

The control exists to prove that the test detects the failure: if the naive frame did not break,
the test would be measuring nothing.

## Method

1. **Emulation** (`relative_frame_is_subpixel_at_system_scale_and_naive_is_not`): the GPU's
   single-precision vertex path (model translation + local offset, minus camera translation,
   rotation, projection) emulated in f32, against an exact f64 projection. Camera 1.7 m above the
   ground walking 10 m; probe points on the ground 2–2 000 m ahead; patch-sized entities. Reported:
   worst screen error in pixels at 1 440 px height and 60° field of view.
2. **In the app:** the body placed at 0 / 10⁶ / 10¹⁰ m from the system origin; the scripted E3
   fly-in (orbit to 30 m skim) with screenshots.

## Results

### Emulated screen error (worst over the walk)

| System distance | Naive frame | Relative frame |
|---|---|---|
| 0 m | 0.81 px | 0.0014 px |
| 10⁴ m | 0.81 px | 0.0014 px |
| 10⁶ m | 36.8 px | 0.0014 px |
| 10⁸ m | 4 533 px | 0.0013 px |
| 10⁹ m | 34 553 px | 0.0014 px |
| 10¹⁰ m | 34 632 px | 0.0006 px |

The naive frame already fails at 10⁶ m (1 000 km). Note the 0.81 px at 0 m: even with the body at
the origin, a camera 24 km from it puts ground 2 m away off by almost a pixel. The relative frame
removes that too.

### In the app

| Run | Valid | Frame p99 / p99.9 | Main p99 | Screenshots |
|---|---|---|---|---|
| relative, 0 m | yes | 7.1 / 15.6 ms | 1.30 ms | as before |
| relative, 10¹⁰ m | yes | 5.5 / 10.4 ms | 1.38 ms | identical to 0 m |
| naive, 10⁶ m | yes | 4.8 / 7.1 ms | 1.06 ms | look correct when still; the error is frame-to-frame jitter (~2.5 px at 30 m altitude) |
| naive, 10¹⁰ m | no (disturbed) | — | — | **terrain shredded**: vertices snap to the ~1 km spacing of f32 at that distance |

The relative frame costs ~0.3 ms of main thread per frame (re-placing ~1 700 patch transforms),
within the 2 ms budget.

## Findings for production

1. **Flight and physics maths must not run in f32 body-local coordinates on large bodies.** The
   spike's free-flight code moves the camera in f32 body-local space: fine on a 24 km body (2 mm
   steps), but ~0.5 m steps on an Earth-sized one, which would make walking stutter. Rendering is
   unaffected (it differences in f64). Carried into E6.
2. **Physics is moved to E6.** With physics in a local frame around the player (docs/10), the
   system distance cannot affect it by construction; the real physics precision risk is local
   coordinates on large bodies, which belongs with E6's engine choice. Candidates seen: `avian3d`
   0.8 (Bevy 0.20, `f64` feature) and `bevy_rapier3d` 0.37 (Bevy 0.20, f32 only).

## Decisions

1. **Pass at 10¹⁰ m with our own camera-relative frame**: emulated error ≤ 0.0014 px at every
   distance; screenshots at 10¹⁰ m identical to the origin. Visual check by the user: PENDING.
2. **No `big_space`.** Our frame passes and is small; adopting a pre-release dependency for it is
   not justified. Revisit only if we need what it adds (hierarchical grids for nested reference
   frames).
3. **Rule: every render translation is computed as a difference of f64 positions, then cast.**
   Never cast a system or body-local position to f32 first.
4. **The resting-body physics test moves to E6** (plan change; see findings).
