# E1 — Field cost

Experiment definition: [21 — Phase 1 Spike Plan](../21-phase-1-spike-plan.md#e1--field-cost).

| | |
|---|---|
| Date | 2026-11-10 |
| Commit | `380400c`, `61e6dc6` before the 2026-11-10 history rewrite (harness in `spikes/field`, run with `cargo run --release --bin e1` from `spikes/`) |
| Hardware | Ryzen 7 9800X3D, 32 GB, RTX 4060 (GPU unused), Windows 11 |
| Build | release, Rust 1.99.0 |
| Threads | 1 unless stated |
| Method | 50 000 random surface directions per run; median of 5 runs |

## What the harness is

A synthetic 24 km body whose **cost** follows the docs, not its looks:

- **L0** — baked 6 × 256 × 256 macro pyramid ([09](../09-surface-field-stack.md#the-macro-pyramid))
  plus analytic plate, crater (48 craters, no spatial index), and erosion terms.
- **L1** — the [05](../05-climate-and-hydrosphere.md) temperature and moisture terms, including the
  upwind `h0` sample for orographic lift and a cached water-distance grid.
- **L2** — the full [19](../19-biome-classification.md) classifier: slope from `h0`, value jitter,
  soft substrate, mosaic, soft budget, ε fallback, power normalisation, one ecotone, one variant.
  No region index: every palette family is a candidate, which overstates real cost.
- **L3** — band-limited suites with soft context. **L4** — band-limited micro detail.
- Not included: normals, mesh building, the benthic classifier, and scatter.

## Results

### Before and after the optimisation pass

The first run missed the L0–L2 budget by 2.7×. Profiling showed that a single noise evaluation cost
119 ns, and noise is used everywhere (fBm octaves, jitter, mosaic, substrate). The fix: one hash mix
per lattice corner instead of three, and a 16-entry gradient table instead of a branch on `% 12`.

| | Before | After | Budget |
|---|---|---|---|
| one `gradient3` | 119 ns | **14 ns** | — |
| `h0` | 326 ns | **166 ns** | — |
| `classify` (palette 16) | 1100 ns | **265 ns** | — |
| L0–L2 sample | 2 696 ns | **1 054 ns** | ≤ 1 000 ns |
| L0–L4 sample | 3 529 ns | **1 182 ns** | ≤ 4 000 ns |

### Cost per band (palette 16, suites 4, K 4)

| Band | Spacing | Layers | ns / sample | 65×65 patch, 1 thread |
|---|---|---|---|---|
| 0 | 4000 m | L0 | 165 | 0.70 ms |
| 1 | 1000 m | L1 | 413 | 1.74 ms |
| 2 | 50 m | L0–L3 | 1 106 | 4.67 ms |
| 3 | 4 m | L0–L3 | 1 115 | 4.71 ms |
| 4 | 0.5 m | L0–L4 | 1 195 | 5.05 ms |

### Layer breakdown at band 4

| Layer | Increment | Notes |
|---|---|---|
| L0 | +168 ns | pyramid 32 ns, 3 noise 43 ns, crater loop ≈ 90 ns |
| L1 | +242 ns | includes one extra `h0` (upwind) |
| L2 | +644 ns | two extra `h0` for slope 331 ns, five noise 72 ns, `classify` 265 ns |
| L3 | +77 ns | 4 suites |
| L4 | +51 ns | 3 micro octaves |

`h0` is evaluated **four times** per sample, about 660 ns of the 1 182 ns total.

### Content sweeps (band 4, full stack)

| Palette | Suites | K | ns / sample |
|---|---|---|---|
| 8 | 4 | 4 | 1 117 |
| 16 | 4 | 4 | 1 187 |
| 32 | 4 | 4 | 1 450 |
| 32 | 4 | 8 | 1 377 |
| 16 | 2 | 4 | 1 132 |
| 16 | 8 | 4 | 1 263 |

Classification sanity (palette 16): **0** of 50 000 samples had family weights not summing to 1;
mean active families (weight > 10⁻⁴) **2.69**, maximum **5**.

### Thread scaling (band 4, full stack, palette 16)

| Worker threads | Samples / s | 65×65 patches / s |
|---|---|---|
| 1 | 0.85 M | 202 |
| 4 | 3.27 M | 774 |
| 8 | 6.33 M | 1 499 |
| 16 (SMT) | 5.61 M | 1 328 |

### `libm` versus `std`

| Function | `libm` | `std` |
|---|---|---|
| `sin` | 2.61 ns | 3.40 ns |
| `exp` | 3.85 ns | 1.51 ns |
| `atan` | 2.09 ns | 2.73 ns |

## Decisions

1. **Sample budgets: pass.** L0–L4 is 3.4× under budget. L0–L2 is at budget (1.05 µs) with two
   known, unoptimised costs still in it (below). Keep both budgets as they are.
2. **Cost is driven by noise and repeated `h0` evaluations, not by content count.** Going from 8 to
   32 biomes adds 30%. [09](../09-surface-field-stack.md#performance-budget) is corrected.
3. **Craters need a spatial index at query time.** The 48-crater linear loop is already half of
   `h0`; an ancient world will have thousands. Mandatory, recorded in
   [06](../06-geology-and-landforms.md#impacts-and-cratering).
4. **`K = 4` stands.** The mean number of active families is 2.7 and the maximum 5, so the soft
   budget rarely binds and costs almost nothing ([19](../19-biome-classification.md#tuning-constants)).
5. **Generation workers = physical cores.** SMT made throughput *worse* (16 threads slower than 8).
   Size the generation pool to physical cores; E4 confirms under load
   ([10](../10-lod-and-streaming.md#asynchronous-generation)).
6. **`libm` stays** (determinism rule 1). Its `exp` is 2.5× slower than `std`, but a sample calls
   only a handful of transcendental functions; the cost is noise in the totals.
7. **Bake: pass.** 62 ms single-threaded for the pyramid and caches, against a 2 s budget.

## Open optimisations (not needed yet)

- **Classification-input cache.** Slope and the orographic upwind term are low-frequency inputs that
  cost three extra `h0` evaluations (about 500 ns). Baking them on the macro grid would keep LOD
  independence and roughly halve L0–L2. `PROPOSED`; adopt if E4 needs the throughput.
- **Multi-channel noise.** The four jitter axes could share one lattice hash with different
  gradient bits.
- **Region index.** The harness scores every palette family; the real conservative index only
  removes work.

## Caveats

- Stub content: synthetic biome ranges and suites. Real content may activate more families per
  sample; the palette sweep bounds the effect.
- No normals or meshing: patch times are field-evaluation times only. E3/E4 measure the rest.
- Single machine, one CPU. The 4-worker row is the figure that counts (see
  [21](../21-phase-1-spike-plan.md#reference-hardware-and-budgets)).
