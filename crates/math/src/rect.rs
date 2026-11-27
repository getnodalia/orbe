// SPDX-License-Identifier: MIT
use crate::Vec2;

/// An axis-aligned rectangle given by its top-left position and its size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect2 {
    /// Top-left corner.
    pub position: Vec2,
    /// Width and height (may be negative for an unnormalised rectangle).
    pub size: Vec2,
}

impl Rect2 {
    /// Builds a rectangle from position and size.
    pub const fn new(position: Vec2, size: Vec2) -> Self {
        Self { position, size }
    }

    /// Bottom-right corner.
    pub fn end(self) -> Vec2 {
        self.position + self.size
    }

    /// The same rectangle with a non-negative size.
    pub fn abs(self) -> Rect2 {
        let a = self.position;
        let b = self.end();
        let min = Vec2::new(a.x.min(b.x), a.y.min(b.y));
        let max = Vec2::new(a.x.max(b.x), a.y.max(b.y));
        Rect2::new(min, max - min)
    }

    /// True when `p` is inside (the top and left edges are inside, the bottom and right are not).
    pub fn contains(self, p: Vec2) -> bool {
        let r = self.abs();
        let e = r.end();
        p.x >= r.position.x && p.y >= r.position.y && p.x < e.x && p.y < e.y
    }

    /// The overlap of two rectangles, or `None` when they do not intersect.
    pub fn intersection(self, o: Rect2) -> Option<Rect2> {
        let (a, b) = (self.abs(), o.abs());
        let min = Vec2::new(
            a.position.x.max(b.position.x),
            a.position.y.max(b.position.y),
        );
        let (ae, be) = (a.end(), b.end());
        let max = Vec2::new(ae.x.min(be.x), ae.y.min(be.y));
        if max.x > min.x && max.y > min.y {
            Some(Rect2::new(min, max - min))
        } else {
            None
        }
    }

    /// The smallest rectangle containing both.
    pub fn merge(self, o: Rect2) -> Rect2 {
        let (a, b) = (self.abs(), o.abs());
        let min = Vec2::new(
            a.position.x.min(b.position.x),
            a.position.y.min(b.position.y),
        );
        let (ae, be) = (a.end(), b.end());
        let max = Vec2::new(ae.x.max(be.x), ae.y.max(be.y));
        Rect2::new(min, max - min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect2 {
        Rect2::new(Vec2::new(x, y), Vec2::new(w, h))
    }

    #[test]
    fn contains_edges() {
        let a = r(0.0, 0.0, 10.0, 10.0);
        assert!(a.contains(Vec2::new(0.0, 0.0)));
        assert!(!a.contains(Vec2::new(10.0, 5.0)));
    }

    #[test]
    fn negative_size_is_normalised() {
        assert_eq!(r(10.0, 10.0, -10.0, -10.0).abs(), r(0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn intersection_and_merge() {
        let a = r(0.0, 0.0, 10.0, 10.0);
        let b = r(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.intersection(b), Some(r(5.0, 5.0, 5.0, 5.0)));
        assert_eq!(a.merge(b), r(0.0, 0.0, 15.0, 15.0));
        assert!(a.intersection(r(20.0, 20.0, 1.0, 1.0)).is_none());
        assert!(a.intersection(r(10.0, 0.0, 5.0, 5.0)).is_none());
    }
}
