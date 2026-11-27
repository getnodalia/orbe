# ADR 0001: Orbe's engine moves to Rust, with its own engines

Status: accepted (2026-11-27, decided by the project lead). Phases R0 and R1 landed with this record; R2 (scene tree, `.orbe` format, importer for text `.tscn`/`.tres`) followed.

## Context

Orbe 0.3.0 is a C++ editor and runtime of about 3.1 million lines (editor 1.6 M, scene 0.33 M,
servers 0.32 M, modules 0.32 M, core 0.19 M) built with SCons, derived from an upstream open-source
engine. The project's rule is that Nodalia's apps are Nodalia's own, written in Rust on Nodalia's
own libraries, like Torno, Forja and Vortia. Orbe is the one app that is neither.

## Decision

Build a new engine in Rust inside this repository, as a Cargo workspace next to the C++ tree
(`crates/`, `xtask/`). The C++ editor keeps being released until the Rust editor reaches parity, so
users never lose a working Orbe; then the C++ is retired. Nothing is claimed as parity before its
tests pass, and each phase says honestly how mature it is.

Rules for the Rust engine:

- No `unwrap`, `expect`, `panic!` or `unsafe` in non-test code (workspace lints); every input is
  hostile and returns a `Result` or `Option`.
- Layers are checked by `cargo xtask layers`: a crate depends only on lower layers, and each crate
  has a row in `LAYERS` (`xtask/src/main.rs`).
- Only Nodalia's own crates or permissive crates.io dependencies (MIT, Apache-2.0, BSD, ISC, Zlib,
  Unicode, CC0). No code is copied from the C++ tree: behaviour comes from public documentation and
  observation, and from this repository's own tests.
- The editor UI is egui with `nodalia-theme`; NDL's rules (window controls, hamburger menu, no
  decorative gradients) apply unchanged.
- Everything is a command, and programmatic calls never open dialogs.
- Projects made for the upstream engine keep opening: the importer reads `project.godot`
  (rebrand: keep), `.tscn` and `.tres`; Orbe's own formats are the destination.

## Phases

| Phase | Delivers | Replaces |
|---|---|---|
| R0 | Workspace, `xtask` (ci, layers), CI, this ADR | n/a |
| R1 | `orbe-math` (vectors, rectangles, transforms, quaternions, matrices, colour), `orbe-core` (generational handles, resource paths, variants) | core math and variant types |
| R2 | `orbe-scene`: node tree, scenes, Orbe's own format, importer for upstream scenes and resources | scene, resource loading |
| R3 | `orbe-render` on wgpu: 2D first, then 3D PBR | rendering server and drivers |
| R4 | `orbe-physics` for 2D and 3D | physics servers and modules |
| R5 | `orbe-audio`, `orbe-input`, `orbe-anim` | audio server, input, animation |
| R6 | `orbe-script`: Orbe's own scripting language, plus a compatibility bridge for existing scripts | script modules |
| R7 | `orbe-editor` in egui and `nodalia-theme`: scene tree, inspector, 2D and 3D viewports | editor |
| R8 | Exporters for Linux, macOS, Windows and web | platform and export code |
| R9 | Parity checks, then removing the C++ tree | everything left |

## Consequences

- Two build systems live side by side for a long time (SCons for the C++ editor, Cargo for the
  engine). CI runs both; the Rust gate is `cargo xtask ci`.
- There is no upstream sync any more: Orbe's C++ tree is maintained here, and it is retired with the
  Rust editor.
- Licence notices of the upstream authors stay in `LICENSE.txt`, `COPYRIGHT.txt`, `AUTHORS.md`,
  `NOTICE` and the About window for as long as any of their code or compatibility surface remains.
