// SPDX-License-Identifier: MIT
//! Orbe's scene tree: a tree of named, typed nodes that carry properties, and the text format
//! (`.orbe`) scenes are saved in.
//!
//! A [`SceneTree`] never panics on hostile input: every operation that can fail returns a
//! [`SceneError`], and the tree is bounded in size ([`MAX_NODES`]) and depth ([`MAX_TREE_DEPTH`]).

mod error;
mod format;
mod tree;

pub use error::{FormatError, SceneError};
pub use format::{FORMAT_VERSION, MAX_FILE_BYTES};
pub use tree::{
    valid_kind, valid_name, valid_property, Node, SceneTree, MAX_NODES, MAX_TREE_DEPTH,
};
