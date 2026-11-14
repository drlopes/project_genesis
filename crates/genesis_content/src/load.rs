//! Pack loading: manifests, deterministic pack and file order, raw entries, shorthand resolution,
//! and `extends` (docs/16 "File layout and manifest", "References", "Override and extension").

use crate::id::{ContentId, Kind, Report, valid_name};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use toml::{Table, Value};

/// Pack schema versions this engine reads (docs/16: a pack outside the range fails to load).
pub const SUPPORTED_SCHEMA: std::ops::RangeInclusive<u32> = 1..=1;
/// Longest `extends` chain (docs/16 suggests 8).
pub const MAX_EXTENDS_DEPTH: usize = 8;

/// One pack's text: its manifest and its content files (paths are for messages and ordering).
#[derive(Clone, Debug)]
pub struct PackSource {
    pub dir: String,
    pub manifest: String,
    pub files: Vec<(String, String)>,
}

impl PackSource {
    /// Read a pack directory: `manifest.toml` and every other `*.toml` beneath it, path-sorted.
    pub fn read(dir: &Path) -> std::io::Result<PackSource> {
        let manifest = std::fs::read_to_string(dir.join("manifest.toml"))?;
        let mut files = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for entry in std::fs::read_dir(&d)? {
                let p = entry?.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|e| e == "toml")
                    && p.file_name().is_some_and(|n| n != "manifest.toml")
                {
                    let rel = p.strip_prefix(dir).unwrap_or(&p);
                    let name = rel.to_string_lossy().replace('\\', "/");
                    files.push((
                        format!("{}/{name}", dir.display()),
                        std::fs::read_to_string(&p)?,
                    ));
                }
            }
        }
        files.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(PackSource {
            dir: dir.display().to_string().replace('\\', "/"),
            manifest,
            files,
        })
    }
}

/// `data/<pack>/` for every subdirectory of `root` that has a manifest, plus `root/mods/<mod>/`.
pub fn read_root(root: &Path) -> std::io::Result<Vec<PackSource>> {
    let mut dirs = Vec::new();
    for base in [root.to_path_buf(), root.join("mods")] {
        if !base.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&base)? {
            let p = entry?.path();
            if p.is_dir() && p.join("manifest.toml").is_file() {
                dirs.push(p);
            }
        }
    }
    dirs.sort();
    dirs.iter().map(|d| PackSource::read(d)).collect()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// The pack ID, which is also its namespace.
    pub id: String,
    /// The pack version, `major.minor.patch`.
    pub version: String,
    pub schema_version: u32,
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Old full content ID → current full content ID.
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

pub struct RawEntry {
    pub id: ContentId,
    pub file: String,
    /// Index into the load order.
    pub pack: usize,
    pub table: Table,
}

/// Everything parsed, in load order, before typing.
pub struct Raw {
    pub packs: Vec<Manifest>,
    pub entries: BTreeMap<ContentId, RawEntry>,
    pub classification: Vec<(String, usize, Table)>,
    aliases: BTreeMap<String, ContentId>,
}

fn is_version(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Parse manifests and files. Problems go to `report`; what parsed is returned.
pub fn parse(sources: &[PackSource], report: &mut Report) -> Raw {
    // ---- manifests and load order (dependencies first, then by ID) ----
    let mut manifests: Vec<(Manifest, &PackSource)> = Vec::new();
    for s in sources {
        let file = format!("{}/manifest.toml", s.dir);
        match toml::from_str::<Manifest>(&s.manifest) {
            Err(e) => report.error(&file, None, format!("manifest: {}", e.message())),
            Ok(m) => {
                if !valid_name(&m.id) {
                    report.error(
                        &file,
                        None,
                        format!("pack id `{}` is not a valid name", m.id),
                    );
                } else if !is_version(&m.version) {
                    report.error(
                        &file,
                        None,
                        format!("version `{}` is not `major.minor.patch`", m.version),
                    );
                } else if !SUPPORTED_SCHEMA.contains(&m.schema_version) {
                    report.error(
                        &file,
                        None,
                        format!(
                            "schema_version {} is outside the supported range {:?}",
                            m.schema_version, SUPPORTED_SCHEMA
                        ),
                    );
                } else if manifests.iter().any(|(o, _)| o.id == m.id) {
                    report.error(&file, None, format!("pack `{}` is loaded twice", m.id));
                } else {
                    manifests.push((m, s));
                }
            }
        }
    }
    let mut order: Vec<(Manifest, &PackSource)> = Vec::new();
    let mut remaining = manifests;
    remaining.sort_by(|a, b| a.0.id.cmp(&b.0.id));
    while !remaining.is_empty() {
        let placed: BTreeSet<&str> = order.iter().map(|(m, _)| m.id.as_str()).collect();
        let next = remaining
            .iter()
            .position(|(m, _)| m.dependencies.iter().all(|d| placed.contains(d.as_str())));
        match next {
            Some(i) => {
                let p = remaining.remove(i);
                order.push(p);
            }
            None => {
                for (m, s) in &remaining {
                    let missing: Vec<&String> = m
                        .dependencies
                        .iter()
                        .filter(|d| !placed.contains(d.as_str()))
                        .collect();
                    report.error(
                        &format!("{}/manifest.toml", s.dir),
                        None,
                        format!("dependencies {missing:?} are missing or form a cycle"),
                    );
                }
                break;
            }
        }
    }

    // ---- entries ----
    let mut entries: BTreeMap<ContentId, RawEntry> = BTreeMap::new();
    let mut classification = Vec::new();
    for (pack, (m, s)) in order.iter().enumerate() {
        for (file, text) in &s.files {
            let table = match toml::from_str::<Table>(text) {
                Ok(t) => t,
                Err(e) => {
                    report.error(file, None, format!("TOML: {}", e.message()));
                    continue;
                }
            };
            for (prefix, value) in table {
                if prefix == "classification" {
                    match value {
                        Value::Table(t) => classification.push((file.clone(), pack, t)),
                        _ => report.error(file, None, "`classification` must be a table"),
                    }
                    continue;
                }
                let Some(kind) = Kind::parse(&prefix) else {
                    report.error(file, None, format!("unknown content type `[{prefix}.…]`"));
                    continue;
                };
                let Value::Table(items) = value else {
                    report.error(file, None, format!("`{prefix}` must contain named tables"));
                    continue;
                };
                for (name, body) in items {
                    let id = ContentId::new(&m.id, kind, &name);
                    let shown = id.to_string();
                    if !valid_name(&name) {
                        report.error(
                            file,
                            Some(&shown),
                            "names are lowercase ASCII, digits, `_`, `-`",
                        );
                        continue;
                    }
                    let Value::Table(table) = body else {
                        report.error(file, Some(&shown), "an entry must be a table");
                        continue;
                    };
                    if let Some(prev) = entries.get(&id) {
                        report.error(
                            file,
                            Some(&shown),
                            format!("duplicate ID (also in {})", prev.file),
                        );
                        continue;
                    }
                    entries.insert(
                        id.clone(),
                        RawEntry {
                            id,
                            file: file.clone(),
                            pack,
                            table,
                        },
                    );
                }
            }
        }
    }

    // ---- aliases ----
    let mut aliases = BTreeMap::new();
    for (m, s) in &order {
        let file = format!("{}/manifest.toml", s.dir);
        for (old, new) in &m.aliases {
            match (ContentId::parse(old), ContentId::parse(new)) {
                (Some(o), Some(n)) if o.kind == n.kind => {
                    if entries.contains_key(&o) {
                        report.error(&file, None, format!("alias `{old}` is still a declared ID"));
                    } else if !entries.contains_key(&n) {
                        report.error(&file, None, format!("alias target `{new}` does not exist"));
                    } else {
                        aliases.insert(o.to_string(), n);
                    }
                }
                _ => report.error(
                    &file,
                    None,
                    format!("alias `{old}` → `{new}`: both must be full IDs of the same type"),
                ),
            }
        }
    }

    Raw {
        packs: order.into_iter().map(|(m, _)| m).collect(),
        entries,
        classification,
        aliases,
    }
}

impl Raw {
    /// Resolve a reference written in `pack` (docs/16): a full ID as is; a shorthand name in the
    /// pack's own namespace first, then its dependencies in order, then `core`. Aliases apply to
    /// IDs that no longer exist.
    pub fn resolve(&self, text: &str, kind: Kind, pack: usize) -> Result<ContentId, String> {
        let candidates: Vec<ContentId> = if text.contains(':') {
            let id = ContentId::parse(text)
                .ok_or_else(|| format!("`{text}` is not a valid content ID"))?;
            if id.kind != kind {
                return Err(format!(
                    "`{text}` is a {}, expected a {}",
                    id.kind.as_str(),
                    kind.as_str()
                ));
            }
            vec![id]
        } else {
            if !valid_name(text) {
                return Err(format!("`{text}` is not a valid {} name", kind.as_str()));
            }
            let own = &self.packs[pack];
            let mut ns: Vec<&str> = vec![own.id.as_str()];
            ns.extend(own.dependencies.iter().map(String::as_str));
            ns.push("core");
            ns.into_iter()
                .map(|n| ContentId::new(n, kind, text))
                .collect()
        };
        for id in &candidates {
            if self.entries.contains_key(id) {
                return Ok(id.clone());
            }
            if let Some(target) = self.aliases.get(&id.to_string()) {
                return Ok(target.clone());
            }
        }
        Err(format!("{} `{text}` does not exist", kind.as_str()))
    }

    /// Resolve `extends` for every inheriting entry. Returns the resolved tables (without
    /// `extends`) and each entry's parent.
    pub fn resolve_extends(
        &self,
        report: &mut Report,
    ) -> (BTreeMap<ContentId, Table>, BTreeMap<ContentId, ContentId>) {
        let mut done: BTreeMap<ContentId, (Table, usize)> = BTreeMap::new();
        let mut parents: BTreeMap<ContentId, ContentId> = BTreeMap::new();
        let mut failed: BTreeSet<ContentId> = BTreeSet::new();
        for id in self.entries.keys() {
            let mut stack = Vec::new();
            self.resolve_one(id, &mut stack, &mut done, &mut parents, &mut failed, report);
        }
        let done = done.into_iter().map(|(k, (t, _))| (k, t)).collect();
        (done, parents)
    }

    fn resolve_one(
        &self,
        id: &ContentId,
        stack: &mut Vec<ContentId>,
        done: &mut BTreeMap<ContentId, (Table, usize)>,
        parents: &mut BTreeMap<ContentId, ContentId>,
        failed: &mut BTreeSet<ContentId>,
        report: &mut Report,
    ) -> Option<(Table, usize)> {
        if let Some(t) = done.get(id) {
            return Some(t.clone());
        }
        if failed.contains(id) {
            return None;
        }
        let entry = &self.entries[id];
        let shown = id.to_string();
        let fail = |report: &mut Report, msg: String| {
            report.error(&entry.file, Some(&shown), msg);
            None
        };
        if stack.contains(id) {
            failed.insert(id.clone());
            return fail(report, "`extends` forms a cycle".into());
        }
        let table = &entry.table;
        let ops: Vec<&String> = table
            .keys()
            .filter(|k| k.ends_with("_add") || k.ends_with("_remove"))
            .collect();

        // The chain length is the parent's plus one, whatever order entries resolve in (an
        // already-resolved parent does not deepen the recursion, so the stack cannot measure it).
        let mut depth = 0;
        let resolved = match table.get("extends") {
            None => {
                if id.kind.inherits() && !ops.is_empty() {
                    failed.insert(id.clone());
                    return fail(
                        report,
                        format!("`{}` needs `extends` (it edits a parent's list)", ops[0]),
                    );
                }
                table.clone()
            }
            Some(parent) => {
                if !id.kind.inherits() {
                    failed.insert(id.clone());
                    return fail(
                        report,
                        format!("a {} cannot use `extends`", id.kind.as_str()),
                    );
                }
                let Some(text) = parent.as_str() else {
                    failed.insert(id.clone());
                    return fail(
                        report,
                        "`extends` names exactly one parent (a string)".into(),
                    );
                };
                let pid = match self.resolve(text, id.kind, entry.pack) {
                    Ok(p) => p,
                    Err(e) => {
                        failed.insert(id.clone());
                        return fail(report, format!("extends: {e}"));
                    }
                };
                stack.push(id.clone());
                let base = self.resolve_one(&pid, stack, done, parents, failed, report);
                stack.pop();
                let Some((mut base, parent_depth)) = base else {
                    failed.insert(id.clone());
                    return None;
                };
                depth = parent_depth + 1;
                if depth > MAX_EXTENDS_DEPTH {
                    failed.insert(id.clone());
                    return fail(
                        report,
                        format!("`extends` chain is deeper than {MAX_EXTENDS_DEPTH}"),
                    );
                }
                parents.insert(id.clone(), pid);
                // `kind` is never inherited (docs/17): the child states what it is.
                base.remove("kind");
                match merge(base, table) {
                    Ok(t) => t,
                    Err(e) => {
                        failed.insert(id.clone());
                        return fail(report, e);
                    }
                }
            }
        };
        let mut resolved = resolved;
        resolved.remove("extends");
        done.insert(id.clone(), (resolved.clone(), depth));
        Some((resolved, depth))
    }
}

/// docs/16 merge rules: scalars and lists replace; tables merge one level (child keys win) unless
/// the child's table sets `_replace = true`; `<f>_add` appends to the parent's list `f` and
/// `<f>_remove` removes items from it (a string matches an equal string or a table whose
/// `preset` or `suite` equals it).
fn merge(mut base: Table, child: &Table) -> Result<Table, String> {
    let mut adds: Vec<(&str, &Value)> = Vec::new();
    let mut removes: Vec<(&str, &Value)> = Vec::new();
    for (k, v) in child {
        if k == "extends" {
            continue;
        }
        if let Some(f) = k.strip_suffix("_add") {
            adds.push((f, v));
            continue;
        }
        if let Some(f) = k.strip_suffix("_remove") {
            removes.push((f, v));
            continue;
        }
        match (base.get_mut(k), v) {
            (Some(Value::Table(b)), Value::Table(c))
                if c.get("_replace") != Some(&Value::Boolean(true)) =>
            {
                for (ck, cv) in c {
                    b.insert(ck.clone(), cv.clone());
                }
            }
            _ => {
                let mut v = v.clone();
                if let Value::Table(t) = &mut v {
                    t.remove("_replace");
                }
                base.insert(k.clone(), v);
            }
        }
    }
    for (f, v) in removes {
        if child.contains_key(f) {
            return Err(format!(
                "`{f}` and `{f}_remove` together: replace the list or edit it, not both"
            ));
        }
        let items = v.as_array().ok_or(format!("`{f}_remove` must be a list"))?;
        let list = match base.get_mut(f) {
            Some(Value::Array(a)) => a,
            _ => return Err(format!("`{f}_remove`: the parent has no list `{f}`")),
        };
        for r in items {
            let r = r.as_str().ok_or(format!("`{f}_remove` items are names"))?;
            let before = list.len();
            list.retain(|x| !matches_name(x, r));
            if list.len() == before {
                return Err(format!("`{f}_remove`: `{r}` is not in the parent's `{f}`"));
            }
        }
    }
    for (f, v) in adds {
        if child.contains_key(f) {
            return Err(format!(
                "`{f}` and `{f}_add` together: replace the list or edit it, not both"
            ));
        }
        let items = v.as_array().ok_or(format!("`{f}_add` must be a list"))?;
        match base.get_mut(f) {
            Some(Value::Array(a)) => a.extend(items.iter().cloned()),
            None => {
                base.insert(f.to_string(), Value::Array(items.clone()));
            }
            Some(_) => return Err(format!("`{f}_add`: the parent's `{f}` is not a list")),
        }
    }
    Ok(base)
}

fn matches_name(item: &Value, name: &str) -> bool {
    match item {
        Value::String(s) => s == name,
        Value::Table(t) => ["preset", "suite"]
            .iter()
            .any(|k| t.get(*k).and_then(Value::as_str) == Some(name)),
        _ => false,
    }
}
