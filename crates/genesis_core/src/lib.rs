//! Deterministic foundations shared by every generation crate.
//!
//! Nothing in this crate may depend on Bevy, on platform maths, or on global state.
//! Its outputs define world identity: changing any of them changes every world.
//! See `docs/20-tech-stack-and-workspace.md#determinism-rules`.

pub mod math;
pub mod rng;
pub mod seed;

pub use seed::Seed;
