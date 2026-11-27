// SPDX-License-Identifier: MIT
//! The text form of a [`Variant`]: what scene and resource files store.
//!
//! ```text
//! nil  true  42  -1.5  "a \"quoted\" line\n"  vec2(1, 2)  vec3(1, 2, 3)
//! color(1, 0.5, 0, 1)  [1, 2, 3]  {"speed": 4.0, "tags": ["a", "b"]}
//! ```
//!
//! Parsing is total: any input returns a value or a [`ParseError`] with the offset where it went
//! wrong, and nesting is bounded by [`MAX_DEPTH`].

use crate::variant::{Variant, MAX_DEPTH};
use orbe_math::{Color, Vec2, Vec3};
use std::collections::BTreeMap;
use std::fmt;

/// Why a value could not be parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// Character offset where parsing stopped.
    pub offset: usize,
    /// What was wrong.
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (at character {})", self.message, self.offset)
    }
}

impl std::error::Error for ParseError {}

impl Variant {
    /// The value as text, such that [`Variant::parse`] gives it back.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        write_value(self, &mut out);
        out
    }

    /// Parses one value from the start of `text` and returns it with the text after it.
    pub fn parse_prefix(text: &str) -> Result<(Variant, &str), ParseError> {
        let mut p = Parser {
            chars: text.chars().collect(),
            pos: 0,
        };
        p.skip_space();
        let v = p.value(0)?;
        let consumed: usize = p.chars.iter().take(p.pos).map(|c| c.len_utf8()).sum();
        Ok((v, text.get(consumed..).unwrap_or("")))
    }

    /// Parses a value from its text form. Trailing text is an error.
    pub fn parse(text: &str) -> Result<Variant, ParseError> {
        let mut p = Parser {
            chars: text.chars().collect(),
            pos: 0,
        };
        p.skip_space();
        let v = p.value(0)?;
        p.skip_space();
        if p.pos < p.chars.len() {
            return Err(p.err("unexpected text after the value"));
        }
        Ok(v)
    }
}

fn write_float(f: f64, out: &mut String) {
    if f.is_nan() {
        out.push_str("nan");
    } else if f.is_infinite() {
        out.push_str(if f > 0.0 { "inf" } else { "-inf" });
    } else {
        out.push_str(&format!("{f:?}"));
    }
}

fn write_f32(f: f32, out: &mut String) {
    write_float(f64::from(f), out);
}

/// Writes `s` as a quoted string.
pub(crate) fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_value(v: &Variant, out: &mut String) {
    match v {
        Variant::Nil => out.push_str("nil"),
        Variant::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Variant::Int(i) => out.push_str(&i.to_string()),
        Variant::Float(f) => write_float(*f, out),
        Variant::Str(s) => write_string(s, out),
        Variant::Vec2(p) => {
            out.push_str("vec2(");
            write_f32(p.x, out);
            out.push_str(", ");
            write_f32(p.y, out);
            out.push(')');
        }
        Variant::Vec3(p) => {
            out.push_str("vec3(");
            for (i, c) in [p.x, p.y, p.z].into_iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_f32(c, out);
            }
            out.push(')');
        }
        Variant::Color(c) => {
            out.push_str("color(");
            for (i, ch) in [c.r, c.g, c.b, c.a].into_iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_f32(ch, out);
            }
            out.push(')');
        }
        Variant::Array(a) => {
            out.push('[');
            for (i, item) in a.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_value(item, out);
            }
            out.push(']');
        }
        Variant::Dict(d) => {
            out.push('{');
            for (i, (k, item)) in d.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_string(k, out);
                out.push_str(": ");
                write_value(item, out);
            }
            out.push('}');
        }
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn err(&self, message: &str) -> ParseError {
        ParseError {
            offset: self.pos,
            message: message.to_string(),
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, c: char) -> Result<(), ParseError> {
        self.skip_space();
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.err(&format!("expected `{c}`")))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Variant, ParseError> {
        if depth > MAX_DEPTH {
            return Err(self.err("values are nested too deeply"));
        }
        self.skip_space();
        match self.peek() {
            None => Err(self.err("expected a value")),
            Some('"') => Ok(Variant::Str(self.string()?)),
            Some('[') => self.array(depth),
            Some('{') => self.dict(depth),
            Some(c) if c.is_ascii_digit() || c == '-' || c == '+' || c == '.' => {
                self.number_or_word()
            }
            Some(c) if c.is_ascii_alphabetic() => self.word(depth),
            Some(_) => Err(self.err("unexpected character")),
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
        self.chars
            .get(start..self.pos)
            .map(|s| s.iter().collect())
            .unwrap_or_default()
    }

    fn word(&mut self, _depth: usize) -> Result<Variant, ParseError> {
        let start = self.pos;
        let name = self.ident();
        match name.as_str() {
            "nil" => Ok(Variant::Nil),
            "true" => Ok(Variant::Bool(true)),
            "false" => Ok(Variant::Bool(false)),
            "nan" => Ok(Variant::Float(f64::NAN)),
            "inf" => Ok(Variant::Float(f64::INFINITY)),
            "vec2" => match self.floats(2)?.as_slice() {
                [x, y] => Ok(Variant::Vec2(Vec2::new(*x, *y))),
                _ => Err(self.err("expected two numbers")),
            },
            "vec3" => match self.floats(3)?.as_slice() {
                [x, y, z] => Ok(Variant::Vec3(Vec3::new(*x, *y, *z))),
                _ => Err(self.err("expected three numbers")),
            },
            "color" => match self.floats(4)?.as_slice() {
                [r, g, b, a] => Ok(Variant::Color(Color::new(*r, *g, *b, *a))),
                _ => Err(self.err("expected four numbers")),
            },
            _ => {
                self.pos = start;
                Err(self.err("unknown word"))
            }
        }
    }

    /// `(a, b, ...)` with exactly `count` numbers.
    fn floats(&mut self, count: usize) -> Result<Vec<f32>, ParseError> {
        self.eat('(')?;
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            if i > 0 {
                self.eat(',')?;
            }
            self.skip_space();
            match self.number_or_word()? {
                Variant::Int(n) => out.push(n as f32),
                Variant::Float(n) => out.push(n as f32),
                _ => return Err(self.err("expected a number")),
            }
        }
        self.eat(')')?;
        Ok(out)
    }

    fn number_or_word(&mut self) -> Result<Variant, ParseError> {
        let start = self.pos;
        let negative = self.peek() == Some('-');
        if negative || self.peek() == Some('+') {
            self.pos += 1;
            if self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                let name = self.ident();
                return match (negative, name.as_str()) {
                    (true, "inf") => Ok(Variant::Float(f64::NEG_INFINITY)),
                    (false, "inf") => Ok(Variant::Float(f64::INFINITY)),
                    _ => {
                        self.pos = start;
                        Err(self.err("expected a number"))
                    }
                };
            }
        }
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
        {
            self.pos += 1;
        }
        let token: String = self
            .chars
            .get(start..self.pos)
            .map(|s| s.iter().collect())
            .unwrap_or_default();
        let is_float = token.contains(['.', 'e', 'E']);
        let bad = || ParseError {
            offset: start,
            message: format!("`{token}` is not a number"),
        };
        if is_float {
            token.parse::<f64>().map(Variant::Float).map_err(|_| bad())
        } else {
            token.parse::<i64>().map(Variant::Int).map_err(|_| bad())
        }
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.eat('"')?;
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.err("unterminated string"));
            };
            self.pos += 1;
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let Some(e) = self.peek() else {
                        return Err(self.err("unterminated escape"));
                    };
                    self.pos += 1;
                    match e {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => out.push(self.unicode()?),
                        _ => return Err(self.err("unknown escape")),
                    }
                }
                c => out.push(c),
            }
        }
    }

    fn unicode(&mut self) -> Result<char, ParseError> {
        if self.peek() != Some('{') {
            return Err(self.err("expected `{` after \\u"));
        }
        self.pos += 1;
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
            self.pos += 1;
        }
        let digits: String = self
            .chars
            .get(start..self.pos)
            .map(|s| s.iter().collect())
            .unwrap_or_default();
        if self.peek() != Some('}') || digits.is_empty() || digits.len() > 6 {
            return Err(self.err("malformed \\u{...} escape"));
        }
        self.pos += 1;
        u32::from_str_radix(&digits, 16)
            .ok()
            .and_then(char::from_u32)
            .ok_or_else(|| self.err("not a valid character"))
    }

    fn array(&mut self, depth: usize) -> Result<Variant, ParseError> {
        self.eat('[')?;
        let mut items = Vec::new();
        self.skip_space();
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(Variant::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_space();
            match self.peek() {
                Some(',') => self.pos += 1,
                Some(']') => {
                    self.pos += 1;
                    return Ok(Variant::Array(items));
                }
                _ => return Err(self.err("expected `,` or `]`")),
            }
        }
    }

    fn dict(&mut self, depth: usize) -> Result<Variant, ParseError> {
        self.eat('{')?;
        let mut map = BTreeMap::new();
        self.skip_space();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(Variant::Dict(map));
        }
        loop {
            self.skip_space();
            let key = self.string()?;
            self.eat(':')?;
            let v = self.value(depth + 1)?;
            map.insert(key, v);
            self.skip_space();
            match self.peek() {
                Some(',') => self.pos += 1,
                Some('}') => {
                    self.pos += 1;
                    return Ok(Variant::Dict(map));
                }
                _ => return Err(self.err("expected `,` or `}`")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(v: Variant) {
        let text = v.to_text();
        let back = Variant::parse(&text).unwrap_or_else(|e| panic!("{text}: {e}"));
        assert_eq!(back, v, "{text}");
    }

    #[test]
    fn scalars_round_trip() {
        round_trip(Variant::Nil);
        round_trip(Variant::Bool(true));
        round_trip(Variant::Int(i64::MIN));
        round_trip(Variant::Int(i64::MAX));
        round_trip(Variant::Float(0.1));
        round_trip(Variant::Float(-1e300));
        round_trip(Variant::Float(f64::INFINITY));
        round_trip(Variant::Float(f64::NEG_INFINITY));
        round_trip(Variant::Float(3.0));
    }

    #[test]
    fn nan_round_trips_as_nan() {
        let back = Variant::parse(&Variant::Float(f64::NAN).to_text()).unwrap();
        assert!(matches!(back, Variant::Float(f) if f.is_nan()));
    }

    #[test]
    fn strings_with_escapes_round_trip() {
        round_trip(Variant::from("plain"));
        round_trip(Variant::from(
            "a \"quoted\" \\ line\nwith\ttabs\r and \u{1} control and ñ and 日本",
        ));
        round_trip(Variant::from(""));
    }

    #[test]
    fn composites_round_trip() {
        round_trip(Variant::Vec2(Vec2::new(1.5, -2.0)));
        round_trip(Variant::Vec3(Vec3::new(0.1, 0.2, 0.3)));
        round_trip(Variant::Color(Color::new(1.0, 0.5, 0.25, 0.125)));
        let mut d = BTreeMap::new();
        d.insert(
            "a".to_string(),
            Variant::Array(vec![Variant::Int(1), Variant::Nil]),
        );
        d.insert("b\"".to_string(), Variant::Dict(BTreeMap::new()));
        round_trip(Variant::Dict(d));
        round_trip(Variant::Array(vec![]));
    }

    #[test]
    fn accepts_whitespace_and_integer_vectors() {
        assert_eq!(
            Variant::parse(" vec2( 1 ,2 ) ").unwrap(),
            Variant::Vec2(Vec2::new(1.0, 2.0))
        );
        assert_eq!(
            Variant::parse("[ 1 , 2 ]").unwrap(),
            Variant::Array(vec![Variant::Int(1), Variant::Int(2)])
        );
    }

    #[test]
    fn rejects_malformed_input_without_panicking() {
        for bad in [
            "",
            " ",
            "[",
            "[1,",
            "[1 2]",
            "{",
            "{\"a\"}",
            "{\"a\": }",
            "\"open",
            "\"bad \\q\"",
            "\"\\u{}\"",
            "\"\\u{110000}\"",
            "\"\\u{zz}\"",
            "1 2",
            "vec2(1)",
            "vec2(1, 2, 3)",
            "vec2(\"a\", 1)",
            "color(1,2,3)",
            "foo",
            "-",
            "+",
            "--1",
            "1.2.3",
            "99999999999999999999",
            "nil nil",
            "@",
            "-foo",
            "{1: 2}",
        ] {
            assert!(Variant::parse(bad).is_err(), "{bad:?} should fail");
        }
    }

    #[test]
    fn deep_nesting_is_rejected_not_overflowed() {
        let deep = format!("{}1{}", "[".repeat(10_000), "]".repeat(10_000));
        assert!(Variant::parse(&deep).is_err());
        let ok = format!("{}1{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
        assert!(Variant::parse(&ok).is_ok());
    }

    #[test]
    fn parse_prefix_returns_the_rest() {
        let (v, rest) = Variant::parse_prefix("\"ñ\" parent \".\"").unwrap();
        assert_eq!(v, Variant::from("ñ"));
        assert_eq!(rest, " parent \".\"");
        assert!(Variant::parse_prefix("").is_err());
    }

    #[test]
    fn error_reports_offset() {
        let e = Variant::parse("[1, @]").unwrap_err();
        assert_eq!(e.offset, 4);
    }
}
