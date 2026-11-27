// SPDX-License-Identifier: MIT
use core::ops::{Add, Div, Mul, Neg, Sub};

/// A 2D vector of `f32`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    /// Horizontal component.
    pub x: f32,
    /// Vertical component.
    pub y: f32,
}

impl Vec2 {
    /// The zero vector.
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    /// The vector with both components one.
    pub const ONE: Vec2 = Vec2 { x: 1.0, y: 1.0 };

    /// Builds a vector from its components.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Dot product.
    pub fn dot(self, o: Vec2) -> f32 {
        self.x * o.x + self.y * o.y
    }

    /// 2D cross product (the z component of the 3D one).
    pub fn cross(self, o: Vec2) -> f32 {
        self.x * o.y - self.y * o.x
    }

    /// Squared length.
    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    /// Length.
    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    /// The unit vector in the same direction, or `None` for a zero or non-finite vector.
    pub fn normalized(self) -> Option<Vec2> {
        let len = self.length();
        if len > 0.0 && len.is_finite() {
            Some(self / len)
        } else {
            None
        }
    }

    /// Linear interpolation from `self` to `o`.
    pub fn lerp(self, o: Vec2, t: f32) -> Vec2 {
        self + (o - self) * t
    }

    /// The vector rotated by `angle` radians.
    pub fn rotated(self, angle: f32) -> Vec2 {
        let (s, c) = angle.sin_cos();
        Vec2::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }

    /// True when both components are finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f32) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}
impl Div<f32> for Vec2 {
    type Output = Vec2;
    fn div(self, s: f32) -> Vec2 {
        Vec2::new(self.x / s, self.y / s)
    }
}
impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approx_eq;

    #[test]
    fn arithmetic_and_length() {
        let v = Vec2::new(3.0, 4.0);
        assert_eq!(v.length(), 5.0);
        assert_eq!(v + Vec2::ONE, Vec2::new(4.0, 5.0));
        assert_eq!(-v, Vec2::new(-3.0, -4.0));
        assert_eq!(Vec2::new(1.0, 0.0).cross(Vec2::new(0.0, 1.0)), 1.0);
    }

    #[test]
    fn normalizing_zero_and_nan_is_none() {
        assert!(Vec2::ZERO.normalized().is_none());
        assert!(Vec2::new(f32::NAN, 1.0).normalized().is_none());
        assert!(Vec2::new(f32::INFINITY, 0.0).normalized().is_none());
        let n = Vec2::new(0.0, 9.0).normalized();
        assert_eq!(n, Some(Vec2::new(0.0, 1.0)));
    }

    #[test]
    fn rotation_quarter_turn() {
        let r = Vec2::new(1.0, 0.0).rotated(core::f32::consts::FRAC_PI_2);
        assert!(approx_eq(r.x, 0.0) || r.x.abs() < 1e-5);
        assert!(approx_eq(r.y, 1.0));
    }
}
