// SPDX-License-Identifier: MIT
use crate::Vec2;

/// A 2D affine transform: columns `x` and `y` are the transformed basis, `origin` the translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2D {
    /// Image of the unit X vector.
    pub x: Vec2,
    /// Image of the unit Y vector.
    pub y: Vec2,
    /// Translation.
    pub origin: Vec2,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform2D {
    /// The identity transform.
    pub const IDENTITY: Transform2D = Transform2D {
        x: Vec2::new(1.0, 0.0),
        y: Vec2::new(0.0, 1.0),
        origin: Vec2::ZERO,
    };

    /// Builds a transform from rotation (radians), scale and translation, applied in that order
    /// of meaning: scale, then rotate, then translate.
    pub fn from_rotation_scale_origin(rotation: f32, scale: Vec2, origin: Vec2) -> Self {
        let (s, c) = rotation.sin_cos();
        Transform2D {
            x: Vec2::new(c * scale.x, s * scale.x),
            y: Vec2::new(-s * scale.y, c * scale.y),
            origin,
        }
    }

    /// Transforms a point (applies translation).
    pub fn apply(self, p: Vec2) -> Vec2 {
        self.x * p.x + self.y * p.y + self.origin
    }

    /// Transforms a direction (ignores translation).
    pub fn apply_vector(self, v: Vec2) -> Vec2 {
        self.x * v.x + self.y * v.y
    }

    /// Determinant of the linear part.
    pub fn determinant(self) -> f32 {
        self.x.cross(self.y)
    }

    /// `self` applied after `o` (so `a.then(b).apply(p) == a.apply(b.apply(p))`).
    pub fn then(self, o: Transform2D) -> Transform2D {
        Transform2D {
            x: self.apply_vector(o.x),
            y: self.apply_vector(o.y),
            origin: self.apply(o.origin),
        }
    }

    /// The inverse transform, or `None` when singular or non-finite.
    pub fn inverse(self) -> Option<Transform2D> {
        let det = self.determinant();
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        let inv = 1.0 / det;
        let x = Vec2::new(self.y.y * inv, -self.x.y * inv);
        let y = Vec2::new(-self.y.x * inv, self.x.x * inv);
        let linear = Transform2D {
            x,
            y,
            origin: Vec2::ZERO,
        };
        let origin = -linear.apply_vector(self.origin);
        Some(Transform2D { x, y, origin })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approx_eq;

    #[test]
    fn inverse_round_trips() {
        let t =
            Transform2D::from_rotation_scale_origin(0.7, Vec2::new(2.0, 3.0), Vec2::new(5.0, -4.0));
        let inv = t.inverse().unwrap();
        let p = Vec2::new(1.5, 2.5);
        let q = inv.apply(t.apply(p));
        assert!(approx_eq(q.x, p.x) && approx_eq(q.y, p.y));
    }

    #[test]
    fn singular_has_no_inverse() {
        let t = Transform2D::from_rotation_scale_origin(0.0, Vec2::new(0.0, 1.0), Vec2::ZERO);
        assert!(t.inverse().is_none());
    }

    #[test]
    fn composition_order() {
        let a = Transform2D::from_rotation_scale_origin(0.0, Vec2::ONE, Vec2::new(1.0, 0.0));
        let b = Transform2D::from_rotation_scale_origin(0.0, Vec2::new(2.0, 2.0), Vec2::ZERO);
        let p = Vec2::new(1.0, 1.0);
        assert_eq!(a.then(b).apply(p), a.apply(b.apply(p)));
    }
}
