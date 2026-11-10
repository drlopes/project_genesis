# The FNV non-cryptographic hash

| | |
|---|---|
| **ID** | `fowler-noll-vo-fnv-hash` |
| **Type** | article / specification |
| **Authors / creator** | Glenn Fowler, Landon Curt Noll, Kiem-Phong Vo |
| **Year** | 1991 (algorithm); IETF draft published later by Eastlake et al. |
| **Where** | http://www.isthe.com/chongo/tech/comp/fnv/ ; IETF draft "The FNV Non-Cryptographic Hash Algorithm" |
| **Accessed** | 2026-11-10 |
| **Licence / rights** | algorithm in the public domain |
| **Local copy** | none |
| **Topics** | hashing |
| **Status** | distilled |
| **Distilled into** | [Noise and hashing](../../knowledge/pcg/noise-and-hashing.md) |
| **Used in** | `crates/genesis_core/src/seed.rs` (`fnv1a64`, purpose tags and content IDs) |

## Why it matters

Turns purpose tags (`"geology"`) and content IDs (`"core:biome/temperate_forest"`) into 64-bit
values for the seed chain.

## Notes

- FNV-1a, 64-bit: start from the offset basis `0xCBF29CE484222325`; for each byte, XOR it in, then
  multiply by the prime `0x100000001B3`.
- Very simple and fast for short strings, with decent dispersion. Weak avalanche on its own, which
  is why we pass its output through `mix64` before use.
- Published test vectors (`""`, `"a"`, `"foobar"`) are known-answer tests in `seed.rs`.

## Takeaways

- Use FNV-1a for short keys, then a strong mixer. Never rely on FNV alone where avalanche matters.
