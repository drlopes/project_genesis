//! The content digest: a hash of the resolved content set, so that a world can detect content that
//! changed under an unchanged pack version (approved 2026-11-14; stored in the descriptor from
//! P1-C).
//!
//! Canonical encoding, like the descriptor's (docs/03 "Core versus caches"): the load-ordered
//! pack list (`id`, `version`), then every entry in ID order (full ID, then its resolved table),
//! then the classification preset. Tables are encoded with keys sorted, values tagged: strings
//! as `u32` length + UTF-8, integers as `i64`, floats as IEEE bits, booleans as a byte, arrays as
//! a count plus items, datetimes as their TOML text. Hash: `mix64(FNV-1a-64(bytes))`.

use crate::id::ContentId;
use genesis_core::seed::{fnv1a64, mix64};
use std::collections::BTreeMap;
use toml::{Table, Value};

#[derive(Default)]
struct Enc(Vec<u8>);

impl Enc {
    fn str(&mut self, s: &str) {
        self.0.extend_from_slice(&(s.len() as u32).to_le_bytes());
        self.0.extend_from_slice(s.as_bytes());
    }
    fn value(&mut self, v: &Value) {
        match v {
            Value::String(s) => {
                self.0.push(1);
                self.str(s);
            }
            Value::Integer(i) => {
                self.0.push(2);
                self.0.extend_from_slice(&i.to_le_bytes());
            }
            Value::Float(f) => {
                self.0.push(3);
                self.0.extend_from_slice(&f.to_bits().to_le_bytes());
            }
            Value::Boolean(b) => {
                self.0.push(4);
                self.0.push(u8::from(*b));
            }
            Value::Datetime(d) => {
                self.0.push(5);
                self.str(&d.to_string());
            }
            Value::Array(a) => {
                self.0.push(6);
                self.0.extend_from_slice(&(a.len() as u32).to_le_bytes());
                for x in a {
                    self.value(x);
                }
            }
            Value::Table(t) => {
                self.0.push(7);
                self.table(t);
            }
        }
    }
    fn table(&mut self, t: &Table) {
        // `toml::Table` is ordered by key (no `preserve_order` feature), so this is canonical.
        self.0.extend_from_slice(&(t.len() as u32).to_le_bytes());
        for (k, v) in t {
            self.str(k);
            self.value(v);
        }
    }
}

pub fn digest(
    packs: &[(String, String)],
    entries: &BTreeMap<ContentId, Table>,
    classification: Option<&Table>,
) -> u64 {
    let mut e = Enc::default();
    e.0.extend_from_slice(&(packs.len() as u32).to_le_bytes());
    for (id, version) in packs {
        e.str(id);
        e.str(version);
    }
    e.0.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (id, table) in entries {
        e.str(&id.to_string());
        e.table(table);
    }
    match classification {
        None => e.0.push(0),
        Some(t) => {
            e.0.push(1);
            e.table(t);
        }
    }
    mix64(fnv1a64(&e.0))
}
