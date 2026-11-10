# E3 — LOD bands and quadtree

Experiment definition: [21 — Phase 1 Spike Plan](../21-phase-1-spike-plan.md#e3--lod-bands-and-quadtree).
Also records the first E9 build measurements, because they come from the first Bevy build.

| | |
|---|---|
| Date | 2026-11-10 |
| Harness | `spikes/app` (Bevy 0.20.1), field from `spikes/field`; run `spike_app --script --out <file>` |
| Hardware | Ryzen 7 9800X3D, 32 GB, RTX 4060 8 GB, Windows 11; window 2560 × 1440, vsync off |
| Workers | 4 (the docs/21 cap) |
| Path | scripted, frame-rate independent: 60 s exponential descent from 100 km to 30 m altitude while drifting along the surface, then a 30 s skim at 30 m altitude and 30 m/s |
| Raw data | `spikes/app/results/*.json` (summaries, committed). Per-frame `*.csv` dumps are written next to them but not committed (17.7 MB for this experiment); the analyses that used them are recorded below. Re-run a configuration to regenerate. |

## What the app does

- Cube-sphere quadtree; node level ↔ LOD band per [docs/10](../10-lod-and-streaming.md#quadtree-level-versus-lod-band).
  A unit test checks the docs/10 table (24 km body, 65 × 65 grid: levels 0/4/8/11 → bands 1/2/3/4).
- **Selection** by screen-space error: split when one sample spacing projects to more than *SSE*
  pixels, measured to the patch centre on the terrain; hysteresis (merge below ½ SSE); horizon
  culling against the body's lowest terrain sphere.
- **Generation** on a fixed pool of worker threads with a nearest-first queue re-prioritised every
  frame. Patches are meshed from the E1 field at their band (normals from an extra ring of samples,
  skirts against cracks).
- **Integration** on the main thread within a 2 ms budget; stale results are discarded unopened.
- **Display** never shows a hole or an overlap: a wanted patch that is not ready is covered by its
  nearest ready ancestor, or by its ready descendants when zooming out.

## Spike bugs found on the way

Recorded because each is a design lesson for the production streamer
([docs/10](../10-lod-and-streaming.md#streaming)):

1. **Fixed relief margin in the distance estimate.** A constant 2.5 km vertical margin made every
   patch within 2.5 km look 1 m away, so the whole neighbourhood subdivided to the deepest level
   (~200 000 queued patches). Fix: measure to the patch centre on the terrain, with a margin that
   scales with patch size.
2. **Re-queueing in-flight work.** Patches finished but still in the result channel looked missing
   and were queued again, so workers rebuilt them repeatedly; the backlog reached 13 GB of RAM.
   Fix: track in-flight patches and never re-queue them.
3. **O(n²) display logic.** Ancestor and descendant searches over all resident patches took 15–60 ms
   per frame at a few thousand patches. Fix: tree walks with hash lookups, O(patches × depth).
4. **Integrating stale results.** By the time a queued result is integrated, the camera may no
   longer want it. Fix: discard on receipt.

## Results

### Static patch counts

Selected patches on a flat 24 km sphere at three altitudes (the `patch_counts` test; no GPU):

| Grid | SSE | 30 m | 1 km | 100 km |
|---|---|---|---|---|
| 33 | 8 | 2 216 | 1 348 | 36 |
| 65 | 2 | 5 124 (44.6 M tris) | 3 620 | 96 |
| 65 | 4 | 2 012 (17.5 M tris) | 1 348 | 36 |
| 65 | 8 | 860 (7.5 M tris) | 532 | 9 |
| 65 | 16 | 408 (3.6 M tris) | 228 | 6 |
| 129 | 8 | 372 (12.6 M tris) | 228 | 6 |

Each doubling of SSE cuts patches and triangles by about 2.5×. With a sample-spacing error metric,
patch edge ends up at about `d / (10 · SSE / 2)` for distance `d`, so the plan's candidate values of
1–4 px are far too dense (≈1 M on-screen vertices at 2 px).

### First GPU pass (before fixes 2–4)

| Run | Frame p50 / p99 / p99.9 | Main p99 | Max patches | Missing-patch frames | RAM |
|---|---|---|---|---|---|
| grid 65, SSE 2 | 5.7 / 225 / 252 ms | 215 ms | 4 962 | 0 | 9.0 GB |
| grid 65, SSE 4 | 4.1 / 66 / 84 ms | 62 ms | 6 149 | 777 | 7.1 GB |
| grid 65, SSE 8 | 3.2 / 6.1 / 9.8 ms | 1.7 ms | 3 936 | 0 | 0.47 GB |
| grid 65, SSE 16 | 2.7 / 4.3 / 6.4 ms | 0.8 ms | 1 659 | 0 | 0.46 GB |
| grid 33, SSE 8 | 2.8 / 16.8 / 18.1 ms | 15.7 ms | 6 341 | 2 865 | 13.4 GB |
| grid 129, SSE 8 | 3.8 / 7.0 / 12.3 ms | 0.8 ms | 1 464 | 0 | 0.48 GB |

### Where the load peaks (grid 65, SSE 8, after fixes)

| Altitude | Max displayed patches | Missing-patch frames |
|---|---|---|
| > 50 km | 75 | 0 |
| 5–50 km | 788 | 0 |
| 0.5–5 km | 1 877 | 0 |
| 60–500 m | 3 936 | 0 |

The peak is near the ground, about 4.5× the flat-sphere estimate: relief brings slopes much closer
than the camera's height above its own point, and selection ignores view direction, so patches
behind the camera are generated at full detail (captured as idea I-004).

### Final sweep (all fixes in)

VRAM is the whole-GPU figure from `nvidia-smi` at the end of the run minus the baseline at start-up,
so it includes Bevy's own allocations, not only terrain. It is a single end-of-run snapshot, not a
peak: the same configuration (grid 65, SSE 8) measured 2.0 GB here and 2.8 GB in the first pass.
Treat VRAM figures as ±40% until the harness samples peak usage during the run.

| Run | Body | Frame p50 / p99 / p99.9 | Main p99 | Max patches | Max tris | Missing-patch frames | RAM | VRAM used by the run |
|---|---|---|---|---|---|---|---|---|
| grid 33, SSE 8 | 24 km | 2.8 / 8.2 / 14.7 ms | 7.2 ms | 5 954 | 13.8 M | **5 027** | 2.8 GB | 1.3 GB |
| grid 65, SSE 8 | 24 km | 3.5 / 7.0 / 16.9 ms | 1.9 ms | 3 915 | 34.1 M | 0 | 0.43 GB | **2.0 GB** |
| grid 65, SSE 16 | 24 km | 2.8 / 4.9 / 7.0 ms | 0.9 ms | 1 659 | 14.4 M | 0 | 0.43 GB | 1.6 GB |
| grid 65, SSE 16 | 8 km | 2.3 / 4.6 / 5.7 ms | 0.7 ms | 1 353 | 11.8 M | 0 | 0.46 GB | 1.3 GB |
| grid 65, SSE 16 | 40 km | 2.8 / 4.4 / 6.5 ms | 0.8 ms | 1 561 | 13.6 M | 0 | 0.43 GB | 1.6 GB |

**Disturbed runs, rejected.** The first attempts at grid 65 / SSE 8 / 40 km and SSE 16 with band
spacing 0.5× and 2× were invalid: from 34–42 s into each run, 11–14% of frames were pinned at
16.7 ms (the OS throttling an unfocused window to 60 Hz), and in the 0.5× run the window was
minimised, which reported a height of 0 and collapsed the selection to the six root patches. The
harness now keeps the last valid projection scale and records `disturbed_frames` and `valid` in
every results file.

Re-runs:

| Run | Valid | Frame p50 / p99 / p99.9 | Main p99 | Max patches | Max tris | Missing-patch frames | RAM |
|---|---|---|---|---|---|---|---|
| grid 65, SSE 8, 40 km body | yes | 3.4 / 5.2 / 6.0 ms | 1.6 ms | 3 687 | 32.1 M | 4 | 0.44 GB |
| grid 65, SSE 16, band spacing 2× | yes | 2.5 / 4.0 / 5.8 ms | 0.6 ms | 1 464 | 12.8 M | 0 | 0.42 GB |
| grid 65, SSE 16, band spacing 0.5× | **no** (463 disturbed frames) | timings invalid | 0.8 ms | 1 818 | 15.8 M | 0 | 0.43 GB |

The 0.5× run was disturbed again, and the flag caught it. Its patch count is still valid, because
selection no longer depends on the window: band spacing 0.5× needs ~10% more patches than 1×
(1 659), and 2× ~12% fewer. Band spacing only moves the deepest level and the band mapping, so its
cost effect is small either way; the choice between them is about look, not budget. Not re-run
again (stop rule).

### Peak VRAM

The harness samples `nvidia-smi` once a second during the run and keeps the peak; the figure is the
peak minus the baseline measured at start-up. It includes Bevy's own GPU allocations, so terrain
alone is somewhat lower. The baseline itself varied (1.3–2.2 GB) with other applications.

| Run | Valid | Frame p99 / p99.9 | Main p99 | Max patches | Peak VRAM above baseline |
|---|---|---|---|---|---|
| grid 65, SSE 8, rep 1 | no (timings) | — | 1.7 ms | 3 936 | 2 768 MiB |
| grid 65, SSE 8, rep 2 | yes | 6.1 / 9.9 ms | 1.9 ms | 3 936 | 2 715 MiB |
| grid 65, SSE 16, rep 1 | yes | 3.9 / 5.8 ms | 0.7 ms | 1 599 | 1 951 MiB |
| grid 65, SSE 16, rep 2 | yes | 4.4 / 6.6 ms | 0.9 ms | 1 659 | 1 645 MiB |

With the current 40-byte vertex format, **SSE 8 exceeds the 2 GB terrain budget (~2.7 GB);
SSE 16 meets it (1.6–2.0 GB)**. Peak total GPU memory stayed under the 6 GB cap in every run.

## E9 — build iteration

Definition: [docs/21 E9](../21-phase-1-spike-plan.md#e9--build-iteration). Dev profile, one-line
change, wall-clock build time (launch excluded).

| Build | Time |
|---|---|
| Cold dev build of `spike_app` with Bevy 0.20.1, including downloads | 393 s |
| Full rebuild after changing the linker (the linker is part of every artefact's fingerprint) | 374 s |

| Setup | App crate change | Field crate change (stands in for `genesis_gen`) |
|---|---|---|
| Default MSVC `link.exe`, static Bevy | 5.7–14.9 s (noisy across sessions) | 6.7–9.1 s |
| `rust-lld`, static Bevy | **5.9–6.0 s** | **6.0–7.4 s** |
| Bevy `dynamic_linking` | not available, see below | — |
| Targets (docs/21) | ≤ 20 s | ≤ 15 s |

**Bevy `dynamic_linking` cannot be enabled on 0.20.1.** `bevy 0.20.1` requires
`bevy_dylib ^0.20.1`, and only `bevy_dylib 0.20.0` is published on crates.io (checked
2026-11-10). Testing it would mean downgrading to 0.20.0 and a full rebuild; not worth it while
static linking meets both targets. Revisit at the next Bevy upgrade (I-007).

Decision: **static Bevy with `rust-lld`** on Windows, configured in the repository's
`.cargo/config.toml` by name, so it works on any machine with the pinned toolchain. Verified: the
by-name configuration builds both workspaces (full rebuild 375 s), incremental builds measured
6.0–6.3 s for both crates over three repetitions, root tests and clippy pass, and E2 still
matches its recorded hashes.

## Decisions

Measured, therefore decided:

1. **Patch grid: 65 × 65** (64 intervals). Grid 33 fails on the main thread: each patch has a fixed
   main-thread cost (entity, mesh asset, draw call), and grid 33 needs ~1.6× the patches for the
   same detail, which left thousands of frames with holes. Grid 129 works but carries 3–4× the
   triangles of grid 65 at the same patch count. The docs/10 example value stands.
2. **The error metric is "one sample spacing in screen pixels", and its useful range is 8–16 px,**
   not the 1–4 px the plan proposed. At 2 px the near-ground cut is ~5 000 patches and ~45 M
   triangles. Each doubling of the threshold cuts patches and triangles by ~2.5×.
3. **Band spacings stay as in docs/10.** Scaling them by 0.5× or 2× changes patch counts by about
   ±10%; it is a look decision, not a budget one.
4. **Body size does not change the budget picture.** 8, 24, and 40 km bodies behave alike at the
   same threshold, because the error metric is screen-space.
5. **Skirts are sufficient for cracks** at the measured thresholds: no hole frames at SSE 16 on any
   body. Visual confirmation is the user's check below.
6. **Streaming design lessons** (the four spike bugs) go into docs/10: distance to the patch on the
   terrain with a size-scaled margin, in-flight tracking, tree-walk display logic, and discarding
   stale results.
7. **Main-thread integration rate, not worker throughput, is the first ceiling** as patch counts grow.
   Per-patch overhead is captured as I-006.

Depends on the user's visual check:

8. **Default error threshold.** SSE 16 meets every budget with the current vertex format. SSE 8
   needs the compressed vertex format (I-005) to meet the terrain VRAM budget. If SSE 16 looks good
   enough in the fly-in, it becomes the default and I-005 stays optional; if it pops visibly,
   SSE 8 becomes the default and I-005 is accepted as a Phase 1 requirement.

## User check

Watch the fly-in at both thresholds and rate popping (patches visibly changing detail) and cracks
(gaps between patches):

```sh
cd spikes
cargo run --release -p spike_app -- --script --sse 16
cargo run --release -p spike_app -- --script --sse 8
```

Interactive flight: `cargo run --release -p spike_app -- --sse 16` (right mouse to look, WASD to
move, Q/E down/up, Shift faster, F starts the scripted fly-in).

Result: PENDING.
