// SPDX-License-Identifier: MIT
//! Turns a scene tree into a painting-ordered list of shapes.

use crate::texture::{Filter, Texture, Textures};
use orbe_core::{Handle, Variant};
use orbe_math::{Color, Transform2D, Vec2};
use orbe_scene::{Node, SceneTree};
use std::sync::Arc;

/// Most points a polygon may have; longer ones are skipped.
pub const MAX_POLYGON_POINTS: usize = 65_536;

/// What to draw.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A rectangle from the local origin to `size`.
    Rect(Vec2),
    /// A filled polygon in local coordinates.
    Polygon(Vec<Vec2>),
    /// A rectangle from the local origin to `size`, painted with a texture (multiplied by the
    /// item's colour).
    Sprite {
        /// Size in local units.
        size: Vec2,
        /// The image.
        texture: Arc<Texture>,
        /// How texels are sampled.
        filter: Filter,
    },
}

/// One shape with everything needed to paint it.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawItem {
    /// Local to world transform.
    pub world: Transform2D,
    /// Painting order; higher is on top.
    pub z: i64,
    /// The shape.
    pub shape: Shape,
    /// Colour with the inherited `modulate` already applied.
    pub color: Color,
}

/// The view onto the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// World point shown at the centre of the image.
    pub center: Vec2,
    /// Magnification.
    pub zoom: Vec2,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            center: Vec2::ZERO,
            zoom: Vec2::ONE,
        }
    }
}

/// A scene ready to paint.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrawList {
    /// Shapes from back to front.
    pub items: Vec<DrawItem>,
    /// The active camera, or `None` to look at the origin from the top-left corner of the image.
    pub camera: Option<Camera>,
}

fn finite(v: Vec2) -> bool {
    v.is_finite()
}

fn number(v: &Variant) -> Option<f32> {
    let f = match v {
        Variant::Float(f) => *f as f32,
        Variant::Int(i) => *i as f32,
        _ => return None,
    };
    f.is_finite().then_some(f)
}

fn vec2(tree: &SceneTree, h: Handle<Node>, key: &str) -> Option<Vec2> {
    match tree.property(h, key)? {
        Variant::Vec2(v) if finite(*v) => Some(*v),
        other => number(other).map(|n| Vec2::new(n, n)),
    }
}

fn polygon(tree: &SceneTree, h: Handle<Node>) -> Option<Vec<Vec2>> {
    let Variant::Array(items) = tree.property(h, "polygon")? else {
        return None;
    };
    if items.len() > MAX_POLYGON_POINTS * 2 {
        return None;
    }
    let points: Option<Vec<Vec2>> = if items.iter().all(|v| matches!(v, Variant::Vec2(_))) {
        items
            .iter()
            .map(|v| {
                if let Variant::Vec2(p) = v {
                    Some(*p)
                } else {
                    None
                }
            })
            .collect()
    } else {
        let nums: Option<Vec<f32>> = items.iter().map(number).collect();
        nums.map(|n| {
            n.chunks_exact(2)
                .filter_map(|c| match c {
                    [x, y] => Some(Vec2::new(*x, *y)),
                    _ => None,
                })
                .collect()
        })
    };
    let points = points?;
    (points.len() >= 3 && points.len() <= MAX_POLYGON_POINTS && points.iter().all(|p| finite(*p)))
        .then_some(points)
}

fn color(tree: &SceneTree, h: Handle<Node>, key: &str) -> Option<Color> {
    match tree.property(h, key)? {
        Variant::Color(c) if [c.r, c.g, c.b, c.a].iter().all(|x| x.is_finite()) => Some(*c),
        _ => None,
    }
}

fn local_transform(tree: &SceneTree, h: Handle<Node>) -> Transform2D {
    let position = vec2(tree, h, "position").unwrap_or(Vec2::ZERO);
    let scale = vec2(tree, h, "scale").unwrap_or(Vec2::ONE);
    let rotation = tree.property(h, "rotation").and_then(number).unwrap_or(0.0);
    Transform2D::from_rotation_scale_origin(rotation, scale, position)
}

fn multiply(a: Color, b: Color) -> Color {
    Color::new(a.r * b.r, a.g * b.g, a.b * b.b, a.a * b.a)
}

struct Frame {
    node: Handle<Node>,
    world: Transform2D,
    z: i64,
    modulate: Color,
}

/// Builds the draw list for a scene: shapes in painting order (by `z`, ties in tree order) and the
/// first enabled camera.
pub fn build_draw_list(tree: &SceneTree) -> DrawList {
    build_draw_list_with(tree, &Textures::new())
}

/// Like [`build_draw_list`], and `Sprite2D` nodes whose `texture` property names an entry of
/// `textures` are drawn too.
pub fn build_draw_list_with(tree: &SceneTree, textures: &Textures) -> DrawList {
    let mut list = DrawList::default();
    let mut stack = vec![Frame {
        node: tree.root(),
        world: Transform2D::IDENTITY,
        z: 0,
        modulate: Color::new(1.0, 1.0, 1.0, 1.0),
    }];
    while let Some(f) = stack.pop() {
        let Some(node) = tree.node(f.node) else {
            continue;
        };
        if !matches!(tree.property(f.node, "visible"), Some(Variant::Bool(false))) {
            let world = f.world.then(local_transform(tree, f.node));
            let z = f.z.saturating_add(match tree.property(f.node, "z_index") {
                Some(Variant::Int(i)) => (*i).clamp(-4096, 4096),
                _ => 0,
            });
            let modulate = match color(tree, f.node, "modulate") {
                Some(m) => multiply(f.modulate, m),
                None => f.modulate,
            };
            match node.kind() {
                "ColorRect" => {
                    if let Some(size) = vec2(tree, f.node, "size") {
                        let c =
                            color(tree, f.node, "color").unwrap_or(Color::new(1.0, 1.0, 1.0, 1.0));
                        list.items.push(DrawItem {
                            world,
                            z,
                            shape: Shape::Rect(size),
                            color: multiply(c, modulate),
                        });
                    }
                }
                "Polygon2D" => {
                    if let Some(points) = polygon(tree, f.node) {
                        let c =
                            color(tree, f.node, "color").unwrap_or(Color::new(1.0, 1.0, 1.0, 1.0));
                        list.items.push(DrawItem {
                            world,
                            z,
                            shape: Shape::Polygon(points),
                            color: multiply(c, modulate),
                        });
                    }
                }
                "Sprite2D" => {
                    let texture = match tree.property(f.node, "texture") {
                        Some(Variant::Str(path)) => textures.get(path),
                        _ => None,
                    };
                    if let Some(texture) = texture {
                        let natural = Vec2::new(texture.width() as f32, texture.height() as f32);
                        let size = vec2(tree, f.node, "size")
                            .filter(|s| s.x > 0.0 && s.y > 0.0)
                            .unwrap_or(natural);
                        let centered = !matches!(
                            tree.property(f.node, "centered"),
                            Some(Variant::Bool(false))
                        );
                        let world = if centered {
                            world.then(Transform2D::from_rotation_scale_origin(
                                0.0,
                                Vec2::ONE,
                                Vec2::new(-size.x / 2.0, -size.y / 2.0),
                            ))
                        } else {
                            world
                        };
                        let filter = match tree.property(f.node, "texture_filter") {
                            Some(Variant::Str(s)) if s == "linear" => Filter::Linear,
                            _ => Filter::Nearest,
                        };
                        let c =
                            color(tree, f.node, "color").unwrap_or(Color::new(1.0, 1.0, 1.0, 1.0));
                        list.items.push(DrawItem {
                            world,
                            z,
                            shape: Shape::Sprite {
                                size,
                                texture,
                                filter,
                            },
                            color: multiply(c, modulate),
                        });
                    }
                }
                "Camera2D"
                    if list.camera.is_none()
                        && !matches!(
                            tree.property(f.node, "enabled"),
                            Some(Variant::Bool(false))
                        ) =>
                {
                    let zoom = vec2(tree, f.node, "zoom")
                        .filter(|z| z.x > 0.0 && z.y > 0.0)
                        .unwrap_or(Vec2::ONE);
                    list.camera = Some(Camera {
                        center: world.origin,
                        zoom,
                    });
                }
                _ => {}
            }
            // Children are pushed in reverse so the first child is painted first.
            for &child in node.children().iter().rev() {
                stack.push(Frame {
                    node: child,
                    world,
                    z,
                    modulate,
                });
            }
        }
    }
    list.items.sort_by_key(|i| i.z);
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(tree: &mut SceneTree, parent: Handle<Node>, name: &str, size: f32) -> Handle<Node> {
        let h = tree.add_child(parent, name, "ColorRect").unwrap();
        tree.set_property(h, "size", Variant::Vec2(Vec2::new(size, size)))
            .unwrap();
        h
    }

    #[test]
    fn transforms_compose_down_the_tree() {
        let mut t = SceneTree::new("Root", "Node2D").unwrap();
        let root = t.root();
        t.set_property(root, "position", Variant::Vec2(Vec2::new(10.0, 0.0)))
            .unwrap();
        let a = t.add_child(root, "A", "Node2D").unwrap();
        t.set_property(a, "position", Variant::Vec2(Vec2::new(0.0, 5.0)))
            .unwrap();
        t.set_property(a, "scale", Variant::Vec2(Vec2::new(2.0, 2.0)))
            .unwrap();
        rect(&mut t, a, "R", 4.0);
        let list = build_draw_list(&t);
        assert_eq!(list.items.len(), 1);
        let w = list.items[0].world;
        assert_eq!(w.apply(Vec2::new(1.0, 1.0)), Vec2::new(12.0, 7.0));
    }

    #[test]
    fn z_order_visibility_and_modulate() {
        let mut t = SceneTree::new("Root", "Node2D").unwrap();
        let root = t.root();
        t.set_property(
            root,
            "modulate",
            Variant::Color(Color::new(1.0, 1.0, 1.0, 0.5)),
        )
        .unwrap();
        let front = rect(&mut t, root, "Front", 1.0);
        t.set_property(front, "z_index", Variant::Int(5)).unwrap();
        let _back = rect(&mut t, root, "Back", 2.0);
        let hidden = rect(&mut t, root, "Hidden", 3.0);
        t.set_property(hidden, "visible", Variant::Bool(false))
            .unwrap();
        let list = build_draw_list(&t);
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].shape, Shape::Rect(Vec2::new(2.0, 2.0)));
        assert_eq!(list.items[1].z, 5);
        assert_eq!(list.items[1].color.a, 0.5);
    }

    #[test]
    fn camera_and_polygons() {
        let mut t = SceneTree::new("Root", "Node2D").unwrap();
        let root = t.root();
        let cam = t.add_child(root, "Cam", "Camera2D").unwrap();
        t.set_property(cam, "position", Variant::Vec2(Vec2::new(3.0, 4.0)))
            .unwrap();
        t.set_property(cam, "zoom", Variant::Float(2.0)).unwrap();
        let p = t.add_child(root, "P", "Polygon2D").unwrap();
        let flat: Vec<Variant> = [0.0, 0.0, 4.0, 0.0, 0.0, 4.0]
            .iter()
            .map(|f| Variant::Float(*f))
            .collect();
        t.set_property(p, "polygon", Variant::Array(flat)).unwrap();
        let bad = t.add_child(root, "Bad", "Polygon2D").unwrap();
        t.set_property(
            bad,
            "polygon",
            Variant::Array(vec![Variant::Float(f64::NAN); 6]),
        )
        .unwrap();
        let list = build_draw_list(&t);
        assert_eq!(
            list.camera,
            Some(Camera {
                center: Vec2::new(3.0, 4.0),
                zoom: Vec2::new(2.0, 2.0)
            })
        );
        assert_eq!(list.items.len(), 1);
        assert!(matches!(&list.items[0].shape, Shape::Polygon(pts) if pts.len() == 3));
    }

    #[test]
    fn hostile_values_are_ignored() {
        let mut t = SceneTree::new("Root", "Node2D").unwrap();
        let root = t.root();
        let r = rect(&mut t, root, "R", 1.0);
        t.set_property(r, "position", Variant::Vec2(Vec2::new(f32::NAN, 0.0)))
            .unwrap();
        t.set_property(r, "rotation", Variant::Float(f64::INFINITY))
            .unwrap();
        t.set_property(r, "z_index", Variant::Int(i64::MAX))
            .unwrap();
        t.set_property(r, "color", Variant::Str("red".into()))
            .unwrap();
        let list = build_draw_list(&t);
        assert_eq!(list.items.len(), 1);
        assert_eq!(list.items[0].z, 4096);
        assert_eq!(list.items[0].world, Transform2D::IDENTITY);
    }

    #[test]
    fn sprites_need_a_known_texture_and_centre_by_default() {
        let mut t = SceneTree::new("Root", "Node2D").unwrap();
        let root = t.root();
        let s = t.add_child(root, "S", "Sprite2D").unwrap();
        t.set_property(s, "texture", Variant::Str("res://a.png".into()))
            .unwrap();
        assert!(build_draw_list(&t).items.is_empty());
        let mut store = Textures::new();
        store.insert(
            "res://a.png",
            Texture::from_rgba8(2, 2, &[255; 16]).unwrap(),
        );
        let list = build_draw_list_with(&t, &store);
        assert_eq!(list.items.len(), 1);
        assert_eq!(list.items[0].world.apply(Vec2::new(1.0, 1.0)), Vec2::ZERO);
        t.set_property(s, "centered", Variant::Bool(false)).unwrap();
        let list = build_draw_list_with(&t, &store);
        assert_eq!(list.items[0].world.apply(Vec2::ZERO), Vec2::ZERO);
    }
}
