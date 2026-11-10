# Working in this repository

Instructions for anyone, human or AI agent, making changes here.

## Before you finish any change

1. **Run the checks.**
   ```sh
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   node tools/doc-checks/check.js
   ```
   CI runs the same on Windows and Linux, plus the E2 determinism check.
2. **Clean up.** Rust accumulates build data quickly. When a task is done:
   ```sh
   powershell -ExecutionPolicy Bypass -File tools/clean.ps1        # prune incremental caches
   powershell -ExecutionPolicy Bypass -File tools/clean.ps1 -All   # full cargo clean, when target/ is large
   ```
   Also delete any scratch directories or probe files you created (in `%TEMP%` or the repo).
3. **Never commit build output.** `target/` and `node_modules/` are ignored; check
   `git status` before committing anyway.

## Layout

- `docs/` — the architecture. Start at `docs/README.md`. Concepts live in exactly one doc;
  others link to it.
- `crates/` — production Rust workspace. Generation crates never depend on Bevy
  (`docs/20-tech-stack-and-workspace.md`).
- `spikes/` — throwaway experiments, a separate Cargo workspace sharing the root `target/`.
  Results go in `docs/spikes/`.
- `tools/` — doc checks and the clean script.

## Rules that are easy to break

- Generation code uses `genesis_core::math`, never `std` transcendental functions; clippy
  enforces it. See `docs/20-tech-stack-and-workspace.md#determinism-rules`.
- Known-answer tests in `genesis_core` pin world identity. If one fails, you changed every world;
  do not just update the constant.
- `spikes/field/e2-expected.txt` pins cross-platform determinism of the spike field. Update it only
  when the field is changed on purpose, and say so in the commit.
- On this Windows machine, PowerShell script execution is disabled: use `npm.cmd`, and run `.ps1`
  files with `powershell -ExecutionPolicy Bypass -File`.
