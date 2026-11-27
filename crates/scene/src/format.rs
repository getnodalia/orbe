// SPDX-License-Identifier: MIT
//! Orbe's scene file format (`.orbe`), a line-based text format:
//!
//! ```text
//! orbe_scene 1
//!
//! node "World" "Node2D"
//!
//! node "Player" "Node2D" parent "."
//! position = vec2(10, 20)
//!
//! node "Sprite" "Sprite2D" parent "Player"
//! texture = "res://player.png"
//! ```
//!
//! The first `node` is the root and has no `parent`. `parent` is a path from the root, and a
//! parent always comes before its children. Blank lines and lines starting with `#` are ignored.
//! Values use [`Variant`]'s text form and must fit on one line.

use crate::tree::{valid_kind, valid_property};
use crate::{FormatError, SceneTree};
use orbe_core::Variant;

/// The format version this crate writes and reads.
pub const FORMAT_VERSION: u32 = 1;
/// Largest scene file accepted, in bytes.
pub const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;

fn err(line: usize, message: impl Into<String>) -> FormatError {
    FormatError {
        line,
        message: message.into(),
    }
}

impl SceneTree {
    /// The scene as `.orbe` text. Parsing it gives back an equal tree.
    pub fn to_text(&self) -> String {
        let mut out = format!("orbe_scene {FORMAT_VERSION}\n");
        let mut stack = vec![(self.root(), ".".to_string())];
        while let Some((h, path)) = stack.pop() {
            let Some(n) = self.node(h) else { continue };
            out.push('\n');
            out.push_str("node ");
            out.push_str(&Variant::from(n.name()).to_text());
            out.push(' ');
            out.push_str(&Variant::from(n.kind()).to_text());
            if let Some(parent) = n.parent() {
                // The parent path is the node's own path minus its last segment.
                let parent_path = self.path_of(parent).unwrap_or_else(|| ".".to_string());
                out.push_str(" parent ");
                out.push_str(&Variant::Str(parent_path).to_text());
            }
            out.push('\n');
            for (k, v) in n.properties() {
                out.push_str(k);
                out.push_str(" = ");
                out.push_str(&v.to_text());
                out.push('\n');
            }
            for c in n.children().iter().rev() {
                let Some(child) = self.node(*c) else { continue };
                let child_path = if path == "." {
                    child.name().to_string()
                } else {
                    format!("{path}/{}", child.name())
                };
                stack.push((*c, child_path));
            }
        }
        out
    }

    /// Reads a scene from `.orbe` text.
    pub fn from_text(text: &str) -> Result<SceneTree, FormatError> {
        if text.len() > MAX_FILE_BYTES {
            return Err(err(0, "the scene file is too large"));
        }
        let mut lines = text
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.trim()))
            .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'));
        let (hl, header) = lines
            .next()
            .ok_or_else(|| err(0, "the scene file is empty"))?;
        let version = header
            .strip_prefix("orbe_scene")
            .map(str::trim)
            .ok_or_else(|| err(hl, "expected `orbe_scene <version>`"))?;
        match version.parse::<u32>() {
            Ok(FORMAT_VERSION) => {}
            Ok(v) => {
                return Err(err(
                    hl,
                    format!("scene format version {v} is not supported"),
                ))
            }
            Err(_) => return Err(err(hl, "the version must be a number")),
        }

        let mut tree: Option<SceneTree> = None;
        let mut current = None;
        for (ln, line) in lines {
            if let Some(rest) = line.strip_prefix("node ") {
                let (name, rest) = quoted(rest, ln)?;
                let (kind, rest) = quoted(rest, ln)?;
                if !valid_kind(&kind) {
                    return Err(err(ln, "invalid node type"));
                }
                let rest = rest.trim();
                match (tree.as_mut(), rest) {
                    (None, "") => {
                        tree =
                            Some(SceneTree::new(&name, &kind).map_err(|e| err(ln, e.to_string()))?);
                        current = tree.as_ref().map(SceneTree::root);
                    }
                    (None, _) => {
                        return Err(err(ln, "the first node is the root and has no parent"))
                    }
                    (Some(_), "") => return Err(err(ln, "only the first node may omit `parent`")),
                    (Some(t), rest) => {
                        let rest = rest
                            .strip_prefix("parent")
                            .ok_or_else(|| err(ln, "expected `parent \"<path>\"`"))?;
                        let (path, tail) = quoted(rest, ln)?;
                        if !tail.trim().is_empty() {
                            return Err(err(ln, "unexpected text after the parent path"));
                        }
                        let root = t.root();
                        let parent = t
                            .find(root, &path)
                            .ok_or_else(|| err(ln, format!("there is no node at `{path}`")))?;
                        current = Some(
                            t.add_child(parent, &name, &kind)
                                .map_err(|e| err(ln, e.to_string()))?,
                        );
                    }
                }
            } else if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                if !valid_property(key) {
                    return Err(err(ln, "invalid property name"));
                }
                let value = Variant::parse(value).map_err(|e| err(ln, e.to_string()))?;
                let (Some(t), Some(h)) = (tree.as_mut(), current) else {
                    return Err(err(ln, "a property must follow a `node` line"));
                };
                t.set_property(h, key, value)
                    .map_err(|e| err(ln, e.to_string()))?;
            } else {
                return Err(err(ln, "expected a `node` line or `key = value`"));
            }
        }
        tree.ok_or_else(|| err(0, "the scene has no nodes"))
    }
}

/// A quoted string at the start of `s`, and the text after it.
fn quoted(s: &str, line: usize) -> Result<(String, &str), FormatError> {
    let s = s.trim_start();
    if !s.starts_with('"') {
        return Err(err(line, "expected a quoted string"));
    }
    match Variant::parse_prefix(s) {
        Ok((Variant::Str(v), rest)) => Ok((v, rest)),
        Ok(_) => Err(err(line, "expected a quoted string")),
        Err(e) => Err(err(line, e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbe_math::Vec2;

    fn sample() -> SceneTree {
        let mut t = SceneTree::new("World", "Node2D").unwrap();
        let root = t.root();
        let player = t.add_child(root, "Player", "Node2D").unwrap();
        t.set_property(player, "position", Variant::Vec2(Vec2::new(10.0, 20.0)))
            .unwrap();
        let sprite = t.add_child(player, "Sprite", "Sprite2D").unwrap();
        t.set_property(sprite, "texture", Variant::from("res://ñ \"x\".png"))
            .unwrap();
        t.add_child(root, "Enemy", "Node2D").unwrap();
        t
    }

    fn equal(a: &SceneTree, b: &SceneTree) -> bool {
        let (sa, sb) = (a.subtree(a.root()), b.subtree(b.root()));
        sa.len() == sb.len()
            && sa
                .iter()
                .zip(&sb)
                .all(|(x, y)| match (a.node(*x), b.node(*y)) {
                    (Some(p), Some(q)) => {
                        p.name() == q.name()
                            && p.kind() == q.kind()
                            && p.properties() == q.properties()
                            && a.path_of(*x) == b.path_of(*y)
                    }
                    _ => false,
                })
    }

    #[test]
    fn round_trips() {
        let t = sample();
        let text = t.to_text();
        let back = SceneTree::from_text(&text).unwrap();
        assert!(equal(&t, &back), "{text}");
        assert_eq!(back.to_text(), text);
    }

    #[test]
    fn reads_hand_written_scenes_with_comments() {
        let text = "# a scene\norbe_scene 1\n\nnode \"A\" \"Node\"\n\n  # child\nnode \"B\" \"Node\" parent \".\"\nx = 1\n";
        let t = SceneTree::from_text(text).unwrap();
        let b = t.find(t.root(), "B").unwrap();
        assert_eq!(t.property(b, "x"), Some(&Variant::Int(1)));
    }

    #[test]
    fn rejects_bad_scenes_with_a_line_number() {
        let cases: &[(&str, usize)] = &[
            ("", 0),
            ("# only a comment\n", 0),
            ("orbe_scene 1\n", 0),
            ("orbe_scene 2\nnode \"A\" \"Node\"\n", 1),
            ("orbe_scene x\n", 1),
            ("scene 1\n", 1),
            ("orbe_scene 1\nx = 1\n", 2),
            ("orbe_scene 1\nnode \"A\" \"Node\" parent \".\"\n", 2),
            ("orbe_scene 1\nnode \"A\" \"Node\"\nnode \"B\" \"Node\"\n", 3),
            ("orbe_scene 1\nnode \"A\" \"Node\"\nnode \"B\" \"Node\" parent \"Nope\"\n", 3),
            ("orbe_scene 1\nnode \"A\" \"Node\"\nnode \"B\" \"Node\" parent \".\"\nnode \"B\" \"Node\" parent \".\"\n", 4),
            ("orbe_scene 1\nnode \"A\" \"Bad Kind\"\n", 2),
            ("orbe_scene 1\nnode \"a/b\" \"Node\"\n", 2),
            ("orbe_scene 1\nnode A Node\n", 2),
            ("orbe_scene 1\nnode \"A\" \"Node\"\nbad key = 1\n", 3),
            ("orbe_scene 1\nnode \"A\" \"Node\"\nx = [1,\n", 3),
            ("orbe_scene 1\nnode \"A\" \"Node\"\nwhat\n", 3),
            ("orbe_scene 1\nnode \"A\" \"Node\" extra\n", 2),
            ("orbe_scene 1\nnode \"A\" \"Node\"\nnode \"B\" \"Node\" parent \".\" tail\n", 3),
        ];
        for (text, line) in cases {
            let e = SceneTree::from_text(text).unwrap_err();
            assert_eq!(e.line, *line, "{text:?}: {e}");
        }
    }

    #[test]
    fn hostile_inputs_do_not_panic() {
        let deep_value = format!(
            "orbe_scene 1\nnode \"A\" \"Node\"\nx = {}1{}\n",
            "[".repeat(5000),
            "]".repeat(5000)
        );
        assert!(SceneTree::from_text(&deep_value).is_err());
        let mut chain = String::from("orbe_scene 1\nnode \"n\" \"Node\"\n");
        let mut path = String::from(".");
        for i in 0..400 {
            chain.push_str(&format!("node \"c{i}\" \"Node\" parent \"{path}\"\n"));
            path = if path == "." {
                format!("c{i}")
            } else {
                format!("{path}/c{i}")
            };
        }
        assert!(SceneTree::from_text(&chain).is_err());
        for junk in [
            "\0\0\0",
            "node",
            "node \"",
            "orbe_scene",
            "=",
            "\u{feff}orbe_scene 1",
            "orbe_scene 1\nnode \"\\u{d800}\" \"N\"",
        ] {
            assert!(SceneTree::from_text(junk).is_err(), "{junk:?}");
        }
    }

    #[test]
    fn many_siblings_round_trip() {
        let mut t = SceneTree::new("R", "Node").unwrap();
        for i in 0..2000 {
            let c = t.add_child(t.root(), &format!("c{i}"), "Node").unwrap();
            t.set_property(c, "i", Variant::Int(i)).unwrap();
        }
        let back = SceneTree::from_text(&t.to_text()).unwrap();
        assert!(equal(&t, &back));
    }
}
