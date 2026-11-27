// SPDX-License-Identifier: MIT
//! Splits a `.tscn` / `.tres` text file into sections without interpreting their values.

use crate::import::ImportError;

/// One `[tag key=value ...]` header and the `key = value` lines that follow it.
#[derive(Debug)]
pub(crate) struct Section {
    /// 1-based line of the header.
    pub line: usize,
    /// First word of the header, such as `node` or `ext_resource`.
    pub tag: String,
    /// Header attributes with their raw (unparsed) values.
    pub attrs: Vec<(String, String)>,
    /// Property lines: line number, key, raw value.
    pub props: Vec<(usize, String, String)>,
}

impl Section {
    /// The raw value of a header attribute.
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// Tracks brackets and strings across the characters of a value.
#[derive(Default)]
struct Balance {
    depth: i64,
    in_str: bool,
    esc: bool,
}

impl Balance {
    fn feed(&mut self, c: char) {
        if self.in_str {
            if self.esc {
                self.esc = false;
            } else if c == '\\' {
                self.esc = true;
            } else if c == '"' {
                self.in_str = false;
            }
            return;
        }
        match c {
            '"' => self.in_str = true,
            '[' | '{' | '(' => self.depth += 1,
            ']' | '}' | ')' => self.depth -= 1,
            _ => {}
        }
    }

    fn open(&self) -> bool {
        self.in_str || self.depth > 0
    }
}

fn err(line: usize, message: &str) -> ImportError {
    ImportError {
        line,
        message: message.to_string(),
    }
}

/// Reads a header line (starting with `[`) into a section.
fn parse_header(line_no: usize, line: &str) -> Result<Section, ImportError> {
    let chars: Vec<char> = line.trim_end().chars().collect();
    let mut i = 1;
    let skip_ws = |i: &mut usize| {
        while chars.get(*i).is_some_and(|c| c.is_whitespace()) {
            *i += 1;
        }
    };
    let mut tag = String::new();
    while let Some(&c) = chars.get(i) {
        if c.is_alphanumeric() || c == '_' {
            tag.push(c);
            i += 1;
        } else {
            break;
        }
    }
    if tag.is_empty() {
        return Err(err(line_no, "section header has no name"));
    }
    let mut attrs = Vec::new();
    loop {
        skip_ws(&mut i);
        match chars.get(i) {
            None => {
                return Err(err(
                    line_no,
                    "section header is missing its closing bracket",
                ))
            }
            Some(']') => {
                if chars
                    .get(i + 1..)
                    .is_some_and(|rest| rest.iter().any(|c| !c.is_whitespace()))
                {
                    return Err(err(line_no, "unexpected text after the section header"));
                }
                break;
            }
            Some(_) => {}
        }
        let mut key = String::new();
        while let Some(&c) = chars.get(i) {
            if c == '=' || c.is_whitespace() || c == ']' {
                break;
            }
            key.push(c);
            i += 1;
        }
        if chars.get(i) != Some(&'=') || key.is_empty() {
            return Err(err(
                line_no,
                "section header attribute needs a name and a value",
            ));
        }
        i += 1;
        let mut bal = Balance::default();
        let mut value = String::new();
        while let Some(&c) = chars.get(i) {
            if !bal.open() && (c.is_whitespace() || c == ']') {
                break;
            }
            bal.feed(c);
            value.push(c);
            i += 1;
        }
        if bal.open() {
            return Err(err(
                line_no,
                "unbalanced brackets or quotes in the section header",
            ));
        }
        attrs.push((key, value));
    }
    Ok(Section {
        line: line_no,
        tag,
        attrs,
        props: Vec::new(),
    })
}

/// Splits the key from the value on a property line.
fn split_property(line_no: usize, line: &str) -> Result<(String, String), ImportError> {
    let mut in_str = false;
    let mut esc = false;
    for (idx, c) in line.char_indices() {
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '=' => {
                let key = line.get(..idx).unwrap_or("").trim();
                let key = key
                    .strip_prefix('"')
                    .and_then(|k| k.strip_suffix('"'))
                    .unwrap_or(key);
                let value = line.get(idx + 1..).unwrap_or("").trim_start();
                if key.is_empty() {
                    return Err(err(line_no, "property has no name"));
                }
                return Ok((key.to_string(), value.to_string()));
            }
            _ => {}
        }
    }
    Err(err(line_no, "expected `name = value`"))
}

/// Splits `text` into sections. Blank lines and `;` comments are skipped; a property value may
/// span several lines until its brackets and strings balance.
pub(crate) fn split_sections(text: &str) -> Result<Vec<Section>, ImportError> {
    let mut sections: Vec<Section> = Vec::new();
    let mut pending: Option<(usize, String, String, Balance)> = None;
    for (idx, raw) in text.lines().enumerate() {
        let line_no = idx + 1;
        if let Some((start, key, mut value, mut bal)) = pending.take() {
            value.push('\n');
            value.push_str(raw);
            raw.chars().for_each(|c| bal.feed(c));
            if bal.open() {
                pending = Some((start, key, value, bal));
            } else if let Some(s) = sections.last_mut() {
                s.props.push((start, key, value.trim_end().to_string()));
            }
            continue;
        }
        let line = raw.trim_start_matches('\u{feff}');
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') {
            continue;
        }
        if trimmed.starts_with('[') {
            sections.push(parse_header(line_no, trimmed)?);
            continue;
        }
        if sections.is_empty() {
            return Err(err(line_no, "a property appears before any section header"));
        }
        let (key, value) = split_property(line_no, trimmed)?;
        let mut bal = Balance::default();
        value.chars().for_each(|c| bal.feed(c));
        if bal.open() {
            pending = Some((line_no, key, value, bal));
        } else if let Some(s) = sections.last_mut() {
            s.props.push((line_no, key, value.trim_end().to_string()));
        }
    }
    if let Some((start, ..)) = pending {
        return Err(err(
            start,
            "a value is missing its closing bracket or quote",
        ));
    }
    Ok(sections)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_headers_and_properties() {
        let t = "[gd_scene load_steps=2 format=3]\n\n; comment\n[node name=\"A B\" type=\"Node2D\"]\nposition = Vector2(1, 2)\n";
        let s = split_sections(t).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].tag, "node");
        assert_eq!(s[1].attr("name"), Some("\"A B\""));
        assert_eq!(s[1].props[0].1, "position");
        assert_eq!(s[1].props[0].2, "Vector2(1, 2)");
    }

    #[test]
    fn multiline_values_are_joined() {
        let t = "[resource]\na = [\n1,\n[2, 3]\n]\nb = \"x\ny\"\nc = 1\n";
        let s = split_sections(t).unwrap();
        assert_eq!(s[0].props.len(), 3);
        assert!(s[0].props[0].2.contains("[2, 3]"));
        assert_eq!(s[0].props[1].2, "\"x\ny\"");
    }

    #[test]
    fn nested_header_values() {
        let t = "[node name=\"a\" type=\"X\" groups=[\"g1\", \"g 2\"] parent=\".\"]\n";
        let s = split_sections(t).unwrap();
        assert_eq!(s[0].attr("groups"), Some("[\"g1\", \"g 2\"]"));
        assert_eq!(s[0].attr("parent"), Some("\".\""));
    }

    #[test]
    fn rejects_bad_files() {
        assert!(split_sections("a = 1\n").is_err());
        assert!(split_sections("[node name=\"a\"\n").is_err());
        assert!(split_sections("[node name=\"a\"] junk\n").is_err());
        assert!(split_sections("[resource]\nx = [1,\n").is_err());
        assert!(split_sections("[resource]\nnovalue\n").is_err());
        assert!(split_sections("[]\n").is_err());
    }

    #[test]
    fn quoted_keys() {
        let s = split_sections("[resource]\n\"metadata/x\" = 3\n").unwrap();
        assert_eq!(s[0].props[0].1, "metadata/x");
    }
}
