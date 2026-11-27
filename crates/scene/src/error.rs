// SPDX-License-Identifier: MIT
use core::fmt;

/// Why a tree operation was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneError {
    /// The node name is empty, too long or contains a reserved character.
    InvalidName,
    /// The node type name is empty, too long or not made of letters, digits and `_`.
    InvalidKind,
    /// The property name is empty, too long or contains a reserved character.
    InvalidProperty,
    /// A sibling already has that name.
    DuplicateName,
    /// The handle does not point to a live node.
    UnknownNode,
    /// The operation would put a node below itself.
    WouldCycle,
    /// The tree would grow deeper than [`crate::MAX_TREE_DEPTH`].
    TooDeep,
    /// The tree would hold more than [`crate::MAX_NODES`] nodes.
    TooManyNodes,
    /// The root cannot be removed or moved.
    RootIsFixed,
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SceneError::InvalidName => "invalid node name",
            SceneError::InvalidKind => "invalid node type",
            SceneError::InvalidProperty => "invalid property name",
            SceneError::DuplicateName => "a sibling already has that name",
            SceneError::UnknownNode => "no such node",
            SceneError::WouldCycle => "a node cannot be moved below itself",
            SceneError::TooDeep => "the scene is nested too deeply",
            SceneError::TooManyNodes => "the scene has too many nodes",
            SceneError::RootIsFixed => "the root node cannot be removed or moved",
        })
    }
}

impl std::error::Error for SceneError {}

/// Why a scene file could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatError {
    /// 1-based line number, or 0 for a problem with the file as a whole.
    pub line: usize,
    /// What was wrong.
    pub message: String,
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            f.write_str(&self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

impl std::error::Error for FormatError {}
