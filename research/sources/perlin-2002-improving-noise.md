# Improving Noise

| | |
|---|---|
| **ID** | `perlin-2002-improving-noise` |
| **Type** | paper |
| **Authors / creator** | Ken Perlin |
| **Year** | 2002 |
| **Where** | SIGGRAPH 2002 (ACM Transactions on Graphics); reference implementation at https://mrl.cs.nyu.edu/~perlin/noise/ |
| **Accessed** | 2026-11-10 |
| **Licence / rights** | paper copyrighted; read-only |
| **Local copy** | none |
| **Topics** | pcg, noise |
| **Status** | distilled |
| **Distilled into** | [Noise and hashing](../../knowledge/pcg/noise-and-hashing.md) |
| **Used in** | `crates/genesis_core/src/noise.rs` |

## Why it matters

Defines the gradient-noise variant `genesis_core` implements: the interpolation curve and the
gradient set.

## Notes

Two fixes to Perlin's original 1985 noise:

- **Interpolation.** The original cubic blending curve `3t² − 2t³` has a discontinuous second
  derivative at cell boundaries, which shows up as visible creases when noise is used for
  displacement or normals. The quintic `6t⁵ − 15t⁴ + 10t³` has zero first and second derivatives at
  both ends, so the field is C² continuous.
- **Gradients.** Instead of random gradient vectors, use the 12 vectors from the cube centre to the
  midpoints of its edges. They are evenly distributed and avoid the clumping that random gradients
  produce along axes and diagonals. Padding the set to 16 (repeating four) lets a 4-bit hash pick
  one without a modulo.

## Takeaways

- Gradient noise is exactly zero at lattice points; it is band-limited around one cycle per cell.
- The quintic fade is what makes noise-derived normals smooth.
- A fixed, well-spread gradient set beats random gradients.

## Doubts / follow-ups

- Simplex noise (Perlin, 2001) scales better to higher dimensions and has fewer directional
  artefacts. Worth a card if 3D gradient noise shows grid alignment in E8 renders.
