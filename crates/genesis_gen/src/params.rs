//! Bake parameters for the `rocky` class: the class defaults (docs/13 `params`).
//!
//! Every value here is `PROPOSED` (docs/04, docs/06, docs/09 give examples, not ranges; Q4 leaves
//! the radius range open). They become data in the content packs once `genesis_content` exists;
//! until then, **changing a default changes bake output and must bump `GENERATOR_VERSION`**.

/// An inclusive range for a uniform draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub lo: f64,
    pub hi: f64,
}

/// A clamped normal draw: `clamp(median + spread · N(0, 1), lo, hi)` (docs/02 profiles use
/// `{ median, spread }`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Normal {
    pub median: f64,
    pub spread: f64,
    pub lo: f64,
    pub hi: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RockyParams {
    // ---- bulk and rotation ----
    /// Body radius, metres. Q4 open; E3 ran 8 / 24 / 40 km.
    pub radius: Range,
    /// Surface gravity, g (docs/03 example 0.36, docs/04 example 0.38).
    pub gravity: Range,
    /// Day length, game seconds (docs/03 example 512).
    pub day_length: Range,
    /// Obliquity, degrees.
    pub obliquity: Range,

    // ---- water inventory ----
    /// Target ocean fraction; docs/02's default profile `water_fraction = { 0.55, 0.25 }`.
    pub ocean_fraction: Normal,
    /// Below this the body is dry: `state = none`, no sea (docs/04's `barren` label uses 0.02).
    pub dry_below: f64,

    // ---- geology ----
    /// Plate count, inclusive (docs/06: "e.g. 5–12").
    pub plates: (u32, u32),
    /// Probability that a plate is continental.
    pub continental: f64,
    /// docs/02 profile `tectonics = { median = 0.6, spread = 0.2 }`.
    pub tectonic_activity: Normal,
    pub volcanism: Range,
    /// Surface age of oceanic and continental plates (0 = fresh, 1 = ancient).
    pub age_oceanic: Range,
    pub age_continental: Range,

    // ---- relief (L0), metres ----
    /// Base elevation of continental and oceanic plates.
    pub base_continental: f64,
    pub base_oceanic: f64,
    /// Softness of the plate blend for the base elevation (a shelf), metres.
    pub base_softness: f64,
    /// Softness of the boundary features (half-width ≈ 1.8 × this), metres.
    pub ridge_softness: f64,
    /// Boundary amplitude at full convergence: continental–continental, mixed, oceanic–oceanic;
    /// and the rift depth at full divergence. Scaled by tectonic activity.
    pub ridge_cc: f64,
    pub ridge_mixed: f64,
    pub ridge_oo: f64,
    pub rift: f64,
    /// Domain warp of the plate layout, so boundaries are not great-circle arcs.
    pub warp_amplitude: f64,
    pub warp_wavelength: f64,
    /// Macro noise amplitude at activity 0.5 (scaled by `0.5 + activity`), and its first wavelength.
    pub macro_amplitude: f64,
    pub macro_wavelength: f64,
    pub macro_octaves: u32,

    // ---- craters ----
    /// Placement attempts per km² of surface at age 1.
    pub crater_density: f64,
    /// Rim radius range, metres; sizes follow a cumulative power law `N(> r) ∝ r^−crater_slope`.
    pub crater_radius: Range,
    pub crater_slope: f64,
    /// Largest crater radius as a fraction of the body radius.
    pub crater_max_fraction: f64,
    /// Bowl depth as a fraction of the rim radius (depth / diameter = half of this).
    pub crater_depth: f64,
    /// Two craters of similar size (smaller/larger > `crater_similar`) may not have centres closer
    /// than `crater_spacing × (r1 + r2)` (docs/06's Poisson-disk rule, without forbidding small
    /// craters inside large ones).
    pub crater_similar: f64,
    pub crater_spacing: f64,

    // ---- grids ----
    /// Cells per face edge of the coarse relief grid (hypsometry).
    pub relief_grid: usize,
    /// Nodes per face edge of the macro pyramid cache.
    pub pyramid_grid: usize,
    /// Cells per face edge of the crater index cache.
    pub crater_index_grid: usize,
}

impl Default for RockyParams {
    fn default() -> RockyParams {
        RockyParams {
            radius: Range {
                lo: 16_000.0,
                hi: 32_000.0,
            },
            gravity: Range { lo: 0.2, hi: 0.6 },
            day_length: Range {
                lo: 300.0,
                hi: 1800.0,
            },
            obliquity: Range { lo: 0.0, hi: 40.0 },
            ocean_fraction: Normal {
                median: 0.55,
                spread: 0.25,
                lo: 0.0,
                hi: 0.95,
            },
            dry_below: 0.02,
            plates: (5, 12),
            continental: 0.4,
            tectonic_activity: Normal {
                median: 0.6,
                spread: 0.2,
                lo: 0.0,
                hi: 1.0,
            },
            volcanism: Range { lo: 0.0, hi: 1.0 },
            age_oceanic: Range { lo: 0.0, hi: 0.5 },
            age_continental: Range { lo: 0.3, hi: 1.0 },
            base_continental: 500.0,
            base_oceanic: -1000.0,
            base_softness: 2500.0,
            ridge_softness: 1200.0,
            ridge_cc: 1800.0,
            ridge_mixed: 1300.0,
            ridge_oo: 900.0,
            rift: 700.0,
            warp_amplitude: 3000.0,
            warp_wavelength: 15_000.0,
            macro_amplitude: 500.0,
            macro_wavelength: 12_000.0,
            macro_octaves: 6,
            crater_density: 0.04,
            crater_radius: Range {
                lo: 150.0,
                hi: 3000.0,
            },
            crater_slope: 2.0,
            crater_max_fraction: 1.0 / 6.0,
            crater_depth: 0.15,
            crater_similar: 0.5,
            crater_spacing: 0.6,
            relief_grid: 64,
            pyramid_grid: 129,
            crater_index_grid: 16,
        }
    }
}
