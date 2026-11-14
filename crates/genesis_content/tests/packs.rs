//! Content loading and validation: the core pack resolves as intended, and every rule in docs/16
//! and docs/17 that this crate implements rejects a minimal bad input with its own diagnostic.

use genesis_content::types::{BiomeKind, ScatterRule};
use genesis_content::{ContentId, Kind, Loaded, PackSource, Severity, load_sources};
use std::path::Path;

fn core() -> PackSource {
    PackSource::read(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/core")).unwrap()
}

/// The core pack plus one extra file.
fn with(text: &str) -> Loaded {
    let mut p = core();
    p.files.push(("test/extra.toml".into(), text.into()));
    load_sources(&[p])
}

fn id(kind: Kind, name: &str) -> ContentId {
    ContentId::new("core", kind, name)
}

/// The first diagnostic of `severity` whose message contains `needle`, or a panic listing all.
fn expect(l: &Loaded, severity: Severity, needle: &str) {
    let found = l
        .report
        .diagnostics
        .iter()
        .any(|d| d.severity == severity && d.message.contains(needle));
    let all: Vec<String> = l
        .report
        .diagnostics
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        found,
        "expected {severity:?} containing `{needle}`, got:\n{}",
        all.join("\n")
    );
    if severity == Severity::Error {
        assert!(l.content.is_none(), "content must not load with errors");
    }
}

const BIOME_HEAD: &str = r#"display_name = "T"
kind = "biome"
stratum = "aerial"
palette = "green_temperate"
feature_scale = 1.0"#;

fn biome(name: &str, extra: &str, suitability: &str) -> String {
    format!("[biome.{name}]\n{BIOME_HEAD}\n{extra}\n\n[biome.{name}.suitability]\n{suitability}\n")
}

const TEMP: &str = "temp = { range = [270, 300], falloff = 8 }";

#[test]
fn core_pack_loads_clean() {
    let l = load_sources(&[core()]);
    let all: Vec<String> = l
        .report
        .diagnostics
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(all.is_empty(), "{}", all.join("\n"));
    let c = l.content.unwrap();
    assert_eq!(c.content_version(), "core@0.1.0");
    assert_eq!(c.biomes.len(), 10);
    assert_eq!(c.classification.mosaic_strength, 0.9);
}

#[test]
fn variant_inherits_merges_and_edits_lists() {
    let c = load_sources(&[core()]).content.unwrap();
    let v = &c.biomes[&id(Kind::Biome, "temperate_forest_alpine")];
    assert_eq!(v.kind, BiomeKind::Variant); // `kind` is never inherited
    assert_eq!(v.palette, "core:palette/alpine_stone"); // child scalar wins
    // Suitability merges one level: temp and altitude replaced, moisture and slope inherited.
    assert_eq!(v.suitability["temp"].range, Some([250.0, 275.0]));
    assert_eq!(v.suitability["moisture"].range, Some([0.5, 0.95]));
    assert!(v.suitability.contains_key("slope"));
    // scatter_remove matched the inline table by its preset; scatter_add appended.
    let presets: Vec<&str> = v.scatter.iter().map(ScatterRule::preset).collect();
    assert_eq!(presets, ["core:scatter/shrub", "core:scatter/conifer"]);
    assert_eq!(
        c.families[&id(Kind::Biome, "temperate_forest")],
        [
            id(Kind::Biome, "temperate_forest"),
            id(Kind::Biome, "temperate_forest_alpine")
        ]
    );
    // The ecotone biome is not a variant, so it is its own (non-selectable) family.
    assert!(c.families.contains_key(&id(Kind::Biome, "forest_edge")));
}

#[test]
fn references_become_full_ids() {
    let c = load_sources(&[core()]).content.unwrap();
    let e = &c.ecotones[&id(Kind::Ecotone, "forest_edge")];
    assert_eq!(e.between[0], "core:biome/temperate_forest");
    assert_eq!(e.biome, "core:biome/forest_edge");
    let s = &c.suites[&id(Kind::Suite, "dune_seas")];
    assert_eq!(
        s.context.biomes,
        ["core:biome/savanna", "core:biome/cold_steppe"]
    );
    assert_eq!(
        c.palettes[&id(Kind::Palette, "green_temperate")].base,
        "core:material/soil"
    );
}

#[test]
fn module_params_resolve_with_defaults() {
    let c = load_sources(&[core()]).content.unwrap();
    let s = &c.suites[&id(Kind::Suite, "rolling_hills")];
    let schema = genesis_content::modules::module(&s.module).unwrap();
    let full = genesis_content::modules::resolve_params(schema, &s.params);
    assert_eq!(full.len(), schema.params.len());
    assert_eq!(full["amplitude"].as_float(), Some(40.0));
}

#[test]
fn digest_is_stable_and_sensitive() {
    let a = load_sources(&[core()]).content.unwrap().digest;
    let mut p = core();
    p.files.reverse();
    assert_eq!(
        a,
        load_sources(&[p]).content.unwrap().digest,
        "file order must not matter"
    );
    // Same pack version, one value changed: the digest must change.
    let mut p = core();
    for (_, text) in &mut p.files {
        *text = text.replace("strength = 0.6", "strength = 0.7");
    }
    assert_ne!(a, load_sources(&[p]).content.unwrap().digest);
}

#[test]
fn shorthand_resolves_own_pack_first_then_dependencies_then_core() {
    let m = PackSource {
        dir: "mods/demo".into(),
        manifest:
            "id = \"demo\"\nversion = \"1.0.0\"\nschema_version = 1\ndependencies = [\"core\"]\n"
                .into(),
        files: vec![(
            "mods/demo/x.toml".into(),
            format!(
                "[palette.green_temperate]\nbase = \"soil\"\n\n{}",
                biome("glass_forest", "", TEMP)
            ),
        )],
    };
    let l = load_sources(&[m, core()]);
    let c = l.content.expect("loads");
    assert_eq!(c.content_version(), "core@0.1.0,demo@1.0.0"); // dependencies first
    let b = &c.biomes[&ContentId::new("demo", Kind::Biome, "glass_forest")];
    assert_eq!(b.palette, "demo:palette/green_temperate"); // own pack shadows core
    let p = &c.palettes[&ContentId::new("demo", Kind::Palette, "green_temperate")];
    assert_eq!(p.base, "core:material/soil"); // falls through to core
}

#[test]
fn aliases_resolve_retired_ids() {
    let mut p = core();
    p.manifest = p.manifest.replace(
        "aliases        = {}",
        "aliases = { \"core:biome/old_tundra\" = \"core:biome/tundra\" }",
    );
    p.files.push((
        "test/extra.toml".into(),
        "[ecotone.cold_edge]\nbetween = [\"old_tundra\", \"boreal_forest\"]\nbiome = \"forest_edge\"\n".into(),
    ));
    let c = load_sources(&[p]).content.expect("loads");
    assert_eq!(
        c.ecotones[&id(Kind::Ecotone, "cold_edge")].between[0],
        "core:biome/tundra"
    );
}

// ---- one rule each ----

#[test]
fn unknown_keys_are_errors() {
    expect(
        &with(&biome("x", "colour = 1", TEMP)),
        Severity::Error,
        "unknown field `colour`",
    );
}

#[test]
fn unknown_content_type_is_an_error() {
    expect(
        &with("[planet.x]\na = 1\n"),
        Severity::Error,
        "unknown content type",
    );
}

#[test]
fn dangling_and_mistyped_references_are_errors() {
    let bad = biome("x", "", TEMP).replace("green_temperate", "nope");
    expect(
        &with(&bad),
        Severity::Error,
        "palette `nope` does not exist",
    );
    let bad = biome("x", "", TEMP).replace("\"green_temperate\"", "\"core:material/soil\"");
    expect(
        &with(&bad),
        Severity::Error,
        "is a material, expected a palette",
    );
    expect(
        &with(&biome("x", "incompatible_with = [\"nope\"]", TEMP)),
        Severity::Error,
        "incompatible_with",
    );
}

#[test]
fn duplicate_ids_are_errors() {
    expect(
        &with(&biome("tundra", "", TEMP)),
        Severity::Error,
        "duplicate ID",
    );
}

#[test]
fn extends_cycles_and_depth_are_errors() {
    let cyc = "[palette.a]\nextends = \"b\"\n\n[palette.b]\nextends = \"a\"\n";
    expect(&with(cyc), Severity::Error, "cycle");
    let mut chain = String::from("[palette.p0]\nbase = \"soil\"\n");
    for i in 1..=9 {
        chain += &format!("\n[palette.p{i}]\nextends = \"p{}\"\n", i - 1);
    }
    expect(&with(&chain), Severity::Error, "deeper than 8");
}

#[test]
fn list_edits_need_a_parent() {
    expect(
        &with(&biome("x", "scatter_add = [\"shrub\"]", TEMP)),
        Severity::Error,
        "needs `extends`",
    );
}

#[test]
fn variants_must_extend_and_keep_their_stratum() {
    let v = biome("x", "", TEMP).replace("kind = \"biome\"", "kind = \"variant\"");
    expect(&with(&v), Severity::Error, "must `extends`");
    let s = "[biome.x]\nkind = \"variant\"\nextends = \"tundra\"\nstratum = \"benthic\"\n\n[biome.x.suitability]\ntemp = { range = [270, 300], falloff = 8 }\n";
    expect(&with(s), Severity::Error, "may not change the stratum");
}

#[test]
fn exactly_one_fallback_per_stratum() {
    expect(
        &with(&biome("x", "fallback = true", TEMP)),
        Severity::Error,
        "exactly one `fallback = true`",
    );
}

#[test]
fn suitability_rules() {
    let e = |s: &str| with(&biome("x", "", s));
    expect(
        &e("depth = { range = [0, 10], falloff = 1 }"),
        Severity::Error,
        "does not exist in the aerial stratum",
    );
    expect(
        &e("wetness = { range = [0, 1], falloff = 0.1 }"),
        Severity::Error,
        "unknown axis",
    );
    expect(
        &e("temp = { range = [300, 270], falloff = 8 }"),
        Severity::Error,
        "is not ordered",
    );
    expect(
        &e("moisture = { curve = [[0.5, 1], [0.2, 0]] }"),
        Severity::Error,
        "curve x values must increase",
    );
    expect(
        &e("substrate = { any_of = [\"lava\"] }"),
        Severity::Error,
        "is not one of",
    );
    expect(
        &e("temp = { range = [270, 300] }"),
        Severity::Error,
        "needs `falloff`",
    );
    expect(
        &e("temp = { range = [270, 300], falloff = 8, curve = [[0, 0], [1, 1]] }"),
        Severity::Error,
        "exactly one of",
    );
    let w = e("temp = { range = [270, 300], falloff = 0 }");
    expect(&w, Severity::Warning, "hard border");
    assert!(w.content.is_some(), "a warning does not block loading");
    let empty = format!("[biome.x]\n{BIOME_HEAD}\n");
    expect(&with(&empty), Severity::Error, "needs a suitability map");
}

#[test]
fn lod_and_feature_scale_rules() {
    let fs = |v: &str| {
        with(&biome("x", "", TEMP).replace("feature_scale = 1.0", &format!("feature_scale = {v}")))
    };
    expect(&fs("0.0"), Severity::Error, "feature_scale must be > 0");
    expect(&fs("0.2"), Severity::Warning, "below the voxel size");
    expect(
        &with(&biome("x", "min_lod = 4\nmax_lod = 2", TEMP)),
        Severity::Error,
        "min_lod 4 > max_lod 2",
    );
}

#[test]
fn suite_module_and_params_are_checked() {
    let suite = |module: &str, params: &str| {
        with(&format!(
            "[suite.s]\nmodule = \"{module}\"\nfeature_scale = 4.0\n\n[suite.s.params]\n{params}\n"
        ))
    };
    expect(
        &suite("volcano", ""),
        Severity::Error,
        "not in the registry",
    );
    expect(
        &suite("dune_field", "dune_height = 99.0"),
        Severity::Error,
        "outside",
    );
    expect(
        &suite("dune_field", "dune_hieght = 9.0"),
        Severity::Error,
        "unknown parameter `dune_hieght`",
    );
    expect(
        &suite("dune_field", "orientation = [0, 0, 0]"),
        Severity::Error,
        "must not be zero",
    );
}

#[test]
fn ecotone_rules() {
    let e = |between: &str| {
        with(&format!(
            "[ecotone.e]\nbetween = {between}\nbiome = \"forest_edge\"\n"
        ))
    };
    expect(
        &e("[\"tundra\", \"tundra\"]"),
        Severity::Error,
        "must differ",
    );
    expect(
        &e("[\"temperate_forest_alpine\", \"tundra\"]"),
        Severity::Error,
        "is a variant, not a family",
    );
    let w = with("[ecotone.e]\nbetween = [\"tundra\", \"savanna\"]\nbiome = \"boreal_forest\"\n");
    expect(&w, Severity::Warning, "should set `selectable = false`");
}

#[test]
fn overlay_targets_must_exist() {
    expect(
        &with("[overlay.o]\ntarget = \"nope\"\n"),
        Severity::Error,
        "target: biome `nope` does not exist",
    );
    expect(
        &with("[overlay.o]\ntarget = { tag = \"nope\" }\n"),
        Severity::Warning,
        "no biome has the tag",
    );
}

#[test]
fn colours_and_ranges_are_checked() {
    expect(
        &with("[material.m]\nalbedo = \"red\"\nroughness = 0.5\n"),
        Severity::Error,
        "not an sRGB",
    );
    expect(
        &with("[material.m]\nalbedo = \"#ff0000\"\nroughness = 2.0\n"),
        Severity::Error,
        "roughness must be in [0, 1]",
    );
    expect(
        &with("[scatter.s]\nanchor = \"volume\"\ndensity = 0.1\nscale = [1, 2]\nslope_max = 30\n"),
        Severity::Error,
        "apply to surface scatter",
    );
}

#[test]
fn manifest_rules() {
    let mut p = core();
    p.manifest = p
        .manifest
        .replace("schema_version = 1", "schema_version = 2");
    expect(
        &load_sources(&[p]),
        Severity::Error,
        "outside the supported range",
    );
    let mut p = core();
    p.manifest = p
        .manifest
        .replace("dependencies   = []", "dependencies = [\"missing\"]");
    expect(
        &load_sources(&[p]),
        Severity::Error,
        "missing or form a cycle",
    );
}

#[test]
fn the_classification_preset_is_required_and_unique() {
    let mut p = core();
    p.files.retain(|(f, _)| !f.ends_with("classification.toml"));
    expect(
        &load_sources(&[p]),
        Severity::Error,
        "no `[classification]` preset",
    );
    let mut p = core();
    let dup = p
        .files
        .iter()
        .find(|(f, _)| f.ends_with("classification.toml"))
        .unwrap()
        .1
        .clone();
    p.files.push(("test/second.toml".into(), dup));
    expect(
        &load_sources(&[p]),
        Severity::Error,
        "more than one `[classification]`",
    );
}
