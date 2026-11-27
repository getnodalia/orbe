// SPDX-License-Identifier: MIT
//! Triangles for GPU backends: the same shapes the CPU rasteriser draws, in the same order.

use crate::draw::DrawList;
use crate::raster::screen_polygon;
use orbe_math::Vec2;

/// Most points of a polygon that are triangulated; larger ones are not drawn by GPU backends.
pub const MAX_TRIANGULATED_POINTS: usize = 1024;

/// One corner of a triangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    /// Position in image pixels, origin at the top-left corner.
    pub position: [f32; 2],
    /// Linear RGBA with straight alpha.
    pub color: [f32; 4],
}

fn area2(poly: &[Vec2]) -> f32 {
    let mut sum = 0.0;
    let Some(mut prev) = poly.last().copied() else {
        return 0.0;
    };
    for &cur in poly {
        sum += prev.cross(cur);
        prev = cur;
    }
    sum
}

fn in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let d1 = (b - a).cross(p - a);
    let d2 = (c - b).cross(p - b);
    let d3 = (a - c).cross(p - c);
    d1 >= 0.0 && d2 >= 0.0 && d3 >= 0.0
}

/// Splits a simple polygon into triangles by ear clipping, as indices into `poly`. Returns `None`
/// for polygons that are degenerate, self-intersecting or longer than [`MAX_TRIANGULATED_POINTS`].
pub fn triangulate(poly: &[Vec2]) -> Option<Vec<[usize; 3]>> {
    let n = poly.len();
    if !(3..=MAX_TRIANGULATED_POINTS).contains(&n) || !poly.iter().all(|p| p.is_finite()) {
        return None;
    }
    let area = area2(poly);
    if !area.is_finite() || area == 0.0 {
        return None;
    }
    let sign = if area > 0.0 { 1.0 } else { -1.0 };
    let mut idx: Vec<usize> = (0..n).collect();
    let mut out = Vec::with_capacity(n - 2);
    while idx.len() > 3 {
        let m = idx.len();
        let mut clipped = false;
        for k in 0..m {
            let (ia, ib, ic) = (
                *idx.get((k + m - 1) % m)?,
                *idx.get(k)?,
                *idx.get((k + 1) % m)?,
            );
            let (a, b, c) = (*poly.get(ia)?, *poly.get(ib)?, *poly.get(ic)?);
            if (b - a).cross(c - b) * sign <= 0.0 {
                continue;
            }
            let blocked = idx.iter().any(|&j| {
                j != ia
                    && j != ib
                    && j != ic
                    && poly.get(j).is_some_and(|&p| {
                        let (a2, b2, c2) = if sign > 0.0 { (a, b, c) } else { (c, b, a) };
                        in_triangle(p, a2, b2, c2)
                    })
            });
            if blocked {
                continue;
            }
            out.push([ia, ib, ic]);
            idx.remove(k);
            clipped = true;
            break;
        }
        if !clipped {
            return None;
        }
    }
    if let [a, b, c] = idx[..] {
        out.push([a, b, c]);
    }
    Some(out)
}

/// The triangles of every item in `list` for an image of `width` x `height`, back to front.
pub fn build_mesh(list: &DrawList, width: u32, height: u32) -> Vec<Vertex> {
    let mut out = Vec::new();
    for item in &list.items {
        let c = item.color;
        if ![c.r, c.g, c.b, c.a].iter().all(|v| v.is_finite()) || c.a <= 0.0 {
            continue;
        }
        let color = [
            c.r.clamp(0.0, 1.0),
            c.g.clamp(0.0, 1.0),
            c.b.clamp(0.0, 1.0),
            c.a.min(1.0),
        ];
        let Some(pts) = screen_polygon(list, item, width, height) else {
            continue;
        };
        let Some(tris) = triangulate(&pts) else {
            continue;
        };
        for [a, b, c3] in tris {
            for i in [a, b, c3] {
                if let Some(p) = pts.get(i) {
                    out.push(Vertex {
                        position: [p.x, p.y],
                        color,
                    });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{DrawItem, Shape};
    use orbe_math::{Color, Transform2D};

    fn v(x: f32, y: f32) -> Vec2 {
        Vec2::new(x, y)
    }

    fn total_area(poly: &[Vec2], tris: &[[usize; 3]]) -> f32 {
        let _ = poly;
        tris.iter()
            .map(|t| {
                let p: Vec<Vec2> = t.iter().map(|&i| poly[i]).collect();
                area2(&p).abs() / 2.0
            })
            .sum()
    }

    #[test]
    fn squares_and_concave_shapes() {
        let sq = [v(0.0, 0.0), v(4.0, 0.0), v(4.0, 4.0), v(0.0, 4.0)];
        let t = triangulate(&sq).unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(total_area(&sq, &t), 16.0);
        let rev: Vec<Vec2> = sq.iter().rev().copied().collect();
        assert_eq!(total_area(&rev, &triangulate(&rev).unwrap()), 16.0);
        // An L shape of area 12.
        let l = [
            v(0.0, 0.0),
            v(4.0, 0.0),
            v(4.0, 2.0),
            v(2.0, 2.0),
            v(2.0, 4.0),
            v(0.0, 4.0),
        ];
        let t = triangulate(&l).unwrap();
        assert_eq!(t.len(), 4);
        assert_eq!(total_area(&l, &t), 12.0);
    }

    #[test]
    fn bad_polygons_are_refused() {
        assert!(triangulate(&[v(0.0, 0.0), v(1.0, 1.0)]).is_none());
        assert!(triangulate(&[v(0.0, 0.0), v(1.0, 1.0), v(2.0, 2.0)]).is_none());
        assert!(triangulate(&[v(0.0, 0.0), v(f32::NAN, 1.0), v(2.0, 2.0)]).is_none());
        let big: Vec<Vec2> = (0..MAX_TRIANGULATED_POINTS + 1)
            .map(|i| v(i as f32, (i * i) as f32))
            .collect();
        assert!(triangulate(&big).is_none());
        // A bow tie crosses itself; it must not panic, whatever it returns.
        let _ = triangulate(&[v(0.0, 0.0), v(2.0, 2.0), v(2.0, 0.0), v(0.0, 2.0)]);
    }

    #[test]
    fn mesh_follows_draw_order_and_skips_invisible() {
        let item = |x: f32, a: f32| DrawItem {
            world: Transform2D::from_rotation_scale_origin(0.0, Vec2::ONE, v(x, 0.0)),
            z: 0,
            shape: Shape::Rect(v(2.0, 2.0)),
            color: Color::new(1.0, 0.0, 0.0, a),
        };
        let list = DrawList {
            items: vec![item(0.0, 1.0), item(5.0, 0.0), item(8.0, 0.5)],
            camera: None,
        };
        let m = build_mesh(&list, 16, 16);
        assert_eq!(m.len(), 12);
        assert_eq!(m[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert!(m[6..]
            .iter()
            .all(|vx| vx.position[0] >= 8.0 && vx.color[3] == 0.5));
    }
}
