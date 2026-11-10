# Working in this repository

Instructions for anyone, human or AI agent, making changes here. The full process is in
[work/README.md](work/README.md); this is the checklist.

## Start of a session

1. Read this file.
2. Read [work/NOW.md](work/NOW.md): the active goal, **Where we stopped**, and **Waiting on the
   user**.
3. Check [work/BUGS.md](work/BUGS.md) for open **S1** and **S2** bugs. An open S1 is fixed first.

## While working

- **One active goal.** Work only on the goal in `NOW.md`.
- **Capture, don't chase.** Anything found along the way is written down within a minute and left
  alone: defects in `work/BUGS.md`, possibilities in `work/IDEAS.md`, required sub-tasks under the
  active goal in `NOW.md`, questions for the user under **Waiting on the user**. Only an S1 bug may
  interrupt.
- **Never reuse IDs** (`B-###`, `I-###`, `D##`, `Q##`, `E#`).

## Before you finish any change (definition of done)

1. **Run the checks.**
   ```sh
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   node tools/doc-checks/check.js
   ```
   CI runs the same on Windows and Linux, plus the E2 determinism check.
2. **Update the owning docs.** Concepts live in exactly one doc; others link to it.
3. **Update [CHANGELOG.md](CHANGELOG.md)** under **Unreleased**.
4. **Update `work/NOW.md`**: tick tasks, write the exact next step in **Where we stopped**. Move
   fixed bugs to **Closed** with the commit.
5. **Clean up.** Rust accumulates build data quickly:
   ```sh
   powershell -ExecutionPolicy Bypass -File tools/clean.ps1        # prune incremental caches
   powershell -ExecutionPolicy Bypass -File tools/clean.ps1 -All   # full cargo clean, when target/ is large
   ```
   Delete any scratch directories or probe files you created (in `%TEMP%` or the repo).
   With Bevy, `target/` grows past 10 GB after builds in several profiles or a linker change; the
   script warns above 5 GB, and then `-All` is the right call.
6. **Commit and push**, with IDs in the message where relevant. Check `git status` first: never
   commit build output or local-only material.

## End of a session

Even mid-goal: update **Where we stopped** in `NOW.md` with the exact next step, commit, push.

## Layout

| Path | What |
|---|---|
| `docs/` | architecture and decisions; start at `docs/README.md` |
| `docs/spikes/` | spike results (evidence for decisions) |
| `work/` | active goal, bugs, ideas, process rules |
| `research/sources/` | one card per research source (committed) |
| `research/library/` | raw PDFs, videos, cloned code — **local only, never committed** |
| `knowledge/` | distilled notes from research, citing cards |
| `art/concept/` | our concept art (Git LFS) |
| `art/references/` | third-party reference images — **local only, never committed** |
| `crates/` | production Rust workspace; generation crates never depend on Bevy |
| `spikes/` | throwaway experiments; separate Cargo workspace sharing the root `target/` |
| `tools/` | doc checks and the clean script |

## Rules that are easy to break

- **Copyright:** research material and third-party images are never committed. Cards and notes are
  written in our own words, with short attributed quotes at most. See
  [research/README.md](research/README.md).
- Generation code uses `genesis_core::math`, never `std` transcendental functions; clippy enforces
  it. See `docs/20-tech-stack-and-workspace.md#determinism-rules`.
- Known-answer tests in `genesis_core` pin world identity. If one fails, you changed every world;
  do not just update the constant.
- `spikes/field/e2-expected.txt` pins cross-platform determinism of the spike field. Update it only
  when the field is changed on purpose, and say so in the commit.
- On this Windows machine, PowerShell script execution is disabled: use `npm.cmd`, and run `.ps1`
  files with `powershell -ExecutionPolicy Bypass -File`.
- A fresh clone needs `git lfs install --local` once, so concept art downloads correctly.
