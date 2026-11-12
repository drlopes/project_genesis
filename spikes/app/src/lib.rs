//! Spike terrain library: patch meshing, quadtree selection, the worker pool, the fly camera, and
//! the precision emulation. Shared by the E3–E5 app (`main.rs`) and the E6 physics tests
//! (`spikes/physics`).

pub mod flycam;
pub mod patch;
pub mod precision;
pub mod quadtree;
pub mod workers;
