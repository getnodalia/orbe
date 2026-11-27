// SPDX-License-Identifier: MIT
use crate::SceneError;
use orbe_core::{Arena, Handle, Variant};
use std::collections::BTreeMap;

/// Most nodes a tree may hold.
pub const MAX_NODES: usize = 1_000_000;
/// Deepest nesting of nodes (the root is depth 0).
pub const MAX_TREE_DEPTH: usize = 256;

const MAX_NAME_BYTES: usize = 256;

/// True when `name` can name a node: non-empty, at most 256 bytes, and free of `/ : . @ % " \` and
/// control characters (so a name can never be mistaken for a path step).
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && !name
            .chars()
            .any(|c| c.is_control() || "/:.@%\"\\".contains(c))
}

/// True when `kind` can name a node type: 1 to 128 letters, digits or `_`.
pub fn valid_kind(kind: &str) -> bool {
    !kind.is_empty()
        && kind.len() <= 128
        && kind.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// True when `key` can name a property: 1 to 128 letters, digits, `_` or `/` (for grouped names
/// such as `theme/color`).
pub fn valid_property(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 128
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '/')
}

/// One node of a scene.
#[derive(Clone, Debug)]
pub struct Node {
    name: String,
    kind: String,
    props: BTreeMap<String, Variant>,
    parent: Option<Handle<Node>>,
    children: Vec<Handle<Node>>,
}

impl Node {
    /// The node's name, unique among its siblings.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The node's type name (`Node2D`, `Sprite2D`, ...).
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The node's properties, sorted by name.
    pub fn properties(&self) -> &BTreeMap<String, Variant> {
        &self.props
    }

    /// The parent, or `None` for the root.
    pub fn parent(&self) -> Option<Handle<Node>> {
        self.parent
    }

    /// The children in order.
    pub fn children(&self) -> &[Handle<Node>] {
        &self.children
    }
}

/// A tree of nodes with a root. Handles stay valid until their node is removed.
#[derive(Clone, Debug)]
pub struct SceneTree {
    arena: Arena<Node>,
    root: Handle<Node>,
}

impl SceneTree {
    /// A tree holding only a root node.
    pub fn new(root_name: &str, root_kind: &str) -> Result<SceneTree, SceneError> {
        check_name(root_name)?;
        check_kind(root_kind)?;
        let mut arena = Arena::new();
        let root = arena
            .insert(Node {
                name: root_name.to_string(),
                kind: root_kind.to_string(),
                props: BTreeMap::new(),
                parent: None,
                children: Vec::new(),
            })
            .ok_or(SceneError::TooManyNodes)?;
        Ok(SceneTree { arena, root })
    }

    /// The root node.
    pub fn root(&self) -> Handle<Node> {
        self.root
    }

    /// Number of nodes, root included.
    pub fn len(&self) -> usize {
        self.arena.len()
    }

    /// Always false: a tree has a root.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The node behind `h`.
    pub fn node(&self, h: Handle<Node>) -> Option<&Node> {
        self.arena.get(h)
    }

    /// Depth of `h` (the root is 0).
    pub fn depth(&self, h: Handle<Node>) -> Option<usize> {
        let mut depth = 0;
        let mut cur = self.arena.get(h)?;
        while let Some(p) = cur.parent {
            depth += 1;
            cur = self.arena.get(p)?;
            if depth > MAX_TREE_DEPTH + 1 {
                return None;
            }
        }
        Some(depth)
    }

    /// Adds a child at the end of `parent`'s children.
    pub fn add_child(
        &mut self,
        parent: Handle<Node>,
        name: &str,
        kind: &str,
    ) -> Result<Handle<Node>, SceneError> {
        check_name(name)?;
        check_kind(kind)?;
        if self.arena.len() >= MAX_NODES {
            return Err(SceneError::TooManyNodes);
        }
        let depth = self.depth(parent).ok_or(SceneError::UnknownNode)?;
        if depth + 1 > MAX_TREE_DEPTH {
            return Err(SceneError::TooDeep);
        }
        if self.child_named(parent, name).is_some() {
            return Err(SceneError::DuplicateName);
        }
        let h = self
            .arena
            .insert(Node {
                name: name.to_string(),
                kind: kind.to_string(),
                props: BTreeMap::new(),
                parent: Some(parent),
                children: Vec::new(),
            })
            .ok_or(SceneError::TooManyNodes)?;
        match self.arena.get_mut(parent) {
            Some(p) => p.children.push(h),
            None => {
                self.arena.remove(h);
                return Err(SceneError::UnknownNode);
            }
        }
        Ok(h)
    }

    /// The child of `parent` with this name.
    pub fn child_named(&self, parent: Handle<Node>, name: &str) -> Option<Handle<Node>> {
        let p = self.arena.get(parent)?;
        p.children
            .iter()
            .copied()
            .find(|c| self.arena.get(*c).is_some_and(|n| n.name == name))
    }

    /// Renames a node.
    pub fn rename(&mut self, h: Handle<Node>, name: &str) -> Result<(), SceneError> {
        check_name(name)?;
        let parent = self.arena.get(h).ok_or(SceneError::UnknownNode)?.parent;
        if let Some(p) = parent {
            if self.child_named(p, name).is_some_and(|other| other != h) {
                return Err(SceneError::DuplicateName);
            }
        }
        if let Some(n) = self.arena.get_mut(h) {
            n.name = name.to_string();
        }
        Ok(())
    }

    /// All nodes of the subtree at `h`, `h` first, parents before children.
    pub fn subtree(&self, h: Handle<Node>) -> Vec<Handle<Node>> {
        let mut out = Vec::new();
        let mut stack = vec![h];
        while let Some(cur) = stack.pop() {
            let Some(n) = self.arena.get(cur) else {
                continue;
            };
            out.push(cur);
            stack.extend(n.children.iter().rev().copied());
        }
        out
    }

    /// Removes a node and everything below it.
    pub fn remove(&mut self, h: Handle<Node>) -> Result<(), SceneError> {
        if h == self.root {
            return Err(SceneError::RootIsFixed);
        }
        let parent = self.arena.get(h).ok_or(SceneError::UnknownNode)?.parent;
        for gone in self.subtree(h) {
            self.arena.remove(gone);
        }
        if let Some(p) = parent.and_then(|p| self.arena.get_mut(p)) {
            p.children.retain(|c| *c != h);
        }
        Ok(())
    }

    /// Moves `h` (with its subtree) to the end of `new_parent`'s children.
    pub fn reparent(
        &mut self,
        h: Handle<Node>,
        new_parent: Handle<Node>,
    ) -> Result<(), SceneError> {
        if h == self.root {
            return Err(SceneError::RootIsFixed);
        }
        let node = self.arena.get(h).ok_or(SceneError::UnknownNode)?;
        let old_parent = node.parent;
        let name = node.name.clone();
        self.arena.get(new_parent).ok_or(SceneError::UnknownNode)?;
        let mut cur = Some(new_parent);
        while let Some(c) = cur {
            if c == h {
                return Err(SceneError::WouldCycle);
            }
            cur = self.arena.get(c).and_then(|n| n.parent);
        }
        if old_parent != Some(new_parent) && self.child_named(new_parent, &name).is_some() {
            return Err(SceneError::DuplicateName);
        }
        let base = self.depth(new_parent).ok_or(SceneError::UnknownNode)? + 1;
        let below = self
            .subtree(h)
            .into_iter()
            .filter_map(|d| self.depth(d))
            .max()
            .unwrap_or(0);
        let own = self.depth(h).unwrap_or(0);
        if base + below.saturating_sub(own) > MAX_TREE_DEPTH {
            return Err(SceneError::TooDeep);
        }
        if let Some(p) = old_parent.and_then(|p| self.arena.get_mut(p)) {
            p.children.retain(|c| *c != h);
        }
        if let Some(n) = self.arena.get_mut(h) {
            n.parent = Some(new_parent);
        }
        if let Some(p) = self.arena.get_mut(new_parent) {
            p.children.push(h);
        }
        Ok(())
    }

    /// Sets a property.
    pub fn set_property(
        &mut self,
        h: Handle<Node>,
        key: &str,
        value: Variant,
    ) -> Result<(), SceneError> {
        if !valid_property(key) {
            return Err(SceneError::InvalidProperty);
        }
        let n = self.arena.get_mut(h).ok_or(SceneError::UnknownNode)?;
        n.props.insert(key.to_string(), value);
        Ok(())
    }

    /// Reads a property.
    pub fn property(&self, h: Handle<Node>, key: &str) -> Option<&Variant> {
        self.arena.get(h)?.props.get(key)
    }

    /// Removes a property, returning its value.
    pub fn remove_property(&mut self, h: Handle<Node>, key: &str) -> Option<Variant> {
        self.arena.get_mut(h)?.props.remove(key)
    }

    /// Finds a node by path relative to `from`: segments separated by `/`, where `.` is the node
    /// itself and `..` its parent. A leading `/` starts at the root.
    pub fn find(&self, from: Handle<Node>, path: &str) -> Option<Handle<Node>> {
        let (mut cur, rest) = match path.strip_prefix('/') {
            Some(r) => (self.root, r),
            None => (from, path),
        };
        self.arena.get(cur)?;
        for seg in rest.split('/') {
            match seg {
                "" | "." => {}
                ".." => cur = self.arena.get(cur)?.parent?,
                name => cur = self.child_named(cur, name)?,
            }
        }
        Some(cur)
    }

    /// The path of `h` from the root: `.` for the root itself, otherwise `Parent/Child`.
    pub fn path_of(&self, h: Handle<Node>) -> Option<String> {
        let mut names = Vec::new();
        let mut cur = self.arena.get(h)?;
        while let Some(p) = cur.parent {
            names.push(cur.name.as_str());
            cur = self.arena.get(p)?;
            if names.len() > MAX_TREE_DEPTH + 1 {
                return None;
            }
        }
        if names.is_empty() {
            return Some(".".to_string());
        }
        names.reverse();
        Some(names.join("/"))
    }
}

fn check_name(name: &str) -> Result<(), SceneError> {
    if valid_name(name) {
        Ok(())
    } else {
        Err(SceneError::InvalidName)
    }
}

fn check_kind(kind: &str) -> Result<(), SceneError> {
    if valid_kind(kind) {
        Ok(())
    } else {
        Err(SceneError::InvalidKind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (SceneTree, Handle<Node>, Handle<Node>, Handle<Node>) {
        let mut t = SceneTree::new("World", "Node2D").unwrap();
        let root = t.root();
        let player = t.add_child(root, "Player", "Node2D").unwrap();
        let sprite = t.add_child(player, "Sprite", "Sprite2D").unwrap();
        (t, root, player, sprite)
    }

    #[test]
    fn builds_and_finds_by_path() {
        let (t, root, player, sprite) = sample();
        assert_eq!(t.len(), 3);
        assert_eq!(t.find(root, "Player/Sprite"), Some(sprite));
        assert_eq!(t.find(sprite, "../.."), Some(root));
        assert_eq!(t.find(sprite, "/Player"), Some(player));
        assert_eq!(t.find(root, "Nope"), None);
        assert_eq!(t.find(root, ".."), None);
        assert_eq!(t.path_of(sprite).as_deref(), Some("Player/Sprite"));
        assert_eq!(t.path_of(root).as_deref(), Some("."));
        assert_eq!(t.depth(sprite), Some(2));
    }

    #[test]
    fn names_must_be_unique_and_valid() {
        let (mut t, root, player, _) = sample();
        assert_eq!(
            t.add_child(root, "Player", "Node"),
            Err(SceneError::DuplicateName)
        );
        for bad in [
            "",
            "a/b",
            "a.b",
            "..",
            "a:b",
            "a@b",
            "a\nb",
            "a\"b",
            &"x".repeat(257),
        ] {
            assert_eq!(
                t.add_child(root, bad, "Node"),
                Err(SceneError::InvalidName),
                "{bad:?}"
            );
        }
        assert_eq!(
            t.add_child(root, "Ok", "bad kind"),
            Err(SceneError::InvalidKind)
        );
        assert_eq!(t.add_child(root, "Ok", ""), Err(SceneError::InvalidKind));
        let other = t.add_child(root, "Other", "Node").unwrap();
        assert_eq!(t.rename(other, "Player"), Err(SceneError::DuplicateName));
        assert_eq!(t.rename(player, "Player"), Ok(()));
        assert_eq!(t.rename(other, "Renamed"), Ok(()));
    }

    #[test]
    fn remove_drops_the_whole_subtree_and_stales_handles() {
        let (mut t, root, player, sprite) = sample();
        assert_eq!(t.remove(player), Ok(()));
        assert_eq!(t.len(), 1);
        assert!(t.node(player).is_none() && t.node(sprite).is_none());
        assert_eq!(t.node(root).map(|n| n.children().len()), Some(0));
        assert_eq!(t.remove(player), Err(SceneError::UnknownNode));
        assert_eq!(t.remove(root), Err(SceneError::RootIsFixed));
        assert_eq!(
            t.add_child(sprite, "X", "Node"),
            Err(SceneError::UnknownNode)
        );
    }

    #[test]
    fn reparent_rejects_cycles_duplicates_and_the_root() {
        let (mut t, root, player, sprite) = sample();
        assert_eq!(t.reparent(player, sprite), Err(SceneError::WouldCycle));
        assert_eq!(t.reparent(player, player), Err(SceneError::WouldCycle));
        assert_eq!(t.reparent(root, player), Err(SceneError::RootIsFixed));
        let other = t.add_child(root, "Other", "Node").unwrap();
        let twin = t.add_child(other, "Sprite", "Sprite2D").unwrap();
        assert_eq!(t.reparent(twin, player), Err(SceneError::DuplicateName));
        assert_eq!(t.reparent(sprite, other), Err(SceneError::DuplicateName));
        let lone = t.add_child(root, "Lone", "Node").unwrap();
        assert_eq!(t.reparent(lone, player), Ok(()));
        assert_eq!(t.path_of(lone).as_deref(), Some("Player/Lone"));
        assert_eq!(
            t.node(root).map(|n| n.children().contains(&lone)),
            Some(false)
        );
    }

    #[test]
    fn depth_and_size_are_bounded() {
        let mut t = SceneTree::new("R", "Node").unwrap();
        let mut cur = t.root();
        for i in 0..MAX_TREE_DEPTH {
            cur = t.add_child(cur, &format!("n{i}"), "Node").unwrap();
        }
        assert_eq!(t.add_child(cur, "deeper", "Node"), Err(SceneError::TooDeep));
        let side = t.add_child(t.root(), "side", "Node").unwrap();
        assert_eq!(t.reparent(side, cur), Err(SceneError::TooDeep));
        let chain_top = t.find(t.root(), "n0").unwrap();
        let s2 = t.add_child(t.root(), "s2", "Node").unwrap();
        assert_eq!(t.reparent(chain_top, s2), Err(SceneError::TooDeep));
    }

    #[test]
    fn properties_validate_keys() {
        let (mut t, _, player, _) = sample();
        assert_eq!(
            t.set_property(player, "position", Variant::from(1_i64)),
            Ok(())
        );
        assert_eq!(t.property(player, "position"), Some(&Variant::Int(1)));
        assert_eq!(t.set_property(player, "theme/color", Variant::Nil), Ok(()));
        assert_eq!(
            t.set_property(player, "bad key", Variant::Nil),
            Err(SceneError::InvalidProperty)
        );
        assert_eq!(
            t.set_property(player, "", Variant::Nil),
            Err(SceneError::InvalidProperty)
        );
        assert_eq!(t.remove_property(player, "position"), Some(Variant::Int(1)));
        assert!(t.property(player, "position").is_none());
    }
}
