// SPDX-License-Identifier: MIT
use orbe_math::{Color, Vec2, Vec3};
use std::collections::BTreeMap;

/// Deepest nesting of arrays and dictionaries that [`Variant::within_depth`] accepts.
pub const MAX_DEPTH: usize = 64;

/// A dynamically typed value: what scene files, resources and scripts exchange.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Variant {
    /// No value.
    #[default]
    Nil,
    /// Boolean.
    Bool(bool),
    /// 64-bit integer.
    Int(i64),
    /// 64-bit float.
    Float(f64),
    /// UTF-8 string.
    Str(String),
    /// 2D vector.
    Vec2(Vec2),
    /// 3D vector.
    Vec3(Vec3),
    /// Colour.
    Color(Color),
    /// Ordered list.
    Array(Vec<Variant>),
    /// String-keyed map (sorted, so output is deterministic).
    Dict(BTreeMap<String, Variant>),
}

impl Variant {
    /// A short, stable name for the type, for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Variant::Nil => "nil",
            Variant::Bool(_) => "bool",
            Variant::Int(_) => "int",
            Variant::Float(_) => "float",
            Variant::Str(_) => "string",
            Variant::Vec2(_) => "vec2",
            Variant::Vec3(_) => "vec3",
            Variant::Color(_) => "color",
            Variant::Array(_) => "array",
            Variant::Dict(_) => "dict",
        }
    }

    /// The boolean, if this is one.
    pub fn as_bool(&self) -> Option<bool> {
        if let Variant::Bool(b) = self {
            Some(*b)
        } else {
            None
        }
    }

    /// The integer, if this is one.
    pub fn as_int(&self) -> Option<i64> {
        if let Variant::Int(i) = self {
            Some(*i)
        } else {
            None
        }
    }

    /// The number as a float: floats as they are, integers converted.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Variant::Float(f) => Some(*f),
            Variant::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// The string, if this is one.
    pub fn as_str(&self) -> Option<&str> {
        if let Variant::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }

    /// Looks up a key in a dictionary.
    pub fn get(&self, key: &str) -> Option<&Variant> {
        if let Variant::Dict(d) = self {
            d.get(key)
        } else {
            None
        }
    }

    /// True when arrays and dictionaries nest at most `limit` levels deep. Untrusted values
    /// should be checked against [`MAX_DEPTH`] before anything recurses over them.
    pub fn within_depth(&self, limit: usize) -> bool {
        match self {
            Variant::Array(a) => limit > 0 && a.iter().all(|v| v.within_depth(limit - 1)),
            Variant::Dict(d) => limit > 0 && d.values().all(|v| v.within_depth(limit - 1)),
            _ => true,
        }
    }
}

impl From<bool> for Variant {
    fn from(v: bool) -> Self {
        Variant::Bool(v)
    }
}
impl From<i64> for Variant {
    fn from(v: i64) -> Self {
        Variant::Int(v)
    }
}
impl From<f64> for Variant {
    fn from(v: f64) -> Self {
        Variant::Float(v)
    }
}
impl From<&str> for Variant {
    fn from(v: &str) -> Self {
        Variant::Str(v.to_string())
    }
}
impl From<Vec2> for Variant {
    fn from(v: Vec2) -> Self {
        Variant::Vec2(v)
    }
}
impl From<Vec3> for Variant {
    fn from(v: Vec3) -> Self {
        Variant::Vec3(v)
    }
}
impl From<Color> for Variant {
    fn from(v: Color) -> Self {
        Variant::Color(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessors_and_numeric_widening() {
        assert_eq!(Variant::from(3_i64).as_float(), Some(3.0));
        assert_eq!(Variant::from(2.5).as_int(), None);
        assert_eq!(Variant::from("hi").as_str(), Some("hi"));
        assert_eq!(Variant::Nil.type_name(), "nil");
    }

    #[test]
    fn dict_lookup() {
        let mut d = BTreeMap::new();
        d.insert("speed".to_string(), Variant::from(4.0));
        let v = Variant::Dict(d);
        assert_eq!(v.get("speed").and_then(Variant::as_float), Some(4.0));
        assert!(v.get("missing").is_none());
        assert!(Variant::Nil.get("speed").is_none());
    }

    #[test]
    fn depth_limit_rejects_deep_nesting() {
        let mut v = Variant::Nil;
        for _ in 0..10 {
            v = Variant::Array(vec![v]);
        }
        assert!(v.within_depth(10));
        assert!(!v.within_depth(9));
        assert!(Variant::Nil.within_depth(0));
    }
}
