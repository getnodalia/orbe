// SPDX-License-Identifier: MIT
//! Turns the sections of a `.tscn` / `.tres` file into an Orbe scene tree or resource.

use crate::file::{split_sections, Section};
use crate::value::{parse_value, Env};
use orbe_core::{ResPath, Variant};
use orbe_scene::{valid_property, SceneError, SceneTree};
use std::collections::BTreeMap;
use std::fmt;

/// Largest file the importers read, in bytes.
pub const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;

/// Why a file could not be imported at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportError {
    /// 1-based line number, or 0 for a problem with the file as a whole.
    pub line: usize,
    /// What was wrong.
    pub message: String,
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            f.write_str(&self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

impl std::error::Error for ImportError {}

/// Something the importer skipped or could only keep as a marker value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    /// 1-based line number of the section or property.
    pub line: usize,
    /// What was skipped and why.
    pub message: String,
}

/// A resource that lives in another file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtResource {
    /// The resource type, such as `Texture2D`.
    pub kind: String,
    /// Its normalised `res://` path.
    pub path: String,
}

/// A resource stored inside the file that uses it.
#[derive(Clone, Debug, PartialEq)]
pub struct SubResource {
    /// The resource type, such as `RectangleShape2D`.
    pub kind: String,
    /// Its properties.
    pub properties: BTreeMap<String, Variant>,
}

/// A signal connection between two nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connection {
    /// Signal name.
    pub signal: String,
    /// Path of the emitting node.
    pub from: String,
    /// Path of the receiving node.
    pub to: String,
    /// Method called on the receiver.
    pub method: String,
    /// Connection flags.
    pub flags: i64,
}

/// An imported scene.
#[derive(Debug)]
pub struct SceneImport {
    /// The nodes.
    pub tree: SceneTree,
    /// Resources from other files by their id in the source file.
    pub ext_resources: BTreeMap<String, ExtResource>,
    /// Resources defined inside the file by their id.
    pub sub_resources: BTreeMap<String, SubResource>,
    /// Signal connections.
    pub connections: Vec<Connection>,
    /// Everything that was skipped or kept as a marker.
    pub warnings: Vec<Warning>,
}

/// An imported standalone resource.
#[derive(Debug)]
pub struct ResourceImport {
    /// The resource type.
    pub kind: String,
    /// Resources from other files by their id in the source file.
    pub ext_resources: BTreeMap<String, ExtResource>,
    /// Resources defined inside the file by their id.
    pub sub_resources: BTreeMap<String, SubResource>,
    /// The resource's own properties.
    pub properties: BTreeMap<String, Variant>,
    /// Everything that was skipped or kept as a marker.
    pub warnings: Vec<Warning>,
}

fn fail(line: usize, message: &str) -> ImportError {
    ImportError {
        line,
        message: message.to_string(),
    }
}

fn check_size(text: &str) -> Result<(), ImportError> {
    if text.len() > MAX_FILE_BYTES {
        return Err(fail(0, "the file is too large"));
    }
    Ok(())
}

/// A header attribute holding a string.
fn attr_str(sec: &Section, key: &str) -> Option<String> {
    let raw = sec.attr(key)?;
    let ext = BTreeMap::new();
    let mut env = Env {
        ext: &ext,
        warnings: Vec::new(),
    };
    match parse_value(raw, &mut env) {
        Ok(Variant::Str(s)) => Some(s),
        _ => None,
    }
}

fn attr_int(sec: &Section, key: &str) -> Option<i64> {
    let raw = sec.attr(key)?;
    let ext = BTreeMap::new();
    let mut env = Env {
        ext: &ext,
        warnings: Vec::new(),
    };
    match parse_value(raw, &mut env) {
        Ok(Variant::Int(i)) => Some(i),
        _ => None,
    }
}

/// Reads one raw value; a value that cannot be read is kept as `{"$raw": text}` with a warning.
fn read_value(
    line: usize,
    raw: &str,
    ext: &BTreeMap<String, String>,
    warnings: &mut Vec<Warning>,
) -> Variant {
    let mut env = Env {
        ext,
        warnings: Vec::new(),
    };
    let result = parse_value(raw, &mut env);
    for message in env.warnings {
        warnings.push(Warning { line, message });
    }
    match result {
        Ok(v) => v,
        Err(message) => {
            warnings.push(Warning {
                line,
                message: format!("could not read the value ({message}); kept as text"),
            });
            let mut d = BTreeMap::new();
            d.insert("$raw".to_string(), Variant::Str(raw.to_string()));
            Variant::Dict(d)
        }
    }
}

/// Reads every property line of a section.
fn read_props(
    sec: &Section,
    ext: &BTreeMap<String, String>,
    warnings: &mut Vec<Warning>,
) -> Vec<(usize, String, Variant)> {
    sec.props
        .iter()
        .map(|(line, key, raw)| (*line, key.clone(), read_value(*line, raw, ext, warnings)))
        .collect()
}

struct Defs {
    ext_paths: BTreeMap<String, String>,
    ext_resources: BTreeMap<String, ExtResource>,
    sub_resources: BTreeMap<String, SubResource>,
}

/// Reads the `ext_resource` and `sub_resource` sections.
fn read_defs(sections: &[Section], warnings: &mut Vec<Warning>) -> Defs {
    let mut defs = Defs {
        ext_paths: BTreeMap::new(),
        ext_resources: BTreeMap::new(),
        sub_resources: BTreeMap::new(),
    };
    for sec in sections.iter().filter(|s| s.tag == "ext_resource") {
        let (Some(id), Some(raw_path)) = (attr_str(sec, "id"), attr_str(sec, "path")) else {
            warnings.push(Warning {
                line: sec.line,
                message: "external resource without an id and a path".into(),
            });
            continue;
        };
        let path = match ResPath::parse(&raw_path) {
            Ok(p) => p.to_string(),
            Err(e) => {
                warnings.push(Warning {
                    line: sec.line,
                    message: format!(
                        "external resource path `{raw_path}` is not usable ({e}); ignored"
                    ),
                });
                continue;
            }
        };
        let kind = attr_str(sec, "type").unwrap_or_default();
        if defs.ext_paths.insert(id.clone(), path.clone()).is_some() {
            warnings.push(Warning {
                line: sec.line,
                message: format!("external resource id `{id}` is used twice"),
            });
        }
        defs.ext_resources.insert(id, ExtResource { kind, path });
    }
    for sec in sections.iter().filter(|s| s.tag == "sub_resource") {
        let (Some(id), Some(kind)) = (attr_str(sec, "id"), attr_str(sec, "type")) else {
            warnings.push(Warning {
                line: sec.line,
                message: "internal resource without an id and a type".into(),
            });
            continue;
        };
        let properties = read_props(sec, &defs.ext_paths, warnings)
            .into_iter()
            .map(|(_, k, v)| (k, v))
            .collect();
        if defs.sub_resources.contains_key(&id) {
            warnings.push(Warning {
                line: sec.line,
                message: format!("internal resource id `{id}` is used twice"),
            });
            continue;
        }
        defs.sub_resources
            .insert(id, SubResource { kind, properties });
    }
    defs
}

fn scene_error(line: usize, e: SceneError) -> ImportError {
    ImportError {
        line,
        message: e.to_string(),
    }
}

/// Imports a `.tscn` text scene.
pub fn import_scene(text: &str) -> Result<SceneImport, ImportError> {
    check_size(text)?;
    let sections = split_sections(text)?;
    match sections.first() {
        Some(s) if s.tag == "gd_scene" => {}
        Some(s) => return Err(fail(s.line, "a scene file starts with a `gd_scene` header")),
        None => return Err(fail(0, "the file is empty")),
    }
    let mut warnings = Vec::new();
    let defs = read_defs(&sections, &mut warnings);
    let mut tree: Option<SceneTree> = None;
    let mut current = None;
    let mut connections = Vec::new();

    for sec in &sections {
        match sec.tag.as_str() {
            "gd_scene" | "ext_resource" | "sub_resource" | "editable" => {}
            "node" => {
                let Some(name) = attr_str(sec, "name") else {
                    return_or_warn(&mut warnings, sec.line, "node without a name; skipped");
                    current = None;
                    continue;
                };
                let instance = sec
                    .attr("instance")
                    .map(|raw| read_value(sec.line, raw, &defs.ext_paths, &mut warnings));
                let kind = match (attr_str(sec, "type"), &instance) {
                    (Some(t), _) => t,
                    (None, Some(_)) => "SceneInstance".to_string(),
                    (None, None) => {
                        return_or_warn(
                            &mut warnings,
                            sec.line,
                            "node without a type overrides a node of an instanced scene; kept as `Node`",
                        );
                        "Node".to_string()
                    }
                };
                let handle = match tree.as_mut() {
                    None => match SceneTree::new(&name, &kind) {
                        Ok(t) => {
                            let root = t.root();
                            tree = Some(t);
                            Some(root)
                        }
                        Err(e) => return Err(scene_error(sec.line, e)),
                    },
                    Some(t) => add_node(t, sec, &name, &kind, &mut warnings)?,
                };
                current = handle;
                let (Some(h), Some(t)) = (handle, tree.as_mut()) else {
                    continue;
                };
                if let Some(v) = instance {
                    set(t, h, "meta/scene", v, sec.line, &mut warnings);
                }
                if let Some(raw) = sec.attr("groups") {
                    let v = read_value(sec.line, raw, &defs.ext_paths, &mut warnings);
                    set(t, h, "meta/groups", v, sec.line, &mut warnings);
                }
                for (line, key, value) in read_props(sec, &defs.ext_paths, &mut warnings) {
                    set(t, h, &key, value, line, &mut warnings);
                }
            }
            "connection" => {
                let get = |k: &str| attr_str(sec, k);
                match (get("signal"), get("from"), get("to"), get("method")) {
                    (Some(signal), Some(from), Some(to), Some(method)) => {
                        connections.push(Connection {
                            signal,
                            from,
                            to,
                            method,
                            flags: attr_int(sec, "flags").unwrap_or(0),
                        })
                    }
                    _ => return_or_warn(
                        &mut warnings,
                        sec.line,
                        "connection without signal, from, to and method; skipped",
                    ),
                }
            }
            other => {
                return_or_warn(
                    &mut warnings,
                    sec.line,
                    &format!("unsupported section `{other}`; skipped"),
                );
            }
        }
    }
    let _ = current;
    let tree = tree.ok_or_else(|| fail(0, "the scene has no nodes"))?;
    Ok(SceneImport {
        tree,
        ext_resources: defs.ext_resources,
        sub_resources: defs.sub_resources,
        connections,
        warnings,
    })
}

fn return_or_warn(warnings: &mut Vec<Warning>, line: usize, message: &str) {
    warnings.push(Warning {
        line,
        message: message.to_string(),
    });
}

/// Adds a non-root node below the parent its `parent` attribute names.
fn add_node(
    tree: &mut SceneTree,
    sec: &Section,
    name: &str,
    kind: &str,
    warnings: &mut Vec<Warning>,
) -> Result<Option<orbe_core::Handle<orbe_scene::Node>>, ImportError> {
    let Some(parent_path) = attr_str(sec, "parent") else {
        return_or_warn(
            warnings,
            sec.line,
            &format!("second root node `{name}`; skipped"),
        );
        return Ok(None);
    };
    let Some(parent) = tree.find(tree.root(), &parent_path) else {
        return_or_warn(
            warnings,
            sec.line,
            &format!("node `{name}` has no parent `{parent_path}`; skipped"),
        );
        return Ok(None);
    };
    match tree.add_child(parent, name, kind) {
        Ok(h) => Ok(Some(h)),
        Err(e @ (SceneError::TooManyNodes | SceneError::TooDeep)) => Err(scene_error(sec.line, e)),
        Err(e) => {
            return_or_warn(warnings, sec.line, &format!("node `{name}` skipped: {e}"));
            Ok(None)
        }
    }
}

fn set(
    tree: &mut SceneTree,
    h: orbe_core::Handle<orbe_scene::Node>,
    key: &str,
    value: Variant,
    line: usize,
    warnings: &mut Vec<Warning>,
) {
    if !valid_property(key) {
        return_or_warn(
            warnings,
            line,
            &format!("property `{key}` has an unusable name; skipped"),
        );
    } else if let Err(e) = tree.set_property(h, key, value) {
        return_or_warn(warnings, line, &format!("property `{key}` skipped: {e}"));
    }
}

/// Imports a `.tres` text resource.
pub fn import_resource(text: &str) -> Result<ResourceImport, ImportError> {
    check_size(text)?;
    let sections = split_sections(text)?;
    let kind = match sections.first() {
        Some(s) if s.tag == "gd_resource" => attr_str(s, "type").unwrap_or_default(),
        Some(s) => {
            return Err(fail(
                s.line,
                "a resource file starts with a `gd_resource` header",
            ))
        }
        None => return Err(fail(0, "the file is empty")),
    };
    let mut warnings = Vec::new();
    let defs = read_defs(&sections, &mut warnings);
    let mut properties = BTreeMap::new();
    let mut seen_resource = false;
    for sec in &sections {
        match sec.tag.as_str() {
            "gd_resource" | "ext_resource" | "sub_resource" => {}
            "resource" => {
                if seen_resource {
                    return_or_warn(
                        &mut warnings,
                        sec.line,
                        "second `resource` section; skipped",
                    );
                    continue;
                }
                seen_resource = true;
                for (_, key, value) in read_props(sec, &defs.ext_paths, &mut warnings) {
                    properties.insert(key, value);
                }
            }
            other => {
                return_or_warn(
                    &mut warnings,
                    sec.line,
                    &format!("unsupported section `{other}`; skipped"),
                );
            }
        }
    }
    Ok(ResourceImport {
        kind,
        ext_resources: defs.ext_resources,
        sub_resources: defs.sub_resources,
        properties,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbe_math::Vec2;

    const SCENE: &str = r#"[gd_scene load_steps=3 format=3 uid="uid://abc"]

[ext_resource type="Texture2D" path="res://art/../icon.png" id="1_tex"]
[ext_resource type="PackedScene" path="res://enemy.tscn" id="2_enemy"]

[sub_resource type="RectangleShape2D" id="Rect_1"]
size = Vector2(32, 16)

[node name="Main" type="Node2D"]
position = Vector2(10, 20)
metadata/note = "hello"

[node name="Sprite" type="Sprite2D" parent="." groups=["actors", "visible"]]
texture = ExtResource("1_tex")
scale = Vector2(2, 2)

[node name="Body" type="CollisionShape2D" parent="Sprite"]
shape = SubResource("Rect_1")
transform = Transform2D(1, 0, 0, 1, 5, 5)

[node name="Enemy" parent="." instance=ExtResource("2_enemy")]
position = Vector2(100, 0)

[connection signal="ready" from="Sprite" to="." method="_on_ready" flags=3]
[editable path="Enemy"]
"#;

    #[test]
    fn imports_a_scene() {
        let s = import_scene(SCENE).unwrap();
        assert!(s.warnings.is_empty(), "{:?}", s.warnings);
        let t = &s.tree;
        assert_eq!(t.len(), 4);
        let root = t.root();
        assert_eq!(t.node(root).unwrap().kind(), "Node2D");
        assert_eq!(
            t.property(root, "position"),
            Some(&Variant::from(Vec2::new(10.0, 20.0)))
        );
        let sprite = t.find(root, "Sprite").unwrap();
        assert_eq!(t.node(sprite).unwrap().kind(), "Sprite2D");
        let tex = t.property(sprite, "texture").unwrap();
        assert_eq!(tex.to_text(), r#"{"$ext": "res://icon.png"}"#);
        assert!(t.property(sprite, "meta/groups").is_some());
        let body = t.find(root, "Sprite/Body").unwrap();
        assert_eq!(
            t.property(body, "shape").unwrap().to_text(),
            r#"{"$sub": "Rect_1"}"#
        );
        let enemy = t.find(root, "Enemy").unwrap();
        assert_eq!(t.node(enemy).unwrap().kind(), "SceneInstance");
        assert!(t.property(enemy, "meta/scene").is_some());
        assert_eq!(s.sub_resources["Rect_1"].kind, "RectangleShape2D");
        assert_eq!(s.ext_resources["1_tex"].path, "res://icon.png");
        assert_eq!(
            s.connections,
            vec![Connection {
                signal: "ready".into(),
                from: "Sprite".into(),
                to: ".".into(),
                method: "_on_ready".into(),
                flags: 3
            }]
        );
    }

    #[test]
    fn imports_a_resource() {
        let r = import_resource(
            "[gd_resource type=\"Gradient\" format=3]\n\n[ext_resource type=\"Script\" path=\"res://g.gd\" id=\"1\"]\n\n[resource]\noffsets = PackedFloat32Array(0, 1)\nscript = ExtResource(\"1\")\nname = \"warm\"\n",
        )
        .unwrap();
        assert_eq!(r.kind, "Gradient");
        assert_eq!(r.properties["name"], Variant::from("warm"));
        assert_eq!(
            r.properties["script"].to_text(),
            r#"{"$ext": "res://g.gd"}"#
        );
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    }

    #[test]
    fn problems_become_warnings() {
        let s = import_scene(
            "[gd_scene format=3]\n[node name=\"R\" type=\"Node\"]\nx = Weird(1)\ny = [1,\n2\n[node name=\"A\" type=\"Node\" parent=\"missing\"]\n[node name=\"B\" type=\"Node\" parent=\".\"]\n[node name=\"B\" type=\"Node\" parent=\".\"]\n[node name=\"bad/name\" type=\"Node\" parent=\".\"]\n[node name=\"Two\" type=\"Node\"]\n[wat]\n",
        );
        // an unbalanced value is a hard error
        assert!(s.is_err());
        let s = import_scene(
            "[gd_scene format=3]\n[node name=\"R\" type=\"Node\"]\nx = Weird(1)\ny = nope\n[node name=\"A\" type=\"Node\" parent=\"missing\"]\nz = 1\n[node name=\"B\" type=\"Node\" parent=\".\"]\n[node name=\"B\" type=\"Node\" parent=\".\"]\n[node name=\"bad/name\" type=\"Node\" parent=\".\"]\n[node name=\"Two\" type=\"Node\"]\n[wat]\n[node name=\"O\" parent=\".\"]\n",
        )
        .unwrap();
        assert_eq!(s.tree.len(), 3); // R, B, O
        let msgs: Vec<&str> = s.warnings.iter().map(|w| w.message.as_str()).collect();
        for needle in [
            "Weird",
            "could not read",
            "no parent",
            "skipped: a sibling",
            "invalid node name",
            "second root",
            "unsupported section",
            "instanced scene",
        ] {
            assert!(
                msgs.iter().any(|m| m.contains(needle)),
                "missing warning {needle}: {msgs:?}"
            );
        }
        let root = s.tree.root();
        assert!(s.tree.property(root, "x").is_some());
        assert_eq!(
            s.tree.property(root, "y").unwrap().to_text(),
            r#"{"$raw": "nope"}"#
        );
    }

    #[test]
    fn rejects_other_files() {
        assert!(import_scene("").is_err());
        assert!(import_scene("[gd_resource type=\"X\"]\n").is_err());
        assert!(import_resource("[gd_scene]\n").is_err());
        assert!(import_scene("[gd_scene]\n").is_err());
        assert!(import_scene("[gd_scene]\n[node name=\"bad/\" type=\"Node\"]\n").is_err());
    }

    #[test]
    fn escaping_paths_are_refused() {
        let s = import_scene(
            "[gd_scene]\n[ext_resource type=\"Texture2D\" path=\"res://../../etc/passwd\" id=\"1\"]\n[node name=\"R\" type=\"Node\"]\ntex = ExtResource(\"1\")\n",
        )
        .unwrap();
        assert!(s.ext_resources.is_empty());
        assert!(s.warnings.iter().any(|w| w.message.contains("not usable")));
    }

    #[test]
    fn truncated_files_never_panic() {
        for text in [
            SCENE,
            "[gd_resource type=\"X\"]\n[resource]\na = {\"k\": [1, Vector2(1, 2)]}\nb = \"é\"\n",
        ] {
            for (i, _) in text.char_indices() {
                let head = &text[..i];
                let _ = import_scene(head);
                let _ = import_resource(head);
            }
        }
    }

    #[test]
    fn hostile_depth_and_size_are_bounded() {
        let deep = format!(
            "[gd_resource type=\"X\"]\n[resource]\na = {}1{}\n",
            "[".repeat(5000),
            "]".repeat(5000)
        );
        let r = import_resource(&deep).unwrap();
        assert!(!r.warnings.is_empty());
        let mut nodes = String::from("[gd_scene]\n[node name=\"R\" type=\"Node\"]\n");
        let mut parent = ".".to_string();
        for i in 0..300 {
            nodes.push_str(&format!(
                "[node name=\"n{i}\" type=\"Node\" parent=\"{parent}\"]\n"
            ));
            parent = if parent == "." {
                format!("n{i}")
            } else {
                format!("{parent}/n{i}")
            };
        }
        assert!(import_scene(&nodes).is_err());
    }
}
