# Noise and hashing

**Summary.** Procedural worlds need randomness that is a pure function of position and identity,
so any piece can be generated in any order and still agree. That means hashing keys through a
strong bit mixer instead of drawing from a stateful random stream, and building noise on top of
the same hashing. Gradient noise with a quintic fade gives smooth, band-limited fields; summing
octaves (fBm) gives natural-looking detail; fading octaves out by sample spacing keeps every LOD
alias-free without changing the large-scale shape. All of it must use bit-exact maths, or
different machines generate different worlds.

---

## Counter-based randomness

A stateful generator answers "what is the next number?". Procedural generation needs to answer
"what is the number for *this* thing?": a chunk requested out of order, on another thread, or on
another machine must get the same value. So every value is `hash(identity, purpose, index)`.

The building blocks:

- **A string hash** for names and tags. FNV-1a is simple and fast for short keys, but its
  avalanche is weak, so its output is mixed further before use
  ([fnv](../../research/sources/fowler-noll-vo-fnv-hash.md)).
- **A bit mixer**, a bijective function on 64-bit values where every input bit affects every
  output bit with probability close to ½. SplitMix64's finaliser (Stafford's "Mix13" constants) is
  the standard choice ([splitmix](../../research/sources/steele-2014-splitmix.md)).
- **Combination**: mix the parent seed with the tag hash, then fold each index in with another
  mix. Each step separates inputs, so `(1, 2)` and `(2, 1)` differ and a missing index differs from
  index 0.

Turning a 64-bit value into numbers:

- Float in `[0, 1)`: take the top 24 bits (f32) or 53 bits (f64) and scale. The top bits are used
  because they are the best mixed.
- Integer in `[0, n)`: the multiply-shift `(x · n) >> 64` is fast, with bias at most `n / 2^64`;
  an extra rejection step makes it exact
  ([lemire](../../research/sources/lemire-2019-fast-random-integer-in-interval.md)). Plain
  `x % n` is both slower and more biased. `genesis_core::rng::below` uses the rejection step; with
  a counter-based RNG the redraw needs a defined key (it derives one from the rejected value and
  the attempt number).

## Gradient noise

Perlin-style gradient noise assigns a pseudo-random **gradient** to each integer lattice point and
interpolates the dot products between each corner's gradient and the offset to the sample:

```text
cell corners c, offset f = p − floor(p)
value = blend over the 8 corners of  dot(gradient(c), f − c_offset)
blend weights = fade(f) per axis
```

Properties worth knowing ([perlin-2002](../../research/sources/perlin-2002-improving-noise.md)):

- It is **exactly zero at lattice points**, and its energy sits around one cycle per lattice cell,
  so a noise of wavelength *w* is roughly band-limited to features of size *w*.
- The **fade** must be the quintic `6t⁵ − 15t⁴ + 10t³`. The older cubic has a discontinuous second
  derivative, which shows as creases in normals and displacement.
- Use a **fixed gradient set**: the 12 cube-edge directions, padded to 16 so a 4-bit hash selects
  one without a modulo. Random gradients clump.
- The gradient is chosen by hashing the lattice coordinates. Computing per-axis terms once and
  mixing once per corner is far cheaper than mixing each axis separately: in
  [E1](../../docs/spikes/E1-field-cost.md) this alone made noise 8.5× faster.

## Fractal sums (fBm) and band-limiting

Fractal Brownian motion sums octaves of noise, each with a shorter wavelength (÷ *lacunarity*,
usually 2) and a smaller amplitude (× *gain*, usually 0.5).

When sampling on a grid of spacing *s*, any octave with a wavelength below about `2s` cannot be
represented and **aliases** into false detail (Nyquist). So each octave is weighted:

```text
weight = smoothstep(0.5, 1.0, wavelength / (2 · s))
```

Octaves that are too fine contribute nothing and need not be evaluated. The subtle part is
**normalisation**: divide by the sum of *all* octave amplitudes, not just the active ones.
Normalising by the active sum would scale the coarse shape up when fine octaves drop out, so the
terrain would change height as the LOD changes.

## Floating-point determinism

Bit-identical generation across machines needs bit-identical arithmetic:

- `+ − × ÷` and `sqrt` are exactly specified by IEEE-754: the same on every conforming platform.
- **Transcendental functions are not.** `sin`, `exp`, `pow` and the rest come from the platform's
  maths library, and Windows and Linux implementations differ in the last bits. A pure-software
  implementation such as Rust's `libm` gives the same bits everywhere
  ([libm](../../research/sources/rust-libm.md)).
- **Fused multiply-add** rounds once instead of twice, so `a * b + c` with and without FMA differ.
  Never let the compiler or code choose FMA implicitly.
- **Iteration order** matters for float sums: `(a + b) + c ≠ a + (b + c)`. Accumulate in a defined
  order, e.g. sorted IDs.
- **Precision far from the origin**: an `f32` has 24 bits of mantissa, so at 240 000 (a 24 km
  planet at 0.1 m wavelength) its resolution is about 0.016. Compute the lattice cell in `f64`
  and keep only the small fractional part in `f32`.

## Pitfalls

- A weak hash on structured inputs (grid coordinates) produces visible patterns. Always finish with
  a strong mixer.
- Re-deriving seeds with the *same* tag for two different decisions correlates them. One tag, one
  decision.
- Noise with values clamped or remapped non-linearly loses its band-limiting guarantee.
- Changing a mixer constant, the gradient table, or the octave seeding changes every world. Pin
  them with known-answer tests.

## How we use it

- Seed chain and tag rules: [docs/11](../../docs/11-determinism-and-sampling.md#the-seed-chain).
- Determinism rules: [docs/20](../../docs/20-tech-stack-and-workspace.md#determinism-rules).
- Band-limiting in the field stack: [docs/09](../../docs/09-surface-field-stack.md#band-limiting-and-lod).
- Code: `crates/genesis_core/src/seed.rs`, `rng.rs`, `noise.rs`, `math.rs`.
- Evidence: [E1](../../docs/spikes/E1-field-cost.md) (cost), [E2](../../docs/spikes/E2-determinism.md)
  (cross-platform determinism).

## Sources

- [perlin-2002-improving-noise](../../research/sources/perlin-2002-improving-noise.md)
- [steele-2014-splitmix](../../research/sources/steele-2014-splitmix.md)
- [fowler-noll-vo-fnv-hash](../../research/sources/fowler-noll-vo-fnv-hash.md)
- [lemire-2019-fast-random-integer-in-interval](../../research/sources/lemire-2019-fast-random-integer-in-interval.md)
- [rust-libm](../../research/sources/rust-libm.md)
