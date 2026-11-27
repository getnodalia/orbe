// SPDX-License-Identifier: MIT
//! Orbe's own math. Pure data and arithmetic: no I/O, no allocation, no dependencies.
//!
//! Angles are radians, matrices are column-major, and the coordinate system is right-handed with
//! +Y up in 3D. Every operation that can fail on hostile input (inverting a singular matrix,
//! normalising a zero vector) returns an `Option` instead of producing NaN.

mod color;
mod mat4;
mod quat;
mod rect;
mod transform2d;
mod vec2;
mod vec3;

pub use color::Color;
pub use mat4::Mat4;
pub use quat::Quat;
pub use rect::Rect2;
pub use transform2d::Transform2D;
pub use vec2::Vec2;
pub use vec3::Vec3;

/// Tolerance used by the `approx_eq` helpers.
pub const EPSILON: f32 = 1e-5;

/// True when `a` and `b` differ by less than [`EPSILON`] (scaled for large magnitudes).
pub fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() <= EPSILON * 1.0_f32.max(a.abs()).max(b.abs())
}
