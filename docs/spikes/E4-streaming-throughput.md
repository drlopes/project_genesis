# E4 — Streaming throughput

Experiment definition: [21 — Phase 1 Spike Plan](../21-phase-1-spike-plan.md#e4--streaming-throughput).

| | |
|---|---|
| Date | 2026-11-11 |
| Harness | `spikes/app` (Bevy 0.20.1) as left by E3 and I-005: grid 65, SSE 16, 0.5 s geomorphing, 32-byte vertices. Run `spike_app --script --path straight --speed <m/s> --alt <m> --workers <n> --out <file>` |
| Hardware | Ryzen 7 9800X3D (8 cores), 32 GB, RTX 4060 8 GB, Windows 11; window 2560 × 1440, vsync off |
| Path | great circle at constant height above the coarse ground (or sea surface), looking at the ground ahead; 5 s holding still (initial fill), then 60 s at constant speed |
| Raw data | `spikes/app/results/e4-*.json` (summaries, committed); per-frame CSV not committed |

## What is measured

Per frame:

- **Pending:** wanted patches that are not yet generated and integrated. This is the throughput
  measure.
- **Deficit:** wanted patches not shown at their own level. It includes pending patches, plus
  patches held back on purpose: a parent finishing its 0.5 s morph-in keeps its area, and a patch
  integrated this frame shows the next frame. The deficit is therefore never zero while moving,
  even with unlimited throughput.
- **Holes:** wanted patches with nothing displayed: no ready ancestor and no ready descendants.
- **Requests and completions:** patches first wanted, and patches integrated.

Per run (moving phase only):

- **Completion ratio:** completions / requests. Below 1, the camera abandons requests (it has
  moved on before they are built).
- **Backlog** (queued + in flight): max, and its least-squares trend. A bounded trend alone does not
  mean the workers keep up, because abandoned requests leave the queue (see the 1000 m/s smoke run
  below), so the verdict rests on holes and the completion ratio.
- **Latency:** from a patch first being wanted to its integration.
- **Initial fill:** time from start until every wanted patch is shown at its own level.

## Harness lessons

1. **The integration budget must guarantee progress.** The 2 ms main-thread budget covers
   selection and integration together. When selection alone used it up, the loop integrated
   nothing, ever. The app now always integrates at least one finished patch per frame.
   (Production rule for [docs/10](../10-lod-and-streaming.md#frame-budget-targets).)
2. **Defaults must match decisions.** The app still defaulted to SSE 2 from the E3 plan; a run
   that relied on the default selected ~20 000 patches. Default is now SSE 16.

## Results

All 16 runs valid (no disturbed frames). Moving phase, 60 s.

| Workers | Alt | Speed | Requests/s | Completions/s | Completion ratio | Backlog max | Pending p99 | Latency p50 / p99 | Initial fill | Frame p99 / p99.9 | Main p99 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 4 | 50 m | 5 m/s | 9.3 | 9.3 | 1.00 | 9 | 4 | 7 / 9 ms | 1.99 s | 3.4 / 10.0 ms | 1.03 ms |
| 4 | 50 m | 30 m/s | 56.8 | 56.8 | 1.00 | 12 | 4 | 7 / 12 ms | 1.89 s | 3.3 / 3.6 ms | 1.10 ms |
| 4 | 50 m | 200 m/s | 282 | 282 | 1.00 | 29 | 12 | 7 / 32 ms | 1.88 s | 3.5 / 10.5 ms | 1.12 ms |
| 4 | 50 m | 1000 m/s | 1 348 | 621 | **0.46** | 1 505 | 1 470 | 16 / 157 ms | 1.89 s | 2.6 / 3.3 ms | 1.26 ms |
| 4 | 500 m | 5 m/s | 1.0 | 1.0 | 1.00 | 8 | 0 | 6 / 7 ms | 1.31 s | 3.1 / 8.2 ms | 0.70 ms |
| 4 | 500 m | 30 m/s | 5.7 | 5.7 | 1.00 | 8 | 1 | 6 / 8 ms | 1.31 s | 3.2 / 3.6 ms | 0.74 ms |
| 4 | 500 m | 200 m/s | 31.8 | 31.8 | 1.00 | 12 | 4 | 6 / 10 ms | 1.29 s | 3.2 / 6.5 ms | 0.75 ms |
| 4 | 500 m | 1000 m/s | 160.8 | 160.7 | 1.00 | 17 | 8 | 7 / 17 ms | 1.29 s | 3.5 / 3.9 ms | 0.79 ms |
| 8 | 50 m | 5 m/s | 9.3 | 9.3 | 1.00 | 10 | 4 | 6 / 9 ms | 1.05 s | 3.3 / 9.1 ms | 1.03 ms |
| 8 | 50 m | 30 m/s | 56.9 | 56.9 | 1.00 | 16 | 4 | 7 / 8 ms | 0.98 s | 3.3 / 4.3 ms | 1.13 ms |
| 8 | 50 m | 200 m/s | 282 | 282 | 1.00 | 29 | 9 | 7 / 15 ms | 1.04 s | 5.2 / 12.6 ms | 1.11 ms |
| 8 | 50 m | 1000 m/s | 1 339 | 1 179 | **0.88** | 1 144 | 1 028 | 22 / 721 ms | 0.99 s | **49.3 / 70.8 ms** | 1.32 ms |
| 8 | 500 m | 5 m/s | 1.0 | 1.0 | 1.00 | 8 | 0 | 7 / 7 ms | 0.70 s | 3.3 / 6.9 ms | 0.69 ms |
| 8 | 500 m | 30 m/s | 5.7 | 5.7 | 1.00 | 11 | 1 | 6 / 8 ms | 0.68 s | 3.3 / 3.9 ms | 0.74 ms |
| 8 | 500 m | 200 m/s | 31.8 | 31.8 | 1.00 | 16 | 4 | 6 / 10 ms | 0.71 s | 3.2 / 8.5 ms | 0.75 ms |
| 8 | 500 m | 1000 m/s | 160.8 | 160.7 | 1.00 | 21 | 8 | 7 / 10 ms | 0.68 s | 3.8 / 7.7 ms | 0.79 ms |

### The "holes" were invisible

The summaries also count hole frames (a wanted patch with nothing displayed): from 0 at 5–30 m/s
up to 34 690 at 1000 m/s, 50 m, 4 workers, and ~30 at 200 m/s with either worker count. A
diagnostic (`E4_HOLE_LOG=<seconds>` logs each hole after that time) located every one of them in
the moving phase:

| Run (20 s) | Hole lines while moving | Where |
|---|---|---|
| 500 m, 1000 m/s, 4 workers | 72 | all level 2 (9.4 km patches), 43.8–45.0 km away; camera horizon 9.4 km |
| 50 m, 200 m/s, 4 workers | 0 | — |
| 50 m, 1000 m/s, 4 workers | 52 827 | all level 2, 34.8–43.9 km away; camera horizon ~3 km |

They are coarse patches on the far side of the body. The horizon cull is conservative (it inflates
a patch by its size plus the body's whole relief), so these pass it, become wanted, and queue last
under nearest-first ordering; when the workers saturate they starve. **No visible hole occurred in
any run.** The hole metric must exclude patches beyond the visible range (captured with a tighter
cull as I-009).

### What saturation looks like

At 1000 m/s and 50 m with 4 workers, half the requests are abandoned (the camera has passed before
they are built) and the visible symptom is **detail lag**: terrain stays at a coarser level than
wanted, with request-to-integration latency up to 157 ms at p99. There are no visible holes, because
coarser patches cover; coverage held even here.

## Analysis

- **Generation capacity in the app:** ~620 patches/s with 4 workers, ~1 180/s with 8 (×1.9). E1
  measured 774 patches/s for the field alone at 4 workers; the app also samples the parent level for
  morphing and encodes vertices.
- **Load scales with speed over height.** Requests per metre travelled: ~1.4 at 50 m (282/s at
  200 m/s, 1 348/s at 1000 m/s), ~0.16 at 500 m. With 4 workers, full detail is sustained up to an
  estimated **~450 m/s at 50 m** (capacity / load) and was measured at 200 m/s with a completion ratio
  of 1.00 and latency p99 of 32 ms.
- **Worker count: 8 workers on 8 cores break frame pacing under load.** At 1000 m/s and 50 m, frame
  p99 rose to 49 ms and p99.9 to 71 ms (budget 16.6 / 33 ms), while the main thread's terrain work
  stayed at 1.3 ms: generation threads on every core starve the main and render threads. Four
  workers stayed at 2.6 / 3.3 ms in the same run. Below saturation, 8 workers only halve the initial
  fill (0.7–1.05 s against 1.3–2.0 s). E1's "workers = physical cores" rule measured the field alone;
  inside the app it is wrong.
- **Main-thread budget:** terrain work p99 ≤ 1.32 ms in every run (budget 2 ms).

## Decisions

1. **Pass.** At rover speed (30 m/s) with 4 workers the backlog stays bounded (max 12), completions
   equal requests, latency p99 is 12 ms, and no hole occurs.
2. **Speed cap for full detail near the surface: speed / height above ground ≤ ~6 per second**
   (4 workers, reference hardware), about two thirds of the estimated saturation (~9 per second at
   50 m). Measured points: 200 m/s at 50 m (ratio 4) and 1000 m/s at 500 m (ratio 2) keep full
   detail; 1000 m/s at 50 m (ratio 20) does not. Walking and rover speeds use under 10% of
   capacity. Above the cap, terrain degrades to coarser detail, not to holes.
3. **Generation workers: never all physical cores.** E1's "workers = physical cores" (field alone)
   is replaced by the docs/21 thread model, physical cores minus two (main and render threads),
   now backed by measurement: all 8 cores failed the frame budget under saturation, 4 passed. 6
   (cores − 2) is untested; 4 is the verified default on the reference machine. Revisit when
   physics (E6) and audio add threads.
4. **The main-thread integration budget must guarantee progress** (at least one patch per frame).
5. **Ideas:** I-008 (coverage-first priority) stays open, with its evidence corrected: saturation
   showed detail lag, not visible holes; it still matters for arrival after fast travel. New I-009:
   tighter horizon cull. I-001, I-002 (field cost), I-004 (view-dependent LOD), and I-006 (per-patch
   overhead) are not needed at Phase 1 speeds: parked.
