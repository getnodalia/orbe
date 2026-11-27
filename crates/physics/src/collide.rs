// SPDX-License-Identifier: MIT
//! Narrow-phase collision tests.

use orbe_math::Vec2;

/// One contact point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// World position of the contact.
    pub point: Vec2,
    /// How deep the shapes overlap there; positive when overlapping.
    pub depth: f32,
}

/// The contact between two shapes: a normal pointing from the first shape to the second and one
/// or two points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Manifold {
    /// Unit normal from the first shape to the second.
    pub normal: Vec2,
    /// The contact points; `count` of them are valid.
    pub points: [Contact; 2],
    /// How many of `points` are valid (1 or 2).
    pub count: usize,
}

impl Manifold {
    fn one(normal: Vec2, point: Vec2, depth: f32) -> Manifold {
        let c = Contact { point, depth };
        Manifold {
            normal,
            points: [c, c],
            count: 1,
        }
    }

    /// The valid contact points.
    pub fn contacts(&self) -> &[Contact] {
        self.points.get(..self.count).unwrap_or(&[])
    }

    /// The same contact seen from the other shape.
    pub(crate) fn flipped(self) -> Manifold {
        Manifold {
            normal: -self.normal,
            ..self
        }
    }
}

/// Circle against circle.
pub(crate) fn circle_circle(pa: Vec2, ra: f32, pb: Vec2, rb: f32) -> Option<Manifold> {
    let d = pb - pa;
    let r = ra + rb;
    let dist2 = d.length_squared();
    if dist2 >= r * r || dist2.is_nan() {
        return None;
    }
    let dist = dist2.sqrt();
    let normal = if dist > 1e-6 {
        d / dist
    } else {
        Vec2::new(0.0, 1.0)
    };
    let depth = r - dist;
    Some(Manifold::one(
        normal,
        pa + normal * (ra - depth / 2.0),
        depth,
    ))
}

/// The corners of a box, counter-clockwise.
pub(crate) fn box_vertices(pos: Vec2, angle: f32, half: Vec2) -> [Vec2; 4] {
    let local = [
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(half.x, half.y),
        Vec2::new(-half.x, half.y),
    ];
    local.map(|p| pos + p.rotated(angle))
}

/// Circle against a convex polygon with counter-clockwise vertices.
pub(crate) fn circle_polygon(pc: Vec2, r: f32, poly: &[Vec2]) -> Option<Manifold> {
    let n = poly.len();
    // Find the edge the circle is most outside of.
    let mut best = f32::NEG_INFINITY;
    let mut best_i = 0;
    for i in 0..n {
        let (a, b) = (*poly.get(i)?, *poly.get((i + 1) % n)?);
        let e = b - a;
        let normal = Vec2::new(e.y, -e.x).normalized()?;
        let s = normal.dot(pc - a);
        if s > best {
            best = s;
            best_i = i;
        }
    }
    if best > r {
        return None;
    }
    let a = *poly.get(best_i)?;
    let b = *poly.get((best_i + 1) % n)?;
    let e = b - a;
    let face_normal = Vec2::new(e.y, -e.x).normalized()?;
    // Normal from the polygon to the circle.
    let (normal, depth, point) = if best < 0.0 {
        // Centre inside: push out through the closest face.
        (face_normal, r - best, pc - face_normal * best)
    } else {
        let t = ((pc - a).dot(e) / e.length_squared()).clamp(0.0, 1.0);
        let closest = a + e * t;
        let d = pc - closest;
        let dist = d.length();
        if dist > r {
            return None;
        }
        let normal = if dist > 1e-6 { d / dist } else { face_normal };
        (normal, r - dist, closest)
    };
    // The manifold's normal points from the first shape (circle) to the second (polygon).
    Some(Manifold::one(-normal, point, depth))
}

/// The largest separation of `b` from the edges of `a`, and the edge it is on.
fn max_separation(a: &[Vec2], b: &[Vec2]) -> Option<(f32, usize)> {
    let n = a.len();
    let mut best = (f32::NEG_INFINITY, 0);
    for i in 0..n {
        let (v1, v2) = (*a.get(i)?, *a.get((i + 1) % n)?);
        let e = v2 - v1;
        let normal = Vec2::new(e.y, -e.x).normalized()?;
        let mut min = f32::INFINITY;
        for &p in b {
            min = min.min(normal.dot(p - v1));
        }
        if min > best.0 {
            best = (min, i);
        }
    }
    Some(best)
}

/// Clips the segment `seg` to the half plane `dot(normal, p) <= offset`.
fn clip(seg: [Vec2; 2], normal: Vec2, offset: f32) -> Option<[Vec2; 2]> {
    let d0 = normal.dot(seg[0]) - offset;
    let d1 = normal.dot(seg[1]) - offset;
    let mut out = Vec::with_capacity(3);
    if d0 <= 0.0 {
        out.push(seg[0]);
    }
    if d1 <= 0.0 {
        out.push(seg[1]);
    }
    if d0 * d1 < 0.0 {
        let t = d0 / (d0 - d1);
        out.push(seg[0] + (seg[1] - seg[0]) * t);
    }
    match out[..] {
        [a, b] | [a, b, _] => Some([a, b]),
        _ => None,
    }
}

/// Convex polygon against convex polygon (both counter-clockwise) by separating axes and edge
/// clipping.
pub(crate) fn polygon_polygon(a: &[Vec2], b: &[Vec2]) -> Option<Manifold> {
    let (sep_a, edge_a) = max_separation(a, b)?;
    let (sep_b, edge_b) = max_separation(b, a)?;
    if sep_a > 0.0 || sep_b > 0.0 {
        return None;
    }
    // Prefer A's face unless B's is clearly better, so contacts do not flicker between them.
    let (reference, incident, edge, flip) = if sep_b > sep_a + 0.000_5 {
        (b, a, edge_b, true)
    } else {
        (a, b, edge_a, false)
    };
    let n = reference.len();
    let v1 = *reference.get(edge)?;
    let v2 = *reference.get((edge + 1) % n)?;
    let tangent = (v2 - v1).normalized()?;
    let normal = Vec2::new(tangent.y, -tangent.x);
    // The incident edge faces the reference face most directly.
    let m = incident.len();
    let mut min_dot = f32::INFINITY;
    let mut inc = 0;
    for i in 0..m {
        let (p, q) = (*incident.get(i)?, *incident.get((i + 1) % m)?);
        let en = Vec2::new((q - p).y, -(q - p).x).normalized()?;
        let d = normal.dot(en);
        if d < min_dot {
            min_dot = d;
            inc = i;
        }
    }
    let seg = [*incident.get(inc)?, *incident.get((inc + 1) % m)?];
    let seg = clip(seg, -tangent, -tangent.dot(v1))?;
    let seg = clip(seg, tangent, tangent.dot(v2))?;
    let mut points = [Contact {
        point: Vec2::ZERO,
        depth: 0.0,
    }; 2];
    let mut count = 0;
    for p in seg {
        let sep = normal.dot(p - v1);
        if sep <= 0.0 {
            if let Some(slot) = points.get_mut(count) {
                *slot = Contact {
                    point: p,
                    depth: -sep,
                };
                count += 1;
            }
        }
    }
    if count == 0 {
        return None;
    }
    let manifold = Manifold {
        normal,
        points,
        count,
    };
    Some(if flip { manifold.flipped() } else { manifold })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, y: f32) -> Vec2 {
        Vec2::new(x, y)
    }

    #[test]
    fn circles() {
        let m = circle_circle(v(0.0, 0.0), 1.0, v(1.5, 0.0), 1.0).unwrap();
        assert_eq!(m.normal, v(1.0, 0.0));
        assert!((m.points[0].depth - 0.5).abs() < 1e-6);
        assert!(circle_circle(v(0.0, 0.0), 1.0, v(2.0, 0.0), 1.0).is_none());
        assert!(circle_circle(v(0.0, 0.0), 1.0, v(f32::NAN, 0.0), 1.0).is_none());
        // Coincident centres still produce a usable normal.
        assert!(circle_circle(v(1.0, 1.0), 1.0, v(1.0, 1.0), 1.0).is_some());
    }

    #[test]
    fn boxes_overlapping_and_apart() {
        let a = box_vertices(v(0.0, 0.0), 0.0, v(1.0, 1.0));
        let b = box_vertices(v(1.8, 0.0), 0.0, v(1.0, 1.0));
        let m = polygon_polygon(&a, &b).unwrap();
        assert!((m.normal.x - 1.0).abs() < 1e-5 && m.normal.y.abs() < 1e-5);
        assert_eq!(m.count, 2);
        assert!((m.points[0].depth - 0.2).abs() < 1e-4);
        let far = box_vertices(v(3.0, 0.0), 0.0, v(1.0, 1.0));
        assert!(polygon_polygon(&a, &far).is_none());
        // The normal always points from the first shape to the second.
        let m = polygon_polygon(&b, &a).unwrap();
        assert!((m.normal.x + 1.0).abs() < 1e-5);
    }

    #[test]
    fn rotated_box_corner_hits_face() {
        let floor = box_vertices(v(0.0, 5.0), 0.0, v(10.0, 1.0));
        let diamond = box_vertices(v(0.0, 3.5), std::f32::consts::FRAC_PI_4, v(1.0, 1.0));
        let m = polygon_polygon(&floor, &diamond).unwrap();
        assert!(m.normal.y < -0.99, "{:?}", m.normal);
        assert!(m.points[0].depth > 0.0);
    }

    #[test]
    fn circle_against_box() {
        let b = box_vertices(v(0.0, 0.0), 0.0, v(1.0, 1.0));
        // Circle above the box: the manifold from circle to box points downwards.
        let m = circle_polygon(v(0.0, -1.4), 0.5, &b).unwrap();
        assert!(m.normal.y > 0.99);
        assert!((m.points[0].depth - 0.1).abs() < 1e-5);
        assert!(circle_polygon(v(0.0, -2.0), 0.5, &b).is_none());
        // Centre inside the box.
        assert!(circle_polygon(v(0.2, 0.1), 0.2, &b).is_some());
        // Near a corner.
        assert!(circle_polygon(v(1.3, -1.3), 0.5, &b).is_some());
        assert!(circle_polygon(v(1.5, -1.5), 0.5, &b).is_none());
    }

    #[test]
    fn degenerate_input_never_panics() {
        let nasty = [f32::NAN, f32::INFINITY, 0.0, 1e30, -1e30];
        for &x in &nasty {
            for &y in &nasty {
                let a = box_vertices(v(x, y), x, v(y, x));
                let b = box_vertices(v(0.0, 0.0), 0.0, v(1.0, 1.0));
                let _ = polygon_polygon(&a, &b);
                let _ = polygon_polygon(&b, &a);
                let _ = circle_polygon(v(x, y), y, &b);
                let _ = circle_polygon(v(0.0, 0.0), 1.0, &a);
                let _ = circle_circle(v(x, y), x, v(y, x), y);
            }
        }
        assert!(polygon_polygon(&[], &[]).is_none());
        assert!(circle_polygon(v(0.0, 0.0), 1.0, &[]).is_none());
    }
}
