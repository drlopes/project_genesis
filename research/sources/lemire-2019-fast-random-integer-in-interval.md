# Fast Random Integer Generation in an Interval

| | |
|---|---|
| **ID** | `lemire-2019-fast-random-integer-in-interval` |
| **Type** | paper |
| **Authors / creator** | Daniel Lemire |
| **Year** | 2019 |
| **Where** | ACM Transactions on Modeling and Computer Simulation; preprint arXiv:1805.10941 |
| **Accessed** | 2026-11-10 |
| **Licence / rights** | copyrighted; preprint freely readable |
| **Local copy** | none |
| **Topics** | rng |
| **Status** | distilled |
| **Distilled into** | [Noise and hashing](../../knowledge/pcg/noise-and-hashing.md) |
| **Used in** | `crates/genesis_core/src/rng.rs` (`below`); see bug B-001 |

## Why it matters

How to turn a uniform 64-bit value into a uniform integer in `[0, n)` without the bias and the
division cost of `x % n`.

## Notes

- Multiply the 64-bit random value by `n` as a 128-bit product; the high 64 bits are the result.
  This maps `[0, 2^64)` onto `[0, n)` with one multiplication.
- On its own, the multiply-shift is **slightly biased**: some outputs receive one more input than
  others, a relative bias of at most `n / 2^64`.
- The paper's contribution is a cheap **rejection** step that makes the result exactly uniform: if
  the low 64 bits of the product fall below `2^64 mod n`, draw again. The check almost never fires,
  and the expensive modulo is only computed when it might.

## Takeaways

- Multiply-shift without rejection: fast, bias ≤ `n / 2^64`, fine for procedural content.
- With rejection: exactly uniform, still fast. With a counter-based RNG, "draw again" means using a
  second key, which must be part of the defined behaviour.
- Our `below` implements the first, but its comment claims the second (B-001).
