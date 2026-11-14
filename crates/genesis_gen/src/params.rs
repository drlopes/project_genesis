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

/// Stand-in energy and atmosphere draws until stars and orbits exist (P1-C1), and the climate
/// tuning constants (docs/05; "a climate preset", later content data).
#[derive(Clone, Debug, PartialEq)]
pub struct ClimateParams {
    // ---- energy (docs/05 "Energy budget") ----
    /// `ln` of insolation relative to Earth: `N(0, insolation_sigma)`, clamped.
    pub insolation_sigma: f64,
    pub insolation: Range,
    /// Bond albedo before the ice feedback.
    pub albedo: Range,
    /// Albedo of a frozen sea surface (the feedback target).
    pub ice_albedo: f64,
    pub internal_heat: Range,
    /// K at full internal heat (docs/05: order 10 K).
    pub internal_warming: f64,

    // ---- atmosphere ----
    /// Probability that the body has an atmosphere.
    pub atmosphere: f64,
    /// `ln` of surface pressure (Pa): `N(ln(pressure_median), pressure_sigma)`, clamped.
    pub pressure_median: f64,
    pub pressure_sigma: f64,
    pub pressure: Range,
    /// Greenhouse offset from Earth's baseline, K: `−33 + 33 · min(p / 1e5, 3)^½ + N(0, σ)`;
    /// 0 at one bar, −33 K with no atmosphere (docs/05).
    pub greenhouse_sigma: f64,

    // ---- temperature field (docs/05) ----
    /// `A_lat0`, K.
    pub latitude_amplitude: f64,
    /// Heat transport per bar of pressure, before clamping to [0, 1].
    pub transport_scale: f64,
    /// Lapse rate at 1 g and 1 bar, K/m.
    pub lapse: f64,
    pub tropopause: f64,
    pub oro_cooling: f64,
    pub coastal_warming: f64,
    pub coast_length: f64,

    // ---- moisture and wind (docs/05) ----
    pub moisture_base: f64,
    pub moisture_noise: f64,
    pub moisture_wavelength: f64,
    pub itcz: f64,
    pub itcz_sigma: f64,
    pub subtropic: f64,
    pub subtropic_sigma: f64,
    pub midlatitude: f64,
    pub midlatitude_sigma: f64,
    pub water_moisture: f64,
    pub water_length: f64,
    /// `L_wind`: upwind sampling distance, metres.
    pub wind_distance: f64,
    pub slope_ref: f64,
    pub oro_moisture: f64,
    pub rain_shadow: f64,
    pub wind_meridional: f64,
    pub wind_noise_wavelength: f64,

    // ---- biosphere ----
    /// Weights of `none`, `microbial`, `complex`, `exotic` where life is possible (docs/11's
    /// example table).
    pub biosphere_weights: [f64; 4],
    /// Global mean temperature window for any life, K.
    pub biosphere_temperature: Range,

    // ---- grids ----
    /// Cells per face edge of the water-distance field.
    pub water_distance_grid: usize,
}

impl Default for ClimateParams {
    fn default() -> ClimateParams {
        ClimateParams {
            insolation_sigma: 0.3,
            insolation: Range { lo: 0.25, hi: 4.0 },
            albedo: Range { lo: 0.2, hi: 0.45 },
            ice_albedo: 0.6,
            internal_heat: Range { lo: 0.0, hi: 1.0 },
            internal_warming: 10.0,
            atmosphere: 0.85,
            pressure_median: 60_000.0,
            pressure_sigma: 0.8,
            pressure: Range {
                lo: 1_000.0,
                hi: 1_000_000.0,
            },
            greenhouse_sigma: 8.0,
            latitude_amplitude: 30.0,
            transport_scale: 0.7,
            lapse: 0.0065,
            tropopause: 12_000.0,
            oro_cooling: 2.0,
            coastal_warming: 1.0,
            coast_length: 6_000.0,
            moisture_base: 0.15,
            moisture_noise: 0.25,
            moisture_wavelength: 30_000.0,
            itcz: 0.25,
            itcz_sigma: 12.0,
            subtropic: 0.2,
            subtropic_sigma: 10.0,
            midlatitude: 0.15,
            midlatitude_sigma: 12.0,
            water_moisture: 0.3,
            water_length: 8_000.0,
            wind_distance: 8_000.0,
            slope_ref: 0.05,
            oro_moisture: 0.2,
            rain_shadow: 0.2,
            wind_meridional: 0.2,
            wind_noise_wavelength: 20_000.0,
            biosphere_weights: [0.1, 0.2, 0.6, 0.1],
            biosphere_temperature: Range {
                lo: 240.0,
                hi: 330.0,
            },
            water_distance_grid: 32,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RockyParams {
    pub climate: ClimateParams,
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
            climate: ClimateParams::default(),
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
