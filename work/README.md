# How we work

This folder is the operational state of the project: what is being done now, what is broken,
and what might be worth doing. It exists so that work survives between sessions and nothing found
along the way is lost or allowed to derail the current goal.

| File | Holds | IDs |
|---|---|---|
| [`NOW.md`](NOW.md) | the one active goal, its tasks, the short queue, decisions waiting on the user | — |
| [`BUGS.md`](BUGS.md) | defects: something that does not behave as specified | `B-001`, `B-002`, … |
| [`IDEAS.md`](IDEAS.md) | possibilities: improvements, features, optimisations, questions | `I-001`, `I-002`, … |
| [`../CHANGELOG.md`](../CHANGELOG.md) | what changed, for every change that lands | — |

What does **not** live here:

- Architecture, decisions (`D##`), open design questions (`Q##`), and the phase roadmap: in
  [`docs/`](../docs/README.md), mainly [`docs/14`](../docs/14-open-questions-and-roadmap.md).
- Spike results: [`docs/spikes/`](../docs/spikes/).
- Research and distilled knowledge: [`research/`](../research/README.md) and
  [`knowledge/`](../knowledge/README.md).

---

## The rules

### 1. One active goal

`NOW.md` has exactly **one** active goal at a time, broken into checkable tasks. A goal is a
coherent piece of work that ends in a commit-worthy state: "E3 — LOD spike app", not "work on
terrain".

The **Up next** queue holds at most ~5 goals, in order. Long-term planning stays in
[`docs/14`](../docs/14-open-questions-and-roadmap.md); the queue is pulled from it.

### 2. Capture, don't chase

Anything discovered while working on the active goal is **written down within a minute and left
alone**:

| Found | Goes to |
|---|---|
| Something broken | `BUGS.md`, with a severity |
| Something that could be better, or a new possibility | `IDEAS.md` |
| Something needed to finish the active goal | a new task in `NOW.md` under the active goal |
| A question only the user can answer | `NOW.md` → **Waiting on the user** |

Then return to the active goal. The single exception is an **S1 bug** (below), which may interrupt.

### 3. Bug severity

| Severity | Meaning | Rule |
|---|---|---|
| **S1** | crash, data loss, broken determinism, broken build or CI, wrong saved data | stop and fix before continuing |
| **S2** | wrong behaviour or a violated invariant, but work can continue | fix within the current or next goal |
| **S3** | cosmetic, misleading docs or comments, minor inefficiency | fix when convenient |

### 4. Ideas are triaged, not acted on

An idea is not a task. At the end of each goal, new ideas are reviewed:

- **accepted** → becomes a goal in the queue, a task, or a roadmap item in `docs/14`;
- **rejected** → kept, with the reason, so it is not proposed again;
- **parked** → kept open, revisited at the next milestone.

### 5. Definition of done (every goal, every commit)

1. Checks pass: fmt, clippy, tests, doc checks (see [`AGENTS.md`](../AGENTS.md)).
2. Docs that own anything this change affected are updated (concepts live in exactly one doc).
3. `CHANGELOG.md` has an entry under **Unreleased**.
4. `NOW.md` is updated: tasks ticked, next step written down.
5. Bugs fixed are moved to **Closed** with the fixing commit; new findings are captured.
6. Build bloat cleaned (`tools/clean.ps1`), scratch files deleted.
7. Committed with IDs in the message where relevant (`Fix B-004: …`, `Implement I-012: …`).

### 6. Session rituals

**Start of a session:** read `AGENTS.md`, then `NOW.md` (active goal, next step, waiting items),
then the open S1/S2 entries in `BUGS.md`.

**End of a session**, even mid-goal: update `NOW.md` → **Where we stopped** with the exact next
step, commit, push. A session must never end with the next step existing only in someone's head.

---

## Entry formats

### Bug

```markdown
### B-007 — Short title
- **Severity:** S2 · **Status:** open | in progress | fixed | won't fix
- **Found:** 2026-11-12, commit `abc1234`, while doing <goal>
- **Where:** `crates/genesis_core/src/noise.rs` / docs/09 / CI job
- **Expected:** what should happen (cite the doc that specifies it)
- **Actual:** what happens
- **Repro:** steps or a command
- **Fixed in:** commit (when closed)
```

### Idea

```markdown
### I-012 — Short title
- **Status:** new | accepted → <where it went> | rejected | parked
- **Raised:** 2026-11-12, from <goal / spike / research card>
- **What:** the idea in two or three sentences
- **Why:** the problem it solves or the opportunity
- **Cost / risk:** rough
- **Decision:** (on triage) accepted/rejected/parked and why
```

IDs are never reused, even after an entry is closed or rejected. Closed and rejected entries move
to the bottom section of their file; they are not deleted.

---

## Changelog rules

[`CHANGELOG.md`](../CHANGELOG.md) follows [Keep a Changelog](https://keepachangelog.com/):

- Every change that lands adds a line under **Unreleased**, in one of: **Added**, **Changed**,
  **Fixed**, **Removed**, **Docs**, **Tooling**.
- Lines are written for a reader who was not there: what changed and why it matters, with IDs
  (`B-###`, `I-###`, `D##`, `E#`) where relevant.
- When a roadmap phase completes, **Unreleased** becomes a version: Phase 0 → `0.1.0`,
  Phase 1 → `0.2.0`, and so on, with the date.

## Scaling

These are single files on purpose. If `BUGS.md` or `IDEAS.md` grows past roughly 50 open entries,
split open entries into one file per entry (`work/bugs/B-123.md`) and keep the list file as an
index. If collaborators join, mirror to GitHub Issues (see `I-003`); the Markdown stays the source
of truth.
