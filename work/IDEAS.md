# Ideas

Rules and entry format: [work/README.md](README.md). Ideas are triaged at the end of each goal;
they are not acted on directly.

## Open

### I-001 — Bake classification inputs that need extra `h0` evaluations
- **Status:** parked (revisit after E4)
- **Raised:** 2026-11-10, from [E1](../docs/spikes/E1-field-cost.md#open-optimisations-not-needed-yet)
- **What:** store slope and the orographic upwind term on the macro grid at bake, instead of
  evaluating `h0` three more times per sample.
- **Why:** about 500 ns of the ~1.05 µs L0–L2 cost; roughly halves classification-band cost while
  keeping LOD independence.
- **Cost / risk:** one more cache (memory per body); slope resolution drops to the macro grid
  (~150 m on a 24 km body).
- **Decision:** parked until E4 shows whether throughput needs it.

### I-002 — Multi-channel noise for classification jitter
- **Status:** parked (revisit after E4)
- **Raised:** 2026-11-10, from [E1](../docs/spikes/E1-field-cost.md#open-optimisations-not-needed-yet)
- **What:** one lattice hash per corner, with different gradient bits per channel, so the four
  jitter axes cost about one noise evaluation instead of four.
- **Why:** ~50 ns per sample; small, but jitter runs on every classified sample.
- **Cost / risk:** new noise function to pin with known answers; low.
- **Decision:** parked at the end of the work-structure goal: E1 is within budget, so this only
  matters if E4 shows a throughput shortfall. Revisit together with I-001.

### I-003 — Mirror work items to GitHub Issues when collaborators join
- **Status:** parked
- **Raised:** 2026-11-10, while designing the work structure
- **What:** keep `work/*.md` as the source of truth and mirror open bugs and accepted ideas to
  GitHub Issues for discussion and boards.
- **Why:** Issues have better UI and notifications once more than one person is involved.
- **Cost / risk:** two sources can drift; needs a one-way sync rule or tooling.

## Closed

_None yet._
