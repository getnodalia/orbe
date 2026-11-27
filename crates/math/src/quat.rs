// SPDX-License-Identifier: MIT
use crate::Vec3;
use core::ops::Mul;

/// A rotation quaternion (`x`, `y`, `z` imaginary, `w` real).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    /// Imaginary i.
    pub x: f32,
    /// Imaginary j.
    pub y: f32,
    /// Imaginary k.
    pub z: f32,
    /// Real part.
    pub w: f32,
}

impl Default for Quat {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Quat {
    /// The identity rotation.
    pub const IDENTITY: Quat = Quat {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    /// Rotation of `angle` radians around `axis`, or `None` when the axis is zero or non-finite.
    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Option<Quat> {
        let a = axis.normalized()?;
        let (s, c) = (angle * 0.5).sin_cos();
        Some(Quat {
            x: a.x * s,
            y: a.y * s,
            z: a.z * s,
            w: c,
        })
    }

    /// Squared norm.
    pub fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }

    /// The unit quaternion, or `None` for a zero or non-finite one.
    pub fn normalized(self) -> Option<Quat> {
        let len = self.length_squared().sqrt();
        if len > 0.0 && len.is_finite() {
            Some(Quat {
                x: self.x / len,
                y: self.y / len,
                z: self.z / len,
                w: self.w / len,
            })
        } else {
            None
        }
    }

    /// The conjugate (the inverse for a unit quaternion).
    pub fn conjugate(self) -> Quat {
        Quat {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: self.w,
        }
    }

    /// Rotates a vector.
    pub fn rotate(self, v: Vec3) -> Vec3 {
        let q = Vec3::new(self.x, self.y, self.z);
        let t = q.cross(v) * 2.0;
        v + t * self.w + q.cross(t)
    }
}

impl Mul for Quat {
    type Output = Quat;
    /// Hamilton product: `(a * b).rotate(v) == a.rotate(b.rotate(v))`.
    fn mul(self, o: Quat) -> Quat {
        Quat {
            x: self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            y: self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            z: self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
            w: self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approx_eq;
    use core::f32::consts::FRAC_PI_2;

    #[test]
    fn quarter_turn_about_y() {
        let q = Quat::from_axis_angle(Vec3::Y, FRAC_PI_2).unwrap();
        let v = q.rotate(Vec3::X);
        assert!(v.x.abs() < 1e-5 && v.y.abs() < 1e-5 && approx_eq(v.z, -1.0));
    }

    #[test]
    fn zero_axis_is_none() {
        assert!(Quat::from_axis_angle(Vec3::ZERO, 1.0).is_none());
    }

    #[test]
    fn product_composes() {
        let a = Quat::from_axis_angle(Vec3::Z, 0.4).unwrap();
        let b = Quat::from_axis_angle(Vec3::X, 0.9).unwrap();
        let v = Vec3::new(1.0, 2.0, 3.0);
        let l = (a * b).rotate(v);
        let r = a.rotate(b.rotate(v));
        assert!(approx_eq(l.x, r.x) && approx_eq(l.y, r.y) && approx_eq(l.z, r.z));
        let back = a.conjugate().rotate(a.rotate(v));
        assert!(approx_eq(back.x, v.x) && approx_eq(back.y, v.y) && approx_eq(back.z, v.z));
    }
}
