# Fast Splittable Pseudorandom Number Generators (SplitMix)

| | |
|---|---|
| **ID** | `steele-2014-splitmix` |
| **Type** | paper (plus reference code) |
| **Authors / creator** | Guy L. Steele Jr., Doug Lea, Christine H. Flood; mixer constants by David Stafford |
| **Year** | 2014 |
| **Where** | OOPSLA 2014. C reference of SplitMix64 by Sebastiano Vigna: https://prng.di.unimi.it/splitmix64.c. Stafford's mixer variants: "Better Bit Mixing — Improving on MurmurHash3's 64-bit Finalizer" (blog, 2011) |
| **Accessed** | 2026-11-10 |
| **Licence / rights** | paper copyrighted; Vigna's reference code is public domain |
| **Local copy** | none |
| **Topics** | pcg, rng, hashing |
| **Status** | distilled |
| **Distilled into** | [Noise and hashing](../../knowledge/pcg/noise-and-hashing.md) |
| **Used in** | `crates/genesis_core/src/seed.rs` (`mix64`) |

## Why it matters

SplitMix64's output function is the 64-bit mixer behind every seed in the game (`mix64`).

## Notes

- SplitMix64 is a counter (`state += 0x9E3779B97F4A7C15`) passed through a mixing function. The
  state is trivial; all the quality comes from the mixer.
- The mixer is xor-shift, multiply, xor-shift, multiply, xor-shift. The constants used in Java's
  `SplittableRandom` and Vigna's C code are Stafford's "Mix13" variant, chosen by search for strong
  avalanche (each input bit flips each output bit with probability close to ½).
- Because the mixer is a **bijection** on 64-bit values, distinct inputs never collide.
- "Splittable" means a generator can be split into independent child generators, the same idea as
  our seed chain.

## Takeaways

- A good mixer on a counter is a fine counter-based RNG for procedural content. It is not
  cryptographic, and does not need to be.
- The first output for seed 0 is `0xE220A8397B1DCDAF`; it is a known-answer test in `seed.rs`.

## Doubts / follow-ups

- If statistical quality ever becomes a question (it should not for terrain), PractRand or
  TestU01 are the standard test batteries.
