// SPDX-License-Identifier: MIT
//! Importers for projects made for the upstream engine, so existing scenes and resources open in
//! Orbe: `.tscn` text scenes ([`import_scene`]) and `.tres` text resources ([`import_resource`]).
//!
//! The text format is read from its public documentation. Importing never panics: malformed files
//! give an [`ImportError`], and anything the importer does not understand is kept as a
//! [`Warning`] plus a marker value instead of being dropped silently.
//!
//! Values that have no [`orbe_core::Variant`] of their own are stored as dictionaries with a `$`
//! key:
//!
//! | Source value | Imported as |
//! |---|---|
//! | resource from another file | `{"$ext": "res://path"}` |
//! | resource inside the same file | `{"$sub": "<id>"}` (see [`SceneImport::sub_resources`]) |
//! | node path | `{"$nodepath": "../Node"}` |
//! | rectangles, transforms, quaternions, planes, boxes, bases | `{"$rect2": [x, y, w, h]}`, `{"$transform3d": [..12..]}`, ... |
//! | anything else unsupported | `{"$raw": "<source text>"}` with a warning |

mod file;
mod import;
mod value;

pub use import::{
    import_resource, import_scene, Connection, ExtResource, ImportError, ResourceImport,
    SceneImport, SubResource, Warning, MAX_FILE_BYTES,
};
