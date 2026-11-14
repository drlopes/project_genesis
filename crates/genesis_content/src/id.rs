//! Content IDs `<namespace>:<type>/<name>` (docs/16 "Content IDs") and diagnostics.

use std::fmt;

/// Content types this crate loads. The TOML table prefix declares the type (docs/17).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Biome,
    Ecotone,
    Overlay,
    Suite,
    Palette,
    Material,
    Scatter,
}

impl Kind {
    pub const ALL: [Kind; 7] = [
        Kind::Biome,
        Kind::Ecotone,
        Kind::Overlay,
        Kind::Suite,
        Kind::Palette,
        Kind::Material,
        Kind::Scatter,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Biome => "biome",
            Kind::Ecotone => "ecotone",
            Kind::Overlay => "overlay",
            Kind::Suite => "suite",
            Kind::Palette => "palette",
            Kind::Material => "material",
            Kind::Scatter => "scatter",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == s)
    }

    /// Types that may use `extends` (and therefore `<field>_add` / `<field>_remove` as merge
    /// operations). Overlays and ecotones do not inherit: an overlay's `scatter_add` is its own
    /// field, applied to its target (docs/17 "Overlays").
    pub fn inherits(self) -> bool {
        !matches!(self, Kind::Overlay | Kind::Ecotone)
    }
}

/// A fully qualified content ID.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentId {
    pub ns: String,
    pub kind: Kind,
    pub name: String,
}

impl ContentId {
    pub fn new(ns: &str, kind: Kind, name: &str) -> ContentId {
        ContentId {
            ns: ns.to_string(),
            kind,
            name: name.to_string(),
        }
    }

    /// Parse a full ID `ns:type/name`.
    pub fn parse(s: &str) -> Option<ContentId> {
        let (ns, rest) = s.split_once(':')?;
        let (ty, name) = rest.split_once('/')?;
        let kind = Kind::parse(ty)?;
        (valid_name(ns) && valid_name(name)).then(|| ContentId::new(ns, kind, name))
    }
}

impl fmt::Display for ContentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}/{}", self.ns, self.kind.as_str(), self.name)
    }
}

/// Names and namespaces: lowercase ASCII letters, digits, `_` and `-`, starting with a letter.
/// (docs/16 says both "lowercase snake_case" and `[A-Za-z0-9_]` plus `-`; lowercase wins, so IDs
/// never differ only by case.)
pub fn valid_name(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Warning,
    Error,
}

/// One validation finding, located by file and (where known) entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub file: String,
    pub entry: Option<String>,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            Severity::Warning => "warning",
            Severity::Error => "error",
        };
        match &self.entry {
            Some(e) => write!(f, "{sev}: {}: {e}: {}", self.file, self.message),
            None => write!(f, "{sev}: {}: {}", self.file, self.message),
        }
    }
}

/// Collects diagnostics.
#[derive(Default, Debug)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    pub fn error(&mut self, file: &str, entry: Option<&str>, message: impl Into<String>) {
        self.push(Severity::Error, file, entry, message.into());
    }

    pub fn warn(&mut self, file: &str, entry: Option<&str>, message: impl Into<String>) {
        self.push(Severity::Warning, file, entry, message.into());
    }

    fn push(&mut self, severity: Severity, file: &str, entry: Option<&str>, message: String) {
        self.diagnostics.push(Diagnostic {
            severity,
            file: file.to_string(),
            entry: entry.map(str::to_string),
            message,
        });
    }

    pub fn errors(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    pub fn warnings(&self) -> usize {
        self.diagnostics.len() - self.errors()
    }
}
