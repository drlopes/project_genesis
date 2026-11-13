# E8 — Classification cost and look

Experiment definition: [21 — Phase 1 Spike Plan](../21-phase-1-spike-plan.md#e8--classification-cost-and-look).
Status: **done.** The user picked *k* = 4, mosaic strength **0.9**, jitter 1×. The mosaic default
changes from 0.35; the reason is measured below.

| | |
|---|---|
| Date | 2026-11-13 |
| Harness | `spikes/field`, `e8` binary: `cargo run --release --bin e8 -- --out DIR [--overlap] [--only --k K --mosaic M --jitter J]`. Single-threaded |
| Body | the E1 spike field, seed 3, 24 km, palette 8 (7 families + barren fallback), 1 ecotone (families 1–2), 1 variant (family 1), *K* 4 |
| Maps | 20 × 20 km top-down window, 600 px (33 m/px), north up, gnomonic projection (edges a few per cent short); colour per family blended by weight, variant lighter, ecotone magenta, water dark blue. The window is the land site (≥ 80 % land) where the most families meet |
| Hardware | Ryzen 7 9800X3D, Windows 11 |

## The stimulus problem, and the second content variant

The spike's stub content spreads its families along temperature, each with its own moisture
range: two families are rarely both suitable at one place. The mosaic only acts where they are
(docs/19's "forest and grassland both fully suitable"), so with the stub as specified, the mosaic
row of the sweep showed almost nothing (class shares moved by about one point).

E8 therefore ran two variants:

- **Separated** (the stub as specified): judges sharpness *k* and jitter.
- **Overlap** (`--overlap`): families 2, 4, 6 take the climate envelope of 1, 3, 5, so each zone
  has two equally suitable competitors and only palette weight × mosaic decides between them.
  Judges the mosaic. Families 1–2 are also the ecotone pair.

## Results

### Cost

| Setting | L1 | L0–L2 | L2 increment |
|---|---|---|---|
| *k* 2 / 4 / 6 | 396–405 ns | 892–896 ns | 491–495 ns |
| mosaic 0.6 | 397 ns | 918 ns | 521 ns |
| jitter 2× | 400 ns | 902 ns | 503 ns |

**Pass.** L0–L2 is ~0.9 µs against the 1 µs budget. The settings change cost by ≤ 5 %: *k* is an
integer power, mosaic and jitter change values, not work. (E1 measured +644 ns for L2 at palette
16; this is palette 8.)

### Separated stub: sharpness and jitter

| *k* | mosaic | jitter | Transition (< 90 % dominant) | Border width | Patches | Patch p50 / p90 | Specks < 0.05 km² |
|---|---|---|---|---|---|---|---|
| 2 | 0.35 | 1× | 39 % | 690 m | 35 | 1.5 / 28 km² | 8 |
| **4** | 0.35 | **1×** | 25 % | **430 m** | 35 | 1.5 / 28 km² | 8 |
| 6 | 0.35 | 1× | 16 % | 290 m | 35 | 1.5 / 28 km² | 8 |
| 4 | 0.35 | 0.5× | 24 % | 440 m | 37 | 1.0 / 28 km² | 13 |
| 4 | 0.35 | 2× | 25 % | 390 m | 41 | 0.7 / 26 km² | 6 |

Border width = transition area / border length.

- ***k* changes only how soft the borders are.** Patches are identical at every *k*: power
  normalisation is monotonic, so the dominant family never changes. Width falls from 690 to 290 m.
- **Jitter has a small effect at these amplitudes**: at 2× a few islands appear along borders,
  border length +12 %. The amplitudes (2 K, 0.05, 60 m, 2°) are small next to the families'
  climate ranges, so jitter roughens borders without moving them.

### Overlap variant: mosaic strength

Palette weights of the competing pairs: 3 vs 4 = 0.907 vs 1.341 (×1.48); 5 vs 6 = 0.644 vs 0.964
(×1.50); 1 vs 2 = ×1.95 (not in the window).

| Mosaic | Weaker share, 3 of 3 + 4 | Weaker share, 5 of 5 + 6 | Patches | Patch p50 / p90 | Border width |
|---|---|---|---|---|---|
| 0.35 | **0 %** | **0 %** | 21 | 2.7 / 50 km² | 910 m |
| 0.6 | 7 % | 2 % | 34 | 0.40 / 46 km² | 640 m |
| **0.9** | **19 %** | **10 %** | 49 | 0.48 / 28 km² | 460 m |
| 1.2 | 27 % | 17 % | 54 | 0.55 / 10 km² | 370 m |
| 1.5 | 33 % | 22 % | 53 | 0.70 / 9 km² | 320 m |

At 0.35 the weaker family never dominates anywhere; it only tints its partner. That is the opposite
of the mosaic's purpose. Two causes:

1. **The mosaic noise uses about a third of its nominal range.** docs/19 says `n_F` is "in
   [−1, 1]". Measured over 200 000 points: one field has sd **0.27**, p99 0.59; the difference of
   two independent fields (what decides between two families) has sd **0.38**, p90 0.50, p99 0.89.
2. **The palette weights differ by ~1.5× between partners**, and `exp(0.35 · Δn)` rarely exceeds
   1.4×.

The weaker of two equally suitable families wins where
`strength · Δn > ln(paletteWeight ratio)`. With Δn ~ sd 0.38, a ratio of 1.5 needs strength ≈ 0.9
for a ~10–20 % share, which is what the table shows.

## Look test (the user, 2026-11-13)

Sheet `target/e8/pick.png` (not committed; regenerate with the harness): row 1 *k* 2 / 4 / 6
(separated), row 2 mosaic 0.35 / 0.9 / 1.5 (overlap), row 3 jitter 0.5 / 1 / 2× (separated).

> "The second tile of each row were the ones I felt were the most balanced."

## Decision

- ***k* = 4** (unchanged): borders ~430 m wide in this content.
- **mosaic strength 0.9** (was 0.35): the weaker of two competing families still forms groves and
  clearings (10–20 % at a 1.5× palette ratio) while the world's favoured family dominates.
- **jitter 1×** (unchanged): 2 K / 0.05 / 60 m / 2°, wavelength 2000 m.
- **mosaic scale 1500 m** (unchanged; not varied).
- docs/19 now states the noise's measured range and the sizing rule linking mosaic strength,
  *k*, and the palette spread. The palette draw's spread (lognormal σ) is still unspecified;
  `PROPOSED`: keep typical partner ratios ≤ ~1.6, so that no family that passes *palette_drop* is
  invisible next to a stronger competitor.

The spike's stub content keeps mosaic 0.35, so the E2 reference hash stays valid; E8 sets the
strength explicitly.

## Caveats

- One body, one 20 km window per variant; the overlap variant is synthetic. Real content will have
  partial overlaps, where the mosaic matters less than here.
- Grey (barren fallback) covers 10–35 % of the windows: climates no stub family covers. That is a
  content-coverage gap of the stub, not a classifier result; real packs need a coverage audit.
- Maps judge classification only, with label colours. Materials, scatter, and the ecotone's real
  look are later work.
