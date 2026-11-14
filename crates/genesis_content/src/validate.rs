//! Typing and validation of resolved entries (docs/16 "Validation", docs/17 "Validation rules").
//! Comments cite the rule: `16.n` is docs/16's minimum check n, `17.n` is docs/17's rule n.
//! Every reference is rewritten to its full ID as it is checked.

use crate::id::{ContentId, Kind, Report};
use crate::load::Raw;
use crate::modules;
use crate::types::*;
use std::collections::{BTreeMap, BTreeSet};
use toml::{Table, Value};

/// The voxel size (D44): `feature_scale` below it is a warning (docs/16, 17.9).
pub const VOXEL_SIZE: f64 = 0.5;

pub const SUBSTRATES: &[&str] = &[
    "rock", "rubble", "sand", "silt", "mud", "soil", "ice", "coral", "organic",
];
pub const WATER_BODIES: &[&str] = &[
    "marine",
    "freshwater",
    "brine",
    "hydrocarbon",
    "ammonia",
    "subsurface",
];

/// The axis registry (docs/17): name, enum values (`None` = numeric), and legal strata
/// (`None` = all).
type AxisDef = (
    &'static str,
    Option<&'static [&'static str]>,
    Option<&'static [Stratum]>,
);

const AXES: &[AxisDef] = &[
    (
        "temp",
        None,
        Some(&[Stratum::Aerial, Stratum::Benthic, Stratum::Subterranean]),
    ),
    ("moisture", None, Some(&[Stratum::Aerial])),
    ("humidity", None, Some(&[Stratum::Subterranean])),
    (
        "substrate",
        Some(SUBSTRATES),
        Some(&[Stratum::Aerial, Stratum::Benthic, Stratum::Subterranean]),
    ),
    ("altitude", None, Some(&[Stratum::Aerial])),
    ("slope", None, Some(&[Stratum::Aerial, Stratum::Benthic])),
    ("latitude", None, None),
    ("depth", None, Some(&[Stratum::Benthic])),
    (
        "depth_fraction",
        None,
        Some(&[Stratum::Benthic, Stratum::Pelagic]),
    ),
    ("light", None, Some(&[Stratum::Benthic, Stratum::Pelagic])),
    ("current", None, Some(&[Stratum::Benthic, Stratum::Pelagic])),
    ("vent", None, Some(&[Stratum::Benthic])),
    (
        "water_body",
        Some(WATER_BODIES),
        Some(&[Stratum::Benthic, Stratum::Pelagic]),
    ),
    ("ice", None, None),
];

/// Typed, validated entries with full-ID references.
#[derive(Default)]
pub struct Typed {
    pub biomes: BTreeMap<ContentId, Biome>,
    pub ecotones: BTreeMap<ContentId, Ecotone>,
    pub overlays: BTreeMap<ContentId, Overlay>,
    pub suites: BTreeMap<ContentId, Suite>,
    pub palettes: BTreeMap<ContentId, Palette>,
    pub materials: BTreeMap<ContentId, Material>,
    pub scatter: BTreeMap<ContentId, Scatter>,
    pub classification: Option<Classification>,
    /// Family root → members (root first, then variants by ID) (docs/19 "Families").
    pub families: BTreeMap<ContentId, Vec<ContentId>>,
}

/// Context for one entry's checks: where to report, and which pack resolves its shorthand.
struct Ctx<'a> {
    raw: &'a Raw,
    report: &'a mut Report,
    file: String,
    entry: String,
    pack: usize,
}

impl Ctx<'_> {
    fn error(&mut self, msg: impl Into<String>) {
        self.report.error(&self.file, Some(&self.entry), msg);
    }
    fn warn(&mut self, msg: impl Into<String>) {
        self.report.warn(&self.file, Some(&self.entry), msg);
    }
    /// 16.2: the reference exists and has the expected type; rewritten to the full ID.
    fn reference(&mut self, field: &str, text: &mut String, kind: Kind) -> Option<ContentId> {
        match self.raw.resolve(text, kind, self.pack) {
            Ok(id) => {
                *text = id.to_string();
                Some(id)
            }
            Err(e) => {
                self.error(format!("{field}: {e}"));
                None
            }
        }
    }
    fn color(&mut self, field: &str, s: &Option<String>) {
        if let Some(s) = s
            && !modules::is_color(s)
        {
            self.error(format!("{field}: `{s}` is not an sRGB `#rrggbb` colour"));
        }
    }
    /// 16.5 / 17.10.
    fn lods(&mut self, min: Option<u8>, max: Option<u8>) {
        if min.is_some_and(|l| l > 4) || max.is_some_and(|l| l > 4) {
            self.error("LODs are 0 (coarsest) to 4 (finest)");
        }
        if let (Some(a), Some(b)) = (min, max)
            && a > b
        {
            self.error(format!("min_lod {a} > max_lod {b}"));
        }
    }
    /// 16.6 / 17.9.
    fn feature_scale(&mut self, f: f64) {
        if f.is_nan() || f <= 0.0 {
            self.error(format!("feature_scale must be > 0 (got {f})"));
        } else if f < VOXEL_SIZE {
            self.warn(format!(
                "feature_scale {f} m is below the voxel size ({VOXEL_SIZE} m)"
            ));
        }
    }
    fn non_negative(&mut self, field: &str, x: f64) {
        if x.is_nan() || x < 0.0 {
            self.error(format!("{field} must be ≥ 0 (got {x})"));
        }
    }
    fn unit(&mut self, field: &str, x: Option<f64>) {
        if let Some(x) = x
            && !(0.0..=1.0).contains(&x)
        {
            self.error(format!("{field} must be in [0, 1] (got {x})"));
        }
    }
    fn ordered(&mut self, field: &str, r: Option<[f64; 2]>) {
        if let Some([a, b]) = r
            && (a.is_nan() || b.is_nan() || a > b)
        {
            self.error(format!("{field}: [{a}, {b}] is not ordered"));
        }
    }
    fn scatter_rules(&mut self, field: &str, rules: &mut [ScatterRule]) {
        for r in rules.iter_mut() {
            if let ScatterRule::Inline(u) = r {
                if let Some(d) = u.density {
                    self.non_negative(&format!("{field} density"), d);
                }
                self.ordered(&format!("{field} scale"), u.scale);
                self.ordered(&format!("{field} altitude"), u.altitude);
                let (a, b) = (u.min_lod, u.max_lod);
                self.lods(a, b);
            }
            self.reference(field, r.preset_mut(), Kind::Scatter);
        }
    }
}

fn typed<T: serde::de::DeserializeOwned>(table: &Table) -> Result<T, String> {
    Value::Table(table.clone())
        .try_into::<T>()
        .map_err(|e| e.to_string().trim().replace('\n', " "))
}

/// 17.2–17.4, 17.16 for one axis expression.
fn axis(ctx: &mut Ctx, stratum: Stratum, name: &str, e: &AxisExpr) {
    let Some((_, values, strata)) = AXES.iter().find(|a| a.0 == name) else {
        ctx.error(format!("suitability: unknown axis `{name}`"));
        return;
    };
    if let Some(s) = strata
        && !s.contains(&stratum)
    {
        ctx.error(format!(
            "suitability: axis `{name}` does not exist in the {} stratum",
            stratum.as_str()
        ));
    }
    let forms = [
        e.range.is_some(),
        e.curve.is_some(),
        e.any_of.is_some(),
        e.none_of.is_some(),
    ]
    .iter()
    .filter(|x| **x)
    .count();
    if forms != 1 {
        ctx.error(format!(
            "suitability.{name}: use exactly one of range, curve, any_of, none_of"
        ));
        return;
    }
    if let Some(i) = e.importance {
        ctx.non_negative(&format!("suitability.{name}.importance"), i);
    }
    match values {
        Some(allowed) => {
            if e.range.is_some() || e.curve.is_some() {
                ctx.error(format!(
                    "suitability.{name}: an enum axis takes any_of or none_of"
                ));
            }
            for v in e.any_of.iter().chain(e.none_of.iter()).flatten() {
                if !allowed.contains(&v.as_str()) {
                    ctx.error(format!(
                        "suitability.{name}: `{v}` is not one of {allowed:?}"
                    ));
                }
            }
        }
        None => {
            if e.any_of.is_some() || e.none_of.is_some() {
                ctx.error(format!(
                    "suitability.{name}: a numeric axis takes range or curve"
                ));
            }
            if let Some(r) = e.range {
                ctx.ordered(&format!("suitability.{name}.range"), Some(r));
                match e.falloff {
                    None => ctx.error(format!("suitability.{name}: a range needs `falloff`")),
                    Some(f) if f.is_nan() || f < 0.0 => {
                        ctx.error(format!("suitability.{name}: falloff must be ≥ 0"))
                    }
                    Some(0.0) => {
                        ctx.warn(format!("suitability.{name}: falloff 0 makes a hard border"))
                    }
                    _ => {}
                }
            } else if e.falloff.is_some() {
                ctx.error(format!("suitability.{name}: `falloff` belongs to a range"));
            }
            if let Some(c) = &e.curve {
                if c.len() < 2 {
                    ctx.error(format!(
                        "suitability.{name}: a curve needs at least two points"
                    ));
                }
                let increasing = |w: &[[f64; 2]]| w[0][0] < w[1][0];
                if c.iter().any(|p| p[0].is_nan()) || !c.windows(2).all(increasing) {
                    ctx.error(format!("suitability.{name}: curve x values must increase"));
                }
            }
        }
    }
}

pub fn validate(
    raw: &Raw,
    resolved: &BTreeMap<ContentId, Table>,
    parents: &BTreeMap<ContentId, ContentId>,
    report: &mut Report,
) -> Typed {
    let mut out = Typed::default();
    let mut stash: BTreeMap<ContentId, (String, usize)> = BTreeMap::new();

    // ---- typing (16.1, 16.8) ----
    for (id, table) in resolved {
        let e = &raw.entries[id];
        stash.insert(id.clone(), (e.file.clone(), e.pack));
        let shown = id.to_string();
        let res = match id.kind {
            Kind::Biome => typed::<Biome>(table).map(|v| {
                out.biomes.insert(id.clone(), v);
            }),
            Kind::Ecotone => typed::<Ecotone>(table).map(|v| {
                out.ecotones.insert(id.clone(), v);
            }),
            Kind::Overlay => typed::<Overlay>(table).map(|v| {
                out.overlays.insert(id.clone(), v);
            }),
            Kind::Suite => typed::<Suite>(table).map(|v| {
                out.suites.insert(id.clone(), v);
            }),
            Kind::Palette => typed::<Palette>(table).map(|v| {
                out.palettes.insert(id.clone(), v);
            }),
            Kind::Material => typed::<Material>(table).map(|v| {
                out.materials.insert(id.clone(), v);
            }),
            Kind::Scatter => typed::<Scatter>(table).map(|v| {
                out.scatter.insert(id.clone(), v);
            }),
        };
        if let Err(msg) = res {
            report.error(&e.file, Some(&shown), msg);
        }
    }

    // ---- materials, palettes, scatter ----
    for (id, m) in out.materials.iter_mut() {
        let (file, pack) = stash[id].clone();
        let mut c = Ctx {
            raw,
            report,
            file,
            entry: id.to_string(),
            pack,
        };
        c.color("albedo", &Some(m.albedo.clone()));
        c.color("scatter_tint", &m.scatter_tint);
        c.unit("roughness", Some(m.roughness));
        if let Some(w) = &mut m.wet_variant {
            c.reference("wet_variant", w, Kind::Material);
        }
    }
    for (id, p) in out.palettes.iter_mut() {
        let (file, pack) = stash[id].clone();
        let mut c = Ctx {
            raw,
            report,
            file,
            entry: id.to_string(),
            pack,
        };
        c.reference("base", &mut p.base, Kind::Material);
        if let Some(a) = &mut p.accent {
            c.reference("accent", a, Kind::Material);
        }
        c.color("tint", &p.tint);
        c.unit("roughness", p.roughness);
    }
    for (id, s) in out.scatter.iter() {
        let (file, pack) = stash[id].clone();
        let mut c = Ctx {
            raw,
            report,
            file,
            entry: id.to_string(),
            pack,
        };
        c.non_negative("density", s.density);
        c.ordered("scale", Some(s.scale));
        if s.scale[0] <= 0.0 {
            c.error("scale must be > 0");
        }
        c.feature_scale(s.feature_scale.unwrap_or(s.scale[0]));
        c.ordered("altitude", s.altitude);
        c.ordered("depth", s.depth);
        c.unit("clustering", s.clustering);
        c.lods(s.min_lod, s.max_lod);
        // 17.11: terrain constraints only make sense for surface instances.
        if s.anchor == Anchor::Volume && (s.slope_max.is_some() || s.altitude.is_some()) {
            c.error("slope_max and altitude apply to surface scatter, not `anchor = \"volume\"`");
        }
    }

    // ---- suites ----
    for (id, s) in out.suites.iter_mut() {
        let (file, pack) = stash[id].clone();
        let mut c = Ctx {
            raw,
            report,
            file,
            entry: id.to_string(),
            pack,
        };
        match modules::module(&s.module) {
            None => c.error(format!("module `{}` is not in the registry", s.module)), // 17.8
            Some(schema) => {
                for e in modules::check_params(schema, &s.params) {
                    c.error(e);
                }
            }
        }
        c.feature_scale(s.feature_scale);
        c.lods(s.min_lod, s.max_lod);
        c.non_negative("weight", s.weight);
        for sub in &s.context.substrates {
            if !SUBSTRATES.contains(&sub.as_str()) {
                c.error(format!(
                    "context.substrates: `{sub}` is not one of {SUBSTRATES:?}"
                ));
            }
        }
        for b in s.context.biomes.iter_mut() {
            c.reference("context.biomes", b, Kind::Biome);
        }
        c.ordered("context.slope", s.context.slope);
        if let Some(f) = s.context.slope_falloff {
            c.non_negative("context.slope_falloff", f);
        }
        c.scatter_rules("scatter", &mut s.scatter);
    }

    // ---- biomes ----
    let biome_ids: Vec<ContentId> = out.biomes.keys().cloned().collect();
    for id in &biome_ids {
        let (file, pack) = stash[id].clone();
        let parent_stratum = parents
            .get(id)
            .and_then(|p| out.biomes.get(p))
            .map(|p| p.stratum);
        let b = out.biomes.get_mut(id).expect("listed");
        let mut c = Ctx {
            raw,
            report,
            file,
            entry: id.to_string(),
            pack,
        };
        if b.display_name.trim().is_empty() {
            c.error("display_name must not be empty");
        }
        c.non_negative("weight", b.weight); // 17.13
        c.feature_scale(b.feature_scale);
        c.lods(b.min_lod, b.max_lod);
        c.reference("palette", &mut b.palette, Kind::Palette);
        c.color("material_tint", &b.material_tint);
        for (name, e) in &b.suitability {
            axis(&mut c, b.stratum, name, e);
        }
        if b.selectable && b.suitability.is_empty() {
            c.error(
                "a selectable biome needs a suitability map (an empty one scores 1 everywhere)",
            ); // 17.15
        }
        if let Some(m) = &b.mosaic {
            if m.scale.is_nan() || m.scale <= 0.0 {
                c.error("mosaic.scale must be > 0");
            }
            c.non_negative("mosaic.strength", m.strength);
        }
        c.scatter_rules("scatter", &mut b.scatter);
        for s in b.landform_bias.iter_mut() {
            c.non_negative("landform_bias weight", s.weight());
            c.reference("landform_bias", s.suite_mut(), Kind::Suite);
        }
        for other in b.incompatible_with.iter_mut() {
            c.reference("incompatible_with", other, Kind::Biome); // 17.18
        }
        if b.kind == BiomeKind::Variant && !parents.contains_key(id) {
            c.error("a variant must `extends` its family's biome");
        }
        if let Some(ps) = parent_stratum
            && ps != b.stratum
        {
            c.error(format!(
                "`extends` may not change the stratum ({} → {})",
                ps.as_str(),
                b.stratum.as_str()
            )); // 17.5
        }
    }

    // ---- families: a variant belongs to the first `biome`-kind ancestor (docs/19) ----
    for (id, b) in &out.biomes {
        let mut root = id.clone();
        if b.kind == BiomeKind::Variant {
            let mut cur = id.clone();
            while let Some(p) = parents.get(&cur) {
                cur = p.clone();
                if out
                    .biomes
                    .get(&cur)
                    .is_some_and(|x| x.kind == BiomeKind::Biome)
                {
                    break;
                }
            }
            root = cur;
        }
        out.families.entry(root).or_default().push(id.clone());
    }
    for (root, members) in out.families.iter_mut() {
        members.sort_by_key(|m| (m != root, m.clone()));
    }

    // 17.14: exactly one fallback per stratum that has biomes.
    let mut strata: BTreeMap<Stratum, Vec<&ContentId>> = BTreeMap::new();
    let mut fallbacks: BTreeMap<Stratum, Vec<&ContentId>> = BTreeMap::new();
    for (id, b) in &out.biomes {
        strata.entry(b.stratum).or_default().push(id);
        if b.fallback {
            fallbacks.entry(b.stratum).or_default().push(id);
        }
    }
    for (s, ids) in &strata {
        let f = fallbacks.get(s).map_or(0, Vec::len);
        if f != 1 {
            let file = stash[ids[0]].0.clone();
            report.error(
                &file,
                None,
                format!(
                    "the {} stratum needs exactly one `fallback = true` biome (found {f})",
                    s.as_str()
                ),
            );
        }
    }

    // ---- ecotones (17.17) ----
    for (id, e) in out.ecotones.iter_mut() {
        let (file, pack) = stash[id].clone();
        let mut c = Ctx {
            raw,
            report,
            file,
            entry: id.to_string(),
            pack,
        };
        c.non_negative("strength", e.strength);
        let [a, b] = &mut e.between;
        let a = c.reference("between", a, Kind::Biome);
        let b = c.reference("between", b, Kind::Biome);
        let inj = c.reference("biome", &mut e.biome, Kind::Biome);
        if let (Some(a), Some(b)) = (&a, &b) {
            if a == b {
                c.error("between: the two families must differ");
            }
            for f in [a, b] {
                if !out.families.contains_key(f) {
                    c.error(format!("between: `{f}` is a variant, not a family"));
                }
            }
            let (sa, sb) = (out.biomes[a].stratum, out.biomes[b].stratum);
            if sa != sb {
                c.error("between: the two families are in different strata");
            }
            if let Some(i) = &inj {
                let bi = &out.biomes[i];
                if bi.stratum != sa {
                    c.error("biome: the injected biome is in another stratum");
                }
                if bi.selectable {
                    c.warn("biome: the injected biome is selectable; it should set `selectable = false`");
                }
            }
        }
    }

    // ---- overlays (16.9, 17.7) ----
    let tags: BTreeSet<String> = out
        .biomes
        .values()
        .flat_map(|b| b.tags.iter().cloned())
        .collect();
    for (id, o) in out.overlays.iter_mut() {
        let (file, pack) = stash[id].clone();
        let mut c = Ctx {
            raw,
            report,
            file,
            entry: id.to_string(),
            pack,
        };
        match &mut o.target {
            Target::Id(t) => {
                c.reference("target", t, Kind::Biome);
            }
            Target::Tag(t) => {
                if !tags.contains(&t.tag) {
                    c.warn(format!("target: no biome has the tag `{}`", t.tag));
                }
            }
        }
        c.color("tint", &o.tint);
        if let Some(m) = &mut o.material_override {
            c.reference("material_override", m, Kind::Material);
        }
        c.scatter_rules("scatter_add", &mut o.scatter_add);
        for r in o.scatter_remove.iter_mut() {
            c.reference("scatter_remove", r, Kind::Scatter);
        }
    }

    // ---- the classification preset ----
    match raw.classification.as_slice() {
        [] => report.error("(content)", None, "no `[classification]` preset in any pack"),
        [(file, _, table)] => match typed::<Classification>(table) {
            Err(e) => report.error(file, Some("classification"), e),
            Ok(k) => {
                let mut c = Ctx { raw, report, file: file.clone(), entry: "classification".into(), pack: 0 };
                if k.sharpness < 1 || k.budget < 1 || k.palette_max.aerial < 1 || k.palette_max.benthic < 1 {
                    c.error("sharpness, budget and palette_max must be ≥ 1");
                }
                for (n, x) in [
                    ("epsilon", k.epsilon),
                    ("mosaic_scale", k.mosaic_scale),
                    ("jitter_wavelength", k.jitter_wavelength),
                    ("strata_blend", k.strata_blend),
                    ("cave_blend", k.cave_blend),
                ] {
                    if x.is_nan() || x <= 0.0 {
                        c.error(format!("{n} must be > 0"));
                    }
                }
                for (n, x) in [
                    ("mosaic_strength", k.mosaic_strength),
                    ("jitter.temp", k.jitter.temp),
                    ("jitter.moisture", k.jitter.moisture),
                    ("jitter.altitude", k.jitter.altitude),
                    ("jitter.slope", k.jitter.slope),
                    ("palette_drop", k.palette_drop),
                ] {
                    c.non_negative(n, x);
                }
                out.classification = Some(k);
            }
        },
        [.., (file, _, _)] => report.error(
            file,
            Some("classification"),
            "more than one `[classification]` preset (overriding one needs a patch, not supported yet)",
        ),
    }
    out
}
