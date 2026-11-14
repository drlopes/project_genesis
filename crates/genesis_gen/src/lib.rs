//! Body generation: bake (descriptor core + caches) and, later, the surface query.
//!
//! See docs/03-planet-descriptor.md (the descriptor and bake ordering), docs/13-generator-modules.md
//! (the bake contract), docs/09-surface-field-stack.md (L0), and docs/20 (no Bevy here; generation
//! maths only through `genesis_core::math`).

pub mod bake;
pub mod climate;
pub mod cube;
pub mod descriptor;
pub mod encode;
pub mod params;
pub mod physical;
pub mod relief;

pub use bake::{BakeError, Caches, bake_caches, bake_core};
pub use descriptor::{DescriptorCore, SeedChain};
pub use params::RockyParams;
