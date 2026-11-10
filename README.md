# Project Genesis

A seamless space exploration and survival game: fly from a planet's surface to orbit and on to
another world with no loading screens, across thousands of procedurally generated worlds that
should never feel the same. Underwater is a first-class half of the game.

Built in **Rust** with **Bevy**.

## Where things are

| Path | What |
|---|---|
| [`docs/`](docs/README.md) | The architecture: start at [`docs/README.md`](docs/README.md) |
| [`work/`](work/README.md) | What we are doing now, bugs, ideas, and how we work |
| [`CHANGELOG.md`](CHANGELOG.md) | Every change, as it lands |
| [`research/`](research/README.md) | One card per research source; raw files stay local |
| [`knowledge/`](knowledge/README.md) | Distilled knowledge from research |
| [`art/`](art/README.md) | Concept art (Git LFS) |
| `crates/` | The Rust workspace (see [docs/20](docs/20-tech-stack-and-workspace.md)) |
| `spikes/` | Throwaway experiments; results in [`docs/spikes/`](docs/spikes/) |
| `tools/` | Doc checks and the clean script |
| [`AGENTS.md`](AGENTS.md) | Working habits for humans and AI agents |

After cloning, run `git lfs install --local` once so concept art downloads.

## Build and check

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
node tools/doc-checks/check.js      # after `npm install` in tools/doc-checks
```

The toolchain is pinned in `rust-toolchain.toml`.
