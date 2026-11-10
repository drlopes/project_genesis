# Knowledge base

Distilled understanding from research, in our own words, with citations to
[source cards](../research/sources/). Raw material is never here; see
[research/README.md](../research/README.md) for the rules.

How this differs from [`docs/`](../docs/README.md): the docs record **what this game does and
why**; knowledge notes record **how a technique works**, including options we did not choose.
Docs link here for background; notes link to the docs and code that apply them.

## Notes

| Area | Note | Summary |
|---|---|---|
| PCG | [Noise and hashing](pcg/noise-and-hashing.md) | counter-based seeds, mixers, gradient noise, fBm, band-limiting, float determinism |

Planned areas, created when the first note needs them: `pcg/` (noise, erosion, L-systems,
cratering), `simulation/` (water, atmosphere), `rendering/`, `engine/` (Bevy, ECS), `gameplay/`.

## Writing a note

- One topic per file, under an area folder: `knowledge/<area>/<topic>.md`.
- Start with a one-paragraph **summary**, then the explanation, then **pitfalls**, **how we use
  it** (links to docs and code), and **sources** (links to cards).
- Every non-obvious claim cites a card: `([perlin-2002](../research/sources/perlin-2002-improving-noise.md))`.
- Prefer our own diagrams (ASCII or Mermaid) to described figures. Never paste figures from sources.
- When a note changes understanding that a doc relied on, check whether the doc needs updating.
- Add the note to the table above.
