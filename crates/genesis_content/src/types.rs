//! Typed content entries (docs/17), deserialised from the **resolved** table of each entry
//! (after `extends`). Unknown keys are errors (docs/16: authored content is strict).
//!
//! Reference fields hold text as written (shorthand or full); validation rewrites every one of
//! them to a full ID (`ns:type/name`), so a loaded [`crate::Content`] only contains full IDs.

use serde::Deserialize;
use std::collections::BTreeMap;

fn one() -> f64 {
    1.0
}
fn yes() -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BiomeKind {
    Biome,
    Variant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stratum {
    Aerial,
    Benthic,
    Pelagic,
    Subglacial,
    Subterranean,
}

impl Stratum {
    pub fn as_str(self) -> &'static str {
        match self {
            Stratum::Aerial => "aerial",
            Stratum::Benthic => "benthic",
            Stratum::Pelagic => "pelagic",
            Stratum::Subglacial => "subglacial",
            Stratum::Subterranean => "subterranean",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BiosphereLevel {
    None,
    Microbial,
    Complex,
    Exotic,
}

/// How axis scores combine: product (`all`, the default) or maximum (`any`) (docs/17, 19).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    All,
    Any,
}

/// One suitability axis expression: exactly one of `range`, `curve`, `any_of`, `none_of`
/// (checked in validation), plus an optional `importance` exponent.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AxisExpr {
    pub range: Option<[f64; 2]>,
    pub falloff: Option<f64>,
    pub curve: Option<Vec<[f64; 2]>>,
    pub any_of: Option<Vec<String>>,
    pub none_of: Option<Vec<String>>,
    pub importance: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mosaic {
    pub scale: f64,
    pub strength: f64,
}

/// An inline scatter rule: a preset reference with per-biome overrides.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScatterUse {
    pub preset: String,
    pub density: Option<f64>,
    pub scale: Option<[f64; 2]>,
    pub slope_max: Option<f64>,
    pub altitude: Option<[f64; 2]>,
    pub min_lod: Option<u8>,
    pub max_lod: Option<u8>,
    pub cull_distance: Option<f64>,
}

/// `"preset"` or `{ preset = "...", ... }` (docs/17: a bare string is a preset reference).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum ScatterRule {
    Preset(String),
    Inline(ScatterUse),
}

impl ScatterRule {
    pub fn preset(&self) -> &str {
        match self {
            ScatterRule::Preset(p) => p,
            ScatterRule::Inline(u) => &u.preset,
        }
    }
    pub fn preset_mut(&mut self) -> &mut String {
        match self {
            ScatterRule::Preset(p) => p,
            ScatterRule::Inline(u) => &mut u.preset,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuiteWeight {
    pub suite: String,
    #[serde(default = "one")]
    pub weight: f64,
}

/// `"suite"` or `{ suite = "...", weight = w }`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum SuiteBias {
    Suite(String),
    Weighted(SuiteWeight),
}

impl SuiteBias {
    pub fn suite(&self) -> &str {
        match self {
            SuiteBias::Suite(s) => s,
            SuiteBias::Weighted(w) => &w.suite,
        }
    }
    pub fn suite_mut(&mut self) -> &mut String {
        match self {
            SuiteBias::Suite(s) => s,
            SuiteBias::Weighted(w) => &mut w.suite,
        }
    }
    pub fn weight(&self) -> f64 {
        match self {
            SuiteBias::Suite(_) => 1.0,
            SuiteBias::Weighted(w) => w.weight,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Biome {
    pub display_name: String,
    pub kind: BiomeKind,
    pub stratum: Stratum,
    #[serde(default = "one")]
    pub weight: f64,
    pub palette: String,
    pub material_tint: Option<String>,
    pub feature_scale: f64,
    pub min_lod: Option<u8>,
    pub max_lod: Option<u8>,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub suitability: BTreeMap<String, AxisExpr>,
    #[serde(default)]
    pub scatter: Vec<ScatterRule>,
    #[serde(default)]
    pub landform_bias: Vec<SuiteBias>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Free-form gameplay data (forageables, cover, hazard); not interpreted by generation.
    pub gameplay: Option<toml::Table>,
    #[serde(default = "yes")]
    pub selectable: bool,
    #[serde(default)]
    pub fallback: bool,
    pub requires_biosphere: Option<BiosphereLevel>,
    pub mosaic: Option<Mosaic>,
    #[serde(default)]
    pub incompatible_with: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ecotone {
    pub between: [String; 2],
    pub biome: String,
    #[serde(default = "one")]
    pub strength: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagTarget {
    pub tag: String,
}

/// An overlay target: a biome ID or `{ tag = "..." }`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Target {
    Id(String),
    Tag(TagTarget),
}

/// A string or a list of strings (`hazard_add = "biological"` is the docs/17 example).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overlay {
    pub target: Target,
    pub tint: Option<String>,
    pub material_override: Option<String>,
    #[serde(default)]
    pub scatter_add: Vec<ScatterRule>,
    #[serde(default)]
    pub scatter_remove: Vec<String>,
    pub hazard_add: Option<OneOrMany>,
    pub hazard_remove: Option<OneOrMany>,
    #[serde(default)]
    pub priority: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuiteContext {
    #[serde(default)]
    pub strata: Vec<Stratum>,
    #[serde(default)]
    pub substrates: Vec<String>,
    #[serde(default)]
    pub biomes: Vec<String>,
    pub slope: Option<[f64; 2]>,
    /// Soft edge of the slope range, degrees (docs/17: default 5°).
    pub slope_falloff: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suite {
    pub module: String,
    pub feature_scale: f64,
    pub min_lod: Option<u8>,
    pub max_lod: Option<u8>,
    #[serde(default)]
    pub context: SuiteContext,
    /// Checked against the module's schema; see [`crate::modules`].
    pub params: toml::Table,
    #[serde(default)]
    pub scatter: Vec<ScatterRule>,
    #[serde(default = "one")]
    pub weight: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Palette {
    pub base: String,
    pub accent: Option<String>,
    pub tint: Option<String>,
    pub roughness: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Material {
    pub albedo: String,
    pub roughness: f64,
    pub wet_variant: Option<String>,
    pub scatter_tint: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Anchor {
    Surface,
    Volume,
    Both,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scatter {
    pub anchor: Anchor,
    pub density: f64,
    pub scale: [f64; 2],
    pub feature_scale: Option<f64>,
    pub slope_max: Option<f64>,
    pub altitude: Option<[f64; 2]>,
    pub depth: Option<[f64; 2]>,
    pub clustering: Option<f64>,
    pub min_lod: Option<u8>,
    pub max_lod: Option<u8>,
    pub cull_distance: Option<f64>,
    pub instance_cap: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JitterAmplitudes {
    pub temp: f64,
    pub moisture: f64,
    pub altitude: f64,
    pub slope: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteMax {
    pub aerial: u32,
    pub benthic: u32,
}

/// The classification preset (docs/19 "Tuning constants"), `[classification]` in a pack's
/// `classification.toml`. Exactly one in a loaded content set.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Classification {
    pub sharpness: u32,
    pub budget: u32,
    pub epsilon: f64,
    pub mosaic_strength: f64,
    pub mosaic_scale: f64,
    pub jitter_wavelength: f64,
    pub jitter: JitterAmplitudes,
    pub strata_blend: f64,
    pub cave_blend: f64,
    pub palette_drop: f64,
    pub palette_max: PaletteMax,
}
