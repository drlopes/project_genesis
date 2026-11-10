# Project Genesis

A seamless space exploration and survival game: fly from a planet's surface to orbit and on to
another world with no loading screens, across thousands of procedurally generated worlds that
should never feel the same. Underwater is a first-class half of the game.

Built in **Rust** with **Bevy**.

## Where things are

| Path | What |
|---|---|
| [`docs/`](docs/README.md) | The architecture: start at [`docs/README.md`](docs/README.md) |
| `crates/` | The Rust workspace (see [docs/20](docs/20-tech-stack-and-workspace.md)) |
| `tools/doc-checks/` | Validates the docs: TOML blocks, links, anchors |

## Build and check

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
node tools/doc-checks/check.js      # after `npm install` in tools/doc-checks
```

The toolchain is pinned in `rust-toolchain.toml`.
