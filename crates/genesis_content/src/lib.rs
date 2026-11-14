//! Content packs: schemas, loading, `extends` resolution, validation, and the content digest.
//!
//! See docs/16-data-contracts.md (IDs, references, merge rules, manifest, validation) and
//! docs/17-biome-and-landform-format.md (the schemas and their rules). No Bevy; no world-defining
//! maths. Everything is ordered (`BTreeMap`, sorted lists), so loading is deterministic.
//!
//! ```no_run
//! let loaded = genesis_content::load_dir(std::path::Path::new("data")).unwrap();
//! for d in &loaded.report.diagnostics {
//!     eprintln!("{d}");
//! }
//! let content = loaded.content.expect("valid content");
//! println!("{} ({:016X})", content.content_version(), content.digest);
//! ```

pub mod digest;
pub mod id;
pub mod load;
pub mod modules;
pub mod types;
pub mod validate;

pub use id::{ContentId, Diagnostic, Kind, Report, Severity};
pub use load::PackSource;

use std::collections::BTreeMap;
use types::*;

/// A loaded, validated content set. Every reference inside it is a full ID.
#[derive(Debug)]
pub struct Content {
    /// `(id, version)` in load order.
    pub packs: Vec<(String, String)>,
    pub biomes: BTreeMap<ContentId, Biome>,
    pub ecotones: BTreeMap<ContentId, Ecotone>,
    pub overlays: BTreeMap<ContentId, Overlay>,
    pub suites: BTreeMap<ContentId, Suite>,
    pub palettes: BTreeMap<ContentId, Palette>,
    pub materials: BTreeMap<ContentId, Material>,
    pub scatter: BTreeMap<ContentId, Scatter>,
    pub classification: Classification,
    /// Family root → members (root first, then variants by ID).
    pub families: BTreeMap<ContentId, Vec<ContentId>>,
    /// Hash of the resolved content set; see [`digest`].
    pub digest: u64,
}

impl Content {
    /// The resolved pack set, e.g. `core@0.1.0` or `core@0.1.0,mod.x@2.0.1` (load order).
    pub fn content_version(&self) -> String {
        self.packs
            .iter()
            .map(|(id, v)| format!("{id}@{v}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn entries(&self) -> usize {
        self.biomes.len()
            + self.ecotones.len()
            + self.overlays.len()
            + self.suites.len()
            + self.palettes.len()
            + self.materials.len()
            + self.scatter.len()
    }
}

/// The result of loading: the content if there were no errors, and every diagnostic.
pub struct Loaded {
    pub content: Option<Content>,
    pub report: Report,
}

/// Load and validate packs given as text (the core of [`load_dir`]; tests use it directly).
pub fn load_sources(sources: &[PackSource]) -> Loaded {
    let mut report = Report::default();
    let raw = load::parse(sources, &mut report);
    let (resolved, parents) = raw.resolve_extends(&mut report);
    let typed = validate::validate(&raw, &resolved, &parents, &mut report);
    report.diagnostics.sort_by(|a, b| {
        (&a.file, &a.entry, a.severity, &a.message)
            .cmp(&(&b.file, &b.entry, b.severity, &b.message))
    });
    let packs: Vec<(String, String)> = raw
        .packs
        .iter()
        .map(|m| (m.id.clone(), m.version.clone()))
        .collect();
    let content = match (report.errors(), typed.classification) {
        (0, Some(classification)) => Some(Content {
            digest: digest::digest(
                &packs,
                &resolved,
                raw.classification.first().map(|(_, _, t)| t),
            ),
            packs,
            biomes: typed.biomes,
            ecotones: typed.ecotones,
            overlays: typed.overlays,
            suites: typed.suites,
            palettes: typed.palettes,
            materials: typed.materials,
            scatter: typed.scatter,
            classification,
            families: typed.families,
        }),
        _ => None,
    };
    Loaded { content, report }
}

/// Load every pack under `root` (`root/<pack>/manifest.toml` and `root/mods/<mod>/…`).
pub fn load_dir(root: &std::path::Path) -> std::io::Result<Loaded> {
    Ok(load_sources(&load::read_root(root)?))
}
