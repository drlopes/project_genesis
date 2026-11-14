//! The module registry: each landform-suite module's key, version, and typed parameter schema
//! (docs/13 "Module registry", docs/17 "Parameter types").
//!
//! The schemas live here, not in `genesis_gen`, because validation needs them and `genesis_gen`
//! depends on this crate (docs/20). The suite implementations themselves will live in
//! `genesis_gen` and must match the schema version declared here.

use toml::Value;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParamType {
    /// With bounds and a unit for display.
    Float {
        min: f64,
        max: f64,
        unit: &'static str,
    },
    Int {
        min: i64,
        max: i64,
    },
    Bool,
    Enum(&'static [&'static str]),
    /// Three floats; `normalised` vectors must not be zero.
    Vec3 {
        normalised: bool,
    },
    /// sRGB `#rrggbb`.
    Color,
    /// `[min, max]` with `min <= max`, within bounds.
    Range {
        min: f64,
        max: f64,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum DefaultValue {
    Float(f64),
    Int(i64),
    Bool(bool),
    Str(&'static str),
    Vec3([f64; 3]),
    Range([f64; 2]),
}

impl DefaultValue {
    pub fn to_value(self) -> Value {
        let arr = |v: &[f64]| Value::Array(v.iter().map(|x| Value::Float(*x)).collect());
        match self {
            DefaultValue::Float(x) => Value::Float(x),
            DefaultValue::Int(x) => Value::Integer(x),
            DefaultValue::Bool(x) => Value::Boolean(x),
            DefaultValue::Str(x) => Value::String(x.to_string()),
            DefaultValue::Vec3(v) => arr(&v),
            DefaultValue::Range(v) => arr(&v),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Param {
    pub name: &'static str,
    pub ty: ParamType,
    pub default: DefaultValue,
}

#[derive(Debug)]
pub struct ModuleSchema {
    pub key: &'static str,
    pub version: u32,
    pub params: &'static [Param],
}

/// Rolling hills: low smooth relief (docs/03's example params).
const ROLLING_HILLS: ModuleSchema = ModuleSchema {
    key: "rolling_hills",
    version: 1,
    params: &[
        Param {
            name: "amplitude",
            ty: ParamType::Float {
                min: 0.0,
                max: 300.0,
                unit: "m",
            },
            default: DefaultValue::Float(40.0),
        },
        Param {
            name: "wavelength",
            ty: ParamType::Float {
                min: 50.0,
                max: 5000.0,
                unit: "m",
            },
            default: DefaultValue::Float(900.0),
        },
    ],
};

/// Dune fields (docs/17's illustrative schema, made real).
const DUNE_FIELD: ModuleSchema = ModuleSchema {
    key: "dune_field",
    version: 1,
    params: &[
        Param {
            name: "dune_height",
            ty: ParamType::Float {
                min: 0.0,
                max: 50.0,
                unit: "m",
            },
            default: DefaultValue::Float(5.0),
        },
        Param {
            name: "dune_spacing",
            ty: ParamType::Float {
                min: 10.0,
                max: 500.0,
                unit: "m",
            },
            default: DefaultValue::Float(80.0),
        },
        Param {
            name: "asymmetry",
            ty: ParamType::Float {
                min: 0.0,
                max: 1.0,
                unit: "",
            },
            default: DefaultValue::Float(0.5),
        },
        Param {
            name: "dust_cover",
            ty: ParamType::Float {
                min: 0.0,
                max: 1.0,
                unit: "",
            },
            default: DefaultValue::Float(0.0),
        },
        Param {
            name: "orientation",
            ty: ParamType::Vec3 { normalised: true },
            default: DefaultValue::Vec3([0.0, 0.0, 1.0]),
        },
    ],
};

pub const REGISTRY: &[&ModuleSchema] = &[&DUNE_FIELD, &ROLLING_HILLS];

pub fn module(key: &str) -> Option<&'static ModuleSchema> {
    REGISTRY.iter().copied().find(|m| m.key == key)
}

fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Float(x) => Some(*x),
        Value::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

fn floats<const N: usize>(v: &Value) -> Option<[f64; N]> {
    let a = v.as_array()?;
    if a.len() != N {
        return None;
    }
    let mut out = [0.0; N];
    for (o, x) in out.iter_mut().zip(a) {
        *o = as_f64(x)?;
    }
    Some(out)
}

pub fn is_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Check one parameter value; `Err` describes the problem.
fn check(p: &Param, v: &Value) -> Result<(), String> {
    match p.ty {
        ParamType::Float { min, max, unit } => {
            let x = as_f64(v).ok_or("expected a number")?;
            if !(min..=max).contains(&x) {
                return Err(format!("{x} is outside [{min}, {max}] {unit}"));
            }
        }
        ParamType::Int { min, max } => {
            let x = v.as_integer().ok_or("expected an integer")?;
            if !(min..=max).contains(&x) {
                return Err(format!("{x} is outside [{min}, {max}]"));
            }
        }
        ParamType::Bool => {
            v.as_bool().ok_or("expected true or false")?;
        }
        ParamType::Enum(values) => {
            let s = v.as_str().ok_or("expected a string")?;
            if !values.contains(&s) {
                return Err(format!("`{s}` is not one of {values:?}"));
            }
        }
        ParamType::Vec3 { normalised } => {
            let a = floats::<3>(v).ok_or("expected three numbers")?;
            if normalised && a.iter().all(|x| *x == 0.0) {
                return Err("a direction must not be zero".into());
            }
        }
        ParamType::Color => {
            let s = v.as_str().ok_or("expected a colour string")?;
            if !is_color(s) {
                return Err(format!("`{s}` is not an sRGB `#rrggbb` colour"));
            }
        }
        ParamType::Range { min, max } => {
            let [a, b] = floats::<2>(v).ok_or("expected [min, max]")?;
            if a > b || a < min || b > max {
                return Err(format!(
                    "[{a}, {b}] is not an ordered range within [{min}, {max}]"
                ));
            }
        }
    }
    Ok(())
}

/// Validate content `params` against a module schema: unknown keys and bad values are errors;
/// omitted keys take the schema default (docs/13).
pub fn check_params(schema: &ModuleSchema, params: &toml::Table) -> Vec<String> {
    let mut errors = Vec::new();
    for key in params.keys() {
        if !schema.params.iter().any(|p| p.name == key) {
            errors.push(format!(
                "unknown parameter `{key}` for module `{}`",
                schema.key
            ));
        }
    }
    for p in schema.params {
        if let Some(v) = params.get(p.name)
            && let Err(e) = check(p, v)
        {
            errors.push(format!("parameter `{}`: {e}", p.name));
        }
    }
    errors
}

/// Every schema key present: content values where given, defaults otherwise (what a descriptor
/// will store, docs/13 "Descriptor params are fully resolved").
pub fn resolve_params(schema: &ModuleSchema, params: &toml::Table) -> toml::Table {
    schema
        .params
        .iter()
        .map(|p| {
            (
                p.name.to_string(),
                params
                    .get(p.name)
                    .cloned()
                    .unwrap_or_else(|| p.default.to_value()),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_satisfy_their_own_schemas() {
        for m in REGISTRY {
            let resolved = resolve_params(m, &toml::Table::new());
            assert!(check_params(m, &resolved).is_empty(), "{}", m.key);
        }
    }
}
