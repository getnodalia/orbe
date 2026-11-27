// SPDX-License-Identifier: MIT
//! Reads one value in the upstream text syntax into a [`Variant`].

use orbe_core::{Variant, MAX_DEPTH};
use orbe_math::{Color, Vec2, Vec3};
use std::collections::BTreeMap;

/// What the value parser needs to know about the file it reads from.
pub(crate) struct Env<'a> {
    /// External resource id -> path.
    pub ext: &'a BTreeMap<String, String>,
    /// Problems found while reading (unsupported value types).
    pub warnings: Vec<String>,
}

/// Parses `text` as one value. Trailing text is an error.
pub(crate) fn parse_value(text: &str, env: &mut Env) -> Result<Variant, String> {
    let mut p = P {
        chars: text.chars().collect(),
        pos: 0,
    };
    let v = p.value(0, env)?;
    p.skip_ws();
    if p.pos < p.chars.len() {
        return Err("unexpected text after the value".to_string());
    }
    Ok(v)
}

fn marker(key: &str, v: Variant) -> Variant {
    let mut d = BTreeMap::new();
    d.insert(key.to_string(), v);
    Variant::Dict(d)
}

struct P {
    chars: Vec<char>,
    pos: usize,
}

impl P {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, c: char) -> Result<(), String> {
        self.skip_ws();
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("expected `{c}`"))
        }
    }

    fn slice(&self, from: usize) -> String {
        self.chars
            .get(from..self.pos)
            .map(|s| s.iter().collect())
            .unwrap_or_default()
    }

    fn value(&mut self, depth: usize, env: &mut Env) -> Result<Variant, String> {
        if depth > MAX_DEPTH {
            return Err("values are nested too deeply".to_string());
        }
        self.skip_ws();
        match self.peek() {
            None => Err("expected a value".to_string()),
            Some('"') => Ok(Variant::Str(self.string()?)),
            Some('&') => {
                self.pos += 1;
                Ok(Variant::Str(self.string()?))
            }
            Some('^') => {
                self.pos += 1;
                Ok(marker("$nodepath", Variant::Str(self.string()?)))
            }
            Some('[') => self.array(depth, env),
            Some('{') => self.dict(depth, env),
            Some(c) if c.is_ascii_digit() || matches!(c, '-' | '+' | '.') => self.number(),
            Some(c) if c.is_ascii_alphabetic() || c == '_' => self.word(depth, env),
            Some(_) => Err("unexpected character".to_string()),
        }
    }

    fn ident(&mut self) -> String {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            self.pos += 1;
        }
        self.slice(start)
    }

    fn number(&mut self) -> Result<Variant, String> {
        let start = self.pos;
        if matches!(self.peek(), Some('-' | '+')) {
            self.pos += 1;
            if self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                let name = self.ident();
                let neg = self.chars.get(start) == Some(&'-');
                return match name.as_str() {
                    "inf" => Ok(Variant::Float(if neg {
                        f64::NEG_INFINITY
                    } else {
                        f64::INFINITY
                    })),
                    "nan" => Ok(Variant::Float(f64::NAN)),
                    _ => Err("expected a number".to_string()),
                };
            }
        }
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
        {
            self.pos += 1;
        }
        let token = self.slice(start);
        if token.contains(['.', 'e', 'E']) {
            token
                .parse::<f64>()
                .map(Variant::Float)
                .map_err(|_| format!("`{token}` is not a number"))
        } else {
            token
                .parse::<i64>()
                .map(Variant::Int)
                .map_err(|_| format!("`{token}` is not a number"))
        }
    }

    fn string(&mut self) -> Result<String, String> {
        if self.peek() != Some('"') {
            return Err("expected a string".to_string());
        }
        self.pos += 1;
        let mut out = String::new();
        loop {
            let c = self.peek().ok_or("unterminated string")?;
            self.pos += 1;
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let e = self.peek().ok_or("unterminated escape")?;
                    self.pos += 1;
                    match e {
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'u' | 'U' => {
                            let digits = if e == 'u' { 4 } else { 6 };
                            let mut code = 0_u32;
                            for _ in 0..digits {
                                let d = self
                                    .peek()
                                    .and_then(|c| c.to_digit(16))
                                    .ok_or("malformed unicode escape")?;
                                self.pos += 1;
                                code = code * 16 + d;
                            }
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        }
                        other => out.push(other),
                    }
                }
                c => out.push(c),
            }
        }
    }

    fn array(&mut self, depth: usize, env: &mut Env) -> Result<Variant, String> {
        self.eat('[')?;
        Ok(Variant::Array(self.list(']', depth, env)?))
    }

    /// Comma-separated values up to and including `close`.
    fn list(&mut self, close: char, depth: usize, env: &mut Env) -> Result<Vec<Variant>, String> {
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(close) {
            self.pos += 1;
            return Ok(items);
        }
        loop {
            items.push(self.value(depth + 1, env)?);
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.pos += 1;
                    self.skip_ws();
                    if self.peek() == Some(close) {
                        self.pos += 1;
                        return Ok(items);
                    }
                }
                Some(c) if c == close => {
                    self.pos += 1;
                    return Ok(items);
                }
                _ => return Err(format!("expected `,` or `{close}`")),
            }
        }
    }

    fn dict(&mut self, depth: usize, env: &mut Env) -> Result<Variant, String> {
        self.eat('{')?;
        let mut map = BTreeMap::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(Variant::Dict(map));
        }
        loop {
            let key = match self.value(depth + 1, env)? {
                Variant::Str(s) => s,
                other => other.to_text(),
            };
            self.eat(':')?;
            let v = self.value(depth + 1, env)?;
            map.insert(key, v);
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.pos += 1;
                    self.skip_ws();
                    if self.peek() == Some('}') {
                        self.pos += 1;
                        return Ok(Variant::Dict(map));
                    }
                }
                Some('}') => {
                    self.pos += 1;
                    return Ok(Variant::Dict(map));
                }
                _ => return Err("expected `,` or `}`".to_string()),
            }
        }
    }

    /// Skips a balanced `(...)`, `[...]` or `{...}` group, respecting strings.
    fn skip_group(&mut self) -> Result<(), String> {
        let mut depth = 0_i32;
        let mut in_str = false;
        let mut escape = false;
        while let Some(c) = self.peek() {
            self.pos += 1;
            if in_str {
                if escape {
                    escape = false;
                } else if c == '\\' {
                    escape = true;
                } else if c == '"' {
                    in_str = false;
                }
                continue;
            }
            match c {
                '"' => in_str = true,
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
        Err("unbalanced brackets".to_string())
    }

    fn word(&mut self, depth: usize, env: &mut Env) -> Result<Variant, String> {
        let start = self.pos;
        let name = self.ident();
        match name.as_str() {
            "null" => return Ok(Variant::Nil),
            "true" => return Ok(Variant::Bool(true)),
            "false" => return Ok(Variant::Bool(false)),
            "inf" => return Ok(Variant::Float(f64::INFINITY)),
            "nan" => return Ok(Variant::Float(f64::NAN)),
            _ => {}
        }
        self.skip_ws();
        // Typed containers: Array[Type]([...]) and Dictionary[K, V]({...}).
        if self.peek() == Some('[') && matches!(name.as_str(), "Array" | "Dictionary") {
            self.skip_group()?;
            self.skip_ws();
        }
        if self.peek() != Some('(') {
            return Err(format!("unknown word `{name}`"));
        }
        let open = self.pos;
        match name.as_str() {
            "Array" | "Dictionary" => {
                self.pos += 1;
                let mut args = self.list(')', depth, env)?;
                if args.len() == 1 {
                    return Ok(args.remove(0));
                }
                Ok(Variant::Array(args))
            }
            "Vector2" | "Vector2i" | "Vector3" | "Vector3i" | "Color" => {
                self.pos += 1;
                let nums = self.numbers(')', depth, env)?;
                match (name.as_str(), nums.as_slice()) {
                    ("Vector2" | "Vector2i", [x, y]) => Ok(Variant::Vec2(Vec2::new(*x, *y))),
                    ("Vector3" | "Vector3i", [x, y, z]) => Ok(Variant::Vec3(Vec3::new(*x, *y, *z))),
                    ("Color", [r, g, b]) => Ok(Variant::Color(Color::new(*r, *g, *b, 1.0))),
                    ("Color", [r, g, b, a]) => Ok(Variant::Color(Color::new(*r, *g, *b, *a))),
                    _ => Err(format!("`{name}` has the wrong number of components")),
                }
            }
            "ExtResource" | "SubResource" | "NodePath" | "StringName" => {
                self.pos += 1;
                let arg = self.value(depth + 1, env)?;
                self.eat(')')?;
                let Variant::Str(s) = arg else {
                    return Err(format!("`{name}` takes a string"));
                };
                Ok(match name.as_str() {
                    "ExtResource" => match env.ext.get(&s) {
                        Some(path) => marker("$ext", Variant::Str(path.clone())),
                        None => {
                            env.warnings
                                .push(format!("there is no external resource with id `{s}`"));
                            marker("$ext", Variant::Nil)
                        }
                    },
                    "SubResource" => marker("$sub", Variant::Str(s)),
                    "NodePath" => marker("$nodepath", Variant::Str(s)),
                    _ => Variant::Str(s),
                })
            }
            "Rect2" | "Rect2i" | "Transform2D" | "Transform3D" | "Basis" | "Quaternion"
            | "Plane" | "AABB" | "Vector4" | "Vector4i" | "Projection" => {
                self.pos += 1;
                let nums = self.numbers(')', depth, env)?;
                let key = format!("${}", name.to_ascii_lowercase());
                Ok(marker(
                    &key,
                    Variant::Array(
                        nums.into_iter()
                            .map(|n| Variant::Float(f64::from(n)))
                            .collect(),
                    ),
                ))
            }
            "PackedStringArray" | "PackedInt32Array" | "PackedInt64Array"
            | "PackedFloat32Array" | "PackedFloat64Array" | "PackedVector2Array"
            | "PackedVector3Array" | "PackedColorArray" => {
                self.pos += 1;
                Ok(Variant::Array(self.list(')', depth, env)?))
            }
            _ => {
                self.pos = open;
                self.skip_group()?;
                let raw = self.slice(start);
                env.warnings
                    .push(format!("unsupported value `{name}(...)` kept as raw text"));
                Ok(marker("$raw", Variant::Str(raw)))
            }
        }
    }

    fn numbers(&mut self, close: char, depth: usize, env: &mut Env) -> Result<Vec<f32>, String> {
        self.list(close, depth, env)?
            .into_iter()
            .map(|v| match v {
                Variant::Int(i) => Ok(i as f32),
                Variant::Float(f) => Ok(f as f32),
                _ => Err("expected a number".to_string()),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str, ext: &[(&str, &str)]) -> (Result<Variant, String>, Vec<String>) {
        let ext: BTreeMap<String, String> = ext
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let mut env = Env {
            ext: &ext,
            warnings: Vec::new(),
        };
        let r = parse_value(text, &mut env);
        (r, env.warnings)
    }

    fn ok(text: &str) -> Variant {
        parse(text, &[]).0.unwrap()
    }

    #[test]
    fn scalars_and_strings() {
        assert_eq!(ok("null"), Variant::Nil);
        assert_eq!(ok("true"), Variant::Bool(true));
        assert_eq!(ok("-12"), Variant::Int(-12));
        assert_eq!(ok("1.5e2"), Variant::Float(150.0));
        assert_eq!(ok("-inf"), Variant::Float(f64::NEG_INFINITY));
        assert_eq!(ok("\"a\\\"b\\n\\u00f1\""), Variant::from("a\"b\nñ"));
        assert_eq!(ok("&\"name\""), Variant::from("name"));
        assert_eq!(ok("StringName(\"x\")"), Variant::from("x"));
    }

    #[test]
    fn vectors_and_colors() {
        assert_eq!(ok("Vector2(1, 2.5)"), Variant::Vec2(Vec2::new(1.0, 2.5)));
        assert_eq!(ok("Vector2i(3, 4)"), Variant::Vec2(Vec2::new(3.0, 4.0)));
        assert_eq!(
            ok("Vector3(1, 2, 3)"),
            Variant::Vec3(Vec3::new(1.0, 2.0, 3.0))
        );
        assert_eq!(
            ok("Color(1, 0.5, 0, 1)"),
            Variant::Color(Color::new(1.0, 0.5, 0.0, 1.0))
        );
        assert_eq!(
            ok("Color(1, 0, 0)"),
            Variant::Color(Color::new(1.0, 0.0, 0.0, 1.0))
        );
    }

    #[test]
    fn containers() {
        assert_eq!(
            ok("[1, 2, ]"),
            Variant::Array(vec![Variant::Int(1), Variant::Int(2)])
        );
        assert_eq!(ok("Array[int]([1])"), Variant::Array(vec![Variant::Int(1)]));
        assert_eq!(
            ok("PackedStringArray(\"a\", \"b\")"),
            Variant::Array(vec![Variant::from("a"), Variant::from("b")])
        );
        let d = ok("{\n\"a\": 1,\n2: [3]\n}");
        assert_eq!(d.get("a"), Some(&Variant::Int(1)));
        assert_eq!(d.get("2"), Some(&Variant::Array(vec![Variant::Int(3)])));
    }

    #[test]
    fn resources_and_markers() {
        let (v, w) = parse("ExtResource(\"1_ab\")", &[("1_ab", "res://icon.png")]);
        assert_eq!(
            v.unwrap().get("$ext"),
            Some(&Variant::from("res://icon.png"))
        );
        assert!(w.is_empty());
        let (v, w) = parse("ExtResource(\"nope\")", &[]);
        assert_eq!(v.unwrap().get("$ext"), Some(&Variant::Nil));
        assert_eq!(w.len(), 1);
        assert_eq!(
            ok("SubResource(\"s1\")").get("$sub"),
            Some(&Variant::from("s1"))
        );
        assert_eq!(
            ok("NodePath(\"../A\")").get("$nodepath"),
            Some(&Variant::from("../A"))
        );
        let t = ok("Transform2D(1, 0, 0, 1, 5, 6)");
        assert_eq!(
            t.get("$transform2d")
                .map(|v| matches!(v, Variant::Array(a) if a.len() == 6)),
            Some(true)
        );
    }

    #[test]
    fn unknown_constructors_are_kept_raw_with_a_warning() {
        let (v, w) = parse("Object(Foo,\"a\":1,\"b\":[2,3])", &[]);
        assert_eq!(
            v.unwrap().get("$raw"),
            Some(&Variant::from("Object(Foo,\"a\":1,\"b\":[2,3])"))
        );
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn rejects_malformed_values() {
        for bad in [
            "",
            "[",
            "[1 2]",
            "{\"a\"}",
            "\"open",
            "Vector2(1)",
            "Vector2(\"a\", 1)",
            "ExtResource(1)",
            "foo",
            "-",
            "1 2",
            "Object(",
            "Array[int]",
            "99999999999999999999",
            "\"\\u12\"",
        ] {
            assert!(parse(bad, &[]).0.is_err(), "{bad:?}");
        }
    }

    #[test]
    fn deep_nesting_is_rejected() {
        let deep = format!("{}1{}", "[".repeat(10_000), "]".repeat(10_000));
        assert!(parse(&deep, &[]).0.is_err());
    }
}
