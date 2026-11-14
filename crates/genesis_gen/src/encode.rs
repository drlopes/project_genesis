//! Canonical binary encoding and the identity hash.
//!
//! Our own, so that no serialisation crate's upgrade can change world identity (docs/20
//! determinism rule 2). The hash is FNV-1a over the bytes, finished with the SplitMix64 mixer.

use crate::descriptor::Deferred;
use genesis_core::seed::{fnv1a64, mix64};

#[derive(Default)]
pub struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    pub fn u8(&mut self, v: u8) {
        self.bytes.push(v);
    }
    pub fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }
    pub fn u32(&mut self, v: u32) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f64(&mut self, v: f64) {
        self.u64(v.to_bits());
    }
    pub fn vec3(&mut self, v: [f64; 3]) {
        for x in v {
            self.f64(x);
        }
    }
    pub fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.bytes.extend_from_slice(s.as_bytes());
    }
    pub fn opt_str(&mut self, s: Option<&str>) {
        match s {
            None => self.u8(0),
            Some(s) => {
                self.u8(1);
                self.str(s);
            }
        }
    }
    pub fn opt_f64(&mut self, v: Option<f64>) {
        match v {
            None => self.u8(0),
            Some(v) => {
                self.u8(1);
                self.f64(v);
            }
        }
    }
    pub fn opt_bool(&mut self, v: Option<bool>) {
        self.u8(match v {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        });
    }
    /// An optional block: a tag byte (0 absent, 1 present), then its fields if present.
    pub fn opt<T>(&mut self, v: Option<&T>, f: impl FnOnce(&mut Encoder, &T)) {
        match v {
            None => self.u8(0),
            Some(x) => {
                self.u8(1);
                f(self, x);
            }
        }
    }
    /// A deferred block: always absent today; encoded so that filling it changes the bytes.
    pub fn deferred(&mut self, d: Option<Deferred>) {
        self.u8(u8::from(d.is_some()));
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// FNV-1a 64 over the encoding, then `mix64` so that nearby inputs spread over all bits.
pub fn hash(bytes: &[u8]) -> u64 {
    mix64(fnv1a64(bytes))
}
