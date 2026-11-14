//! `genesis`: command-line tools that need no window (docs/20).
//!
//! ```text
//! genesis descriptor --seed 42 [--region 0] [--system 0] [--body 0] [--compact]
//! genesis validate [data]
//! ```
//!
//! `descriptor` bakes a rocky body and prints its descriptor core as JSON on stdout; the identity
//! hash, sizes, and timings go to stderr, so the JSON can be piped.

use genesis_gen::{RockyParams, SeedChain, bake_caches, bake_core};
use std::process::ExitCode;
use std::time::Instant;

fn usage() -> ExitCode {
    eprintln!(
        "usage: genesis descriptor --seed <u64> [--region <u64>] [--system <u64>] [--body <u64>] [--compact]\n       genesis validate [<data dir>]"
    );
    ExitCode::from(2)
}

/// Accepts decimal or `0x`-prefixed hex.
fn parse_u64(s: &str) -> Option<u64> {
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(&hex.replace('_', ""), 16).ok(),
        None => s.replace('_', "").parse().ok(),
    }
}

/// `genesis validate [dir]`: load and validate every pack under `dir` (default `data`); print
/// every diagnostic; exit 1 if there is any error (docs/16 "Validation", docs/20 "Testing").
fn validate(dir: &str) -> ExitCode {
    let loaded = match genesis_content::load_dir(std::path::Path::new(dir)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot read `{dir}`: {e}");
            return ExitCode::FAILURE;
        }
    };
    for d in &loaded.report.diagnostics {
        eprintln!("{d}");
    }
    let (errors, warnings) = (loaded.report.errors(), loaded.report.warnings());
    match &loaded.content {
        Some(c) => {
            eprintln!(
                "ok: {} · {} entries ({} biomes in {} families, {} ecotones, {} overlays, {} suites, {} palettes, {} materials, {} scatter presets) · digest {:016X} · {warnings} warning(s)",
                c.content_version(),
                c.entries(),
                c.biomes.len(),
                c.families.len(),
                c.ecotones.len(),
                c.overlays.len(),
                c.suites.len(),
                c.palettes.len(),
                c.materials.len(),
                c.scatter.len(),
                c.digest
            );
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("failed: {errors} error(s), {warnings} warning(s)");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("validate") {
        return validate(args.get(1).map_or("data", String::as_str));
    }
    if args.first().map(String::as_str) != Some("descriptor") {
        return usage();
    }
    let value = |name: &str| -> Result<Option<u64>, ()> {
        match args.iter().position(|a| a == name) {
            None => Ok(None),
            Some(i) => args
                .get(i + 1)
                .and_then(|v| parse_u64(v))
                .map(Some)
                .ok_or(()),
        }
    };
    let (Ok(Some(galaxy)), Ok(region), Ok(system), Ok(body)) = (
        value("--seed"),
        value("--region"),
        value("--system"),
        value("--body"),
    ) else {
        return usage();
    };
    let chain = SeedChain {
        galaxy,
        region: region.unwrap_or(0),
        system: system.unwrap_or(0),
        body: body.unwrap_or(0),
    };
    let params = RockyParams::default();

    let t = Instant::now();
    let core = match bake_core(chain, &params) {
        Ok(core) => core,
        Err(e) => {
            eprintln!("bake failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    let core_ms = t.elapsed().as_secs_f64() * 1000.0;
    let t = Instant::now();
    let caches = bake_caches(&core, &params);
    let caches_ms = t.elapsed().as_secs_f64() * 1000.0;

    let json = if args.iter().any(|a| a == "--compact") {
        serde_json::to_string(&core)
    } else {
        serde_json::to_string_pretty(&core)
    };
    match json {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("serialisation failed: {e}");
            return ExitCode::FAILURE;
        }
    }
    eprintln!(
        "hash {:016X} · core {} bytes · bake core {:.1} ms, caches {:.1} ms · pyramid {} × {}² ({} KB) · crater index {} cells, {} entries",
        core.hash(),
        core.encode().len(),
        core_ms,
        caches_ms,
        6,
        caches.pyramid.n,
        caches.pyramid.data.len() * 4 / 1024,
        caches.crater_index.cells.len(),
        caches.crater_index.entries()
    );
    ExitCode::SUCCESS
}
