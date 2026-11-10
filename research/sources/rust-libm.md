# libm — pure-Rust maths library

| | |
|---|---|
| **ID** | `rust-libm` |
| **Type** | code |
| **Authors / creator** | Rust project contributors; a port of the musl C library's `libm` |
| **Year** | ongoing |
| **Where** | https://crates.io/crates/libm (source in the `rust-lang` GitHub organisation) |
| **Accessed** | 2026-11-10 |
| **Licence / rights** | MIT (crate); musl's libm is MIT |
| **Local copy** | none |
| **Topics** | determinism, maths |
| **Status** | distilled |
| **Distilled into** | [Noise and hashing](../../knowledge/pcg/noise-and-hashing.md#floating-point-determinism) |
| **Used in** | `crates/genesis_core/src/math.rs`; [docs/20 determinism rules](../../docs/20-tech-stack-and-workspace.md#determinism-rules) |

## Why it matters

Gives the same transcendental-function results on every platform, which `std` does not guarantee.

## Notes

- `f32::sin` and friends in Rust's `std` call the platform's C maths library. Windows and Linux use
  different implementations, so the last bits of `sin`, `exp`, `pow`, … can differ.
- `libm` implements the functions in Rust, using only basic IEEE-754 operations, which are exactly
  specified. Same input, same bits, on every platform.
- Measured in [E1](../../docs/spikes/E1-field-cost.md#libm-versus-std): `sin` and `atan` are faster
  than `std` on Windows; `exp` is about 2.5× slower. Negligible at sample level.
- Confirmed in [E2](../../docs/spikes/E2-determinism.md): identical hashes on Windows and Linux.

## Takeaways

- Determinism needs bit-exact maths, and only basic arithmetic and `sqrt` are bit-exact by IEEE.
- A version change of `libm` could change results: treat it as a potential `generatorVersion`
  change and re-run golden tests.
