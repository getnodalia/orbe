// SPDX-License-Identifier: MIT
//! Orbe's Rust tooling: `cargo xtask ci` is the gate CI runs, `cargo xtask layers` checks that
//! crates only depend on lower layers.
#![allow(missing_docs)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// The layer of every workspace crate. A crate may depend only on crates in lower layers.
/// A new crate needs a row here.
const LAYERS: &[(&str, u8)] = &[
    ("orbe-math", 0),
    ("orbe-core", 1),
    ("orbe-scene", 2),
    ("orbe-import", 3),
];

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "ci" => ci(),
        "layers" => layers(&root()),
        _ => Err("usage: cargo xtask <ci|layers>".to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

fn run(args: &[&str]) -> Result<(), String> {
    println!("$ cargo {}", args.join(" "));
    let status = Command::new("cargo")
        .args(args)
        .current_dir(root())
        .status()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`cargo {}` failed", args.join(" ")))
    }
}

fn ci() -> Result<(), String> {
    run(&["fmt", "--all", "--", "--check"])?;
    run(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ])?;
    run(&["test", "--workspace"])?;
    layers(&root())
}

/// Crate name and the `orbe-*` crates it depends on, read from a manifest without a TOML parser.
fn read_manifest(text: &str) -> (Option<String>, Vec<String>) {
    let mut name = None;
    let mut deps = Vec::new();
    let mut section = String::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(s) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = s.to_string();
            continue;
        }
        let Some((key, _)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if section == "package" && key == "name" {
            name = line.split('"').nth(1).map(str::to_string);
        } else if section.ends_with("dependencies") && key.starts_with("orbe-") {
            deps.push(key.to_string());
        }
    }
    (name, deps)
}

fn layers(root: &Path) -> Result<(), String> {
    let table: BTreeMap<&str, u8> = LAYERS.iter().copied().collect();
    let dir = root.join("crates");
    let entries = fs::read_dir(&dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut problems = Vec::new();
    for entry in entries.flatten() {
        let manifest = entry.path().join("Cargo.toml");
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        let (name, deps) = read_manifest(&text);
        let Some(name) = name else {
            problems.push(format!("{} has no package name", manifest.display()));
            continue;
        };
        let Some(&layer) = table.get(name.as_str()) else {
            problems.push(format!("{name} has no row in LAYERS (xtask/src/main.rs)"));
            continue;
        };
        for dep in deps {
            match table.get(dep.as_str()) {
                Some(&l) if l < layer => {}
                Some(&l) => problems.push(format!("{name} (L{layer}) depends on {dep} (L{l})")),
                None => problems.push(format!("{name} depends on {dep}, which has no layer")),
            }
        }
    }
    for (name, _) in LAYERS {
        if !dir
            .join(name.trim_start_matches("orbe-"))
            .join("Cargo.toml")
            .exists()
        {
            problems.push(format!("LAYERS lists {name} but crates/ has no such crate"));
        }
    }
    if problems.is_empty() {
        println!("layers: ok");
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_names_and_orbe_deps() {
        let t = "[package]\nname = \"orbe-x\"\n[dependencies]\norbe-math = { path = \"../math\" }\nserde = \"1\"\n[dev-dependencies]\norbe-core = { path = \"../core\" }\n";
        let (name, deps) = read_manifest(t);
        assert_eq!(name.as_deref(), Some("orbe-x"));
        assert_eq!(deps, ["orbe-math", "orbe-core"]);
    }

    #[test]
    fn repository_layers_are_clean() {
        assert_eq!(layers(&root()), Ok(()));
    }
}
