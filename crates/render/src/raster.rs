// SPDX-License-Identifier: MIT
//! The CPU reference rasteriser.

use crate::draw::{DrawItem, DrawList, Shape};
use orbe_math::{Color, Vec2};
use std::fmt;

/// Largest image side, in pixels.
pub const MAX_DIMENSION: u32 = 16_384;
/// Largest image area, in pixels.
pub const MAX_PIXELS: u64 = 64 * 1024 * 1024;

/// Checks an image size against [`MAX_DIMENSION`] and [`MAX_PIXELS`].
pub fn check_size(width: u32, height: u32) -> Result<(), RenderError> {
    if width == 0 || height == 0 {
        return Err(RenderError::EmptyImage);
    }
    if width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err(RenderError::TooLarge);
    }
    Ok(())
}

/// Subsamples per pixel side used for anti-aliasing.
const SAMPLES: usize = 4;

/// Why an image could not be rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderError {
    /// A side is zero.
    EmptyImage,
    /// A side or the area is over [`MAX_DIMENSION`] / [`MAX_PIXELS`].
    TooLarge,
    /// The pixel buffer does not match the image size.
    WrongLength,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RenderError::EmptyImage => "the image has no pixels",
            RenderError::TooLarge => "the image is too large",
            RenderError::WrongLength => "the pixel buffer does not match the image size",
        })
    }
}

impl std::error::Error for RenderError {}

/// An 8-bit sRGB RGBA image, rows from the top.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Image {
    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The bytes: `width * height * 4`, as R, G, B, A with straight alpha.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Wraps existing pixels: `pixels` must hold exactly `width * height * 4` bytes and the size
    /// must be within the limits of [`render`].
    pub fn from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Result<Image, RenderError> {
        if width == 0 || height == 0 {
            return Err(RenderError::EmptyImage);
        }
        if width > MAX_DIMENSION
            || height > MAX_DIMENSION
            || u64::from(width) * u64::from(height) > MAX_PIXELS
        {
            return Err(RenderError::TooLarge);
        }
        if pixels.len() as u64 != u64::from(width) * u64::from(height) * 4 {
            return Err(RenderError::WrongLength);
        }
        Ok(Image {
            width,
            height,
            pixels,
        })
    }

    /// Encodes linear premultiplied RGBA pixels (rows from the top) as an 8-bit sRGB image. Both
    /// the CPU rasteriser and GPU backends finish with this, so they encode identically.
    pub fn from_premultiplied_linear(
        width: u32,
        height: u32,
        pixels: &[[f32; 4]],
    ) -> Result<Image, RenderError> {
        let mut bytes = Vec::with_capacity(pixels.len().saturating_mul(4));
        for px in pixels {
            let [r, g, b, a] = *px;
            let (r, g, b) = if a > 0.0 {
                (r / a, g / a, b / a)
            } else {
                (0.0, 0.0, 0.0)
            };
            bytes.extend_from_slice(&[
                srgb_byte(r),
                srgb_byte(g),
                srgb_byte(b),
                (if a.is_finite() {
                    a.clamp(0.0, 1.0)
                } else {
                    0.0
                } * 255.0
                    + 0.5) as u8,
            ]);
        }
        Image::from_rgba(width, height, bytes)
    }

    /// One pixel, or `None` outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        match self.pixels.get(i..i + 4)? {
            [r, g, b, a] => Some([*r, *g, *b, *a]),
            _ => None,
        }
    }
}

fn srgb_byte(linear: f32) -> u8 {
    let c = if linear.is_finite() {
        linear.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let e = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (e * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

/// Whether `p` is inside `poly` by the even-odd rule.
fn inside(poly: &[Vec2], p: Vec2) -> bool {
    let mut odd = false;
    let Some(mut prev) = poly.last().copied() else {
        return false;
    };
    for &cur in poly {
        if (cur.y > p.y) != (prev.y > p.y) {
            let t = (p.y - cur.y) / (prev.y - cur.y);
            if p.x < cur.x + t * (prev.x - cur.x) {
                odd = !odd;
            }
        }
        prev = cur;
    }
    odd
}

/// The corners of `item` in image pixels, or `None` when it cannot be drawn (too few points or
/// a non-finite coordinate).
pub(crate) fn screen_polygon(
    list: &DrawList,
    item: &DrawItem,
    width: u32,
    height: u32,
) -> Option<Vec<Vec2>> {
    let half = Vec2::new(width as f32 / 2.0, height as f32 / 2.0);
    let (center, zoom) = match list.camera {
        Some(c) => (c.center, c.zoom),
        None => (half, Vec2::ONE),
    };
    let local: Vec<Vec2> = match &item.shape {
        Shape::Rect(s) => vec![Vec2::ZERO, Vec2::new(s.x, 0.0), *s, Vec2::new(0.0, s.y)],
        Shape::Polygon(p) => p.clone(),
    };
    let pts: Vec<Vec2> = local
        .iter()
        .map(|&p| {
            let w = item.world.apply(p);
            match list.camera {
                // The camera centre maps to the image centre, scaled by the zoom.
                Some(_) => Vec2::new(
                    (w.x - center.x) * zoom.x + half.x,
                    (w.y - center.y) * zoom.y + half.y,
                ),
                None => w,
            }
        })
        .collect();
    (pts.len() >= 3 && pts.iter().all(|p| p.is_finite())).then_some(pts)
}

/// Draws `list` into a new `width` x `height` image on a background of `clear`.
pub fn render(
    list: &DrawList,
    width: u32,
    height: u32,
    clear: Color,
) -> Result<Image, RenderError> {
    if width == 0 || height == 0 {
        return Err(RenderError::EmptyImage);
    }
    if width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err(RenderError::TooLarge);
    }
    let (w, h) = (width as usize, height as usize);
    let clear_a = if clear.a.is_finite() {
        clear.a.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let premul = |v: f32| {
        if v.is_finite() {
            v.clamp(0.0, 1.0) * clear_a
        } else {
            0.0
        }
    };
    let mut buf = vec![[premul(clear.r), premul(clear.g), premul(clear.b), clear_a]; w * h];

    for item in &list.items {
        let Some(pts) = screen_polygon(list, item, width, height) else {
            continue;
        };
        let a = item.color.a;
        if !a.is_finite() || a <= 0.0 {
            continue;
        }
        let min_x = pts
            .iter()
            .map(|p| p.x)
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.0);
        let max_x = pts
            .iter()
            .map(|p| p.x)
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .min(width as f32);
        let min_y = pts
            .iter()
            .map(|p| p.y)
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.0);
        let max_y = pts
            .iter()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .min(height as f32);
        if min_x >= max_x || min_y >= max_y {
            continue;
        }
        let (x0, x1, y0, y1) = (
            min_x as usize,
            max_x as usize,
            min_y as usize,
            max_y as usize,
        );
        let src = [item.color.r, item.color.g, item.color.b].map(|v| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        });
        let a = a.min(1.0);
        for y in y0..y1 {
            for x in x0..x1 {
                let mut hits = 0u32;
                for sy in 0..SAMPLES {
                    for sx in 0..SAMPLES {
                        let p = Vec2::new(
                            x as f32 + (sx as f32 + 0.5) / SAMPLES as f32,
                            y as f32 + (sy as f32 + 0.5) / SAMPLES as f32,
                        );
                        if inside(&pts, p) {
                            hits += 1;
                        }
                    }
                }
                if hits == 0 {
                    continue;
                }
                let cov = a * hits as f32 / (SAMPLES * SAMPLES) as f32;
                if let Some(px) = buf.get_mut(y * w + x) {
                    for (d, s) in px.iter_mut().zip(src.iter().chain(std::iter::once(&1.0))) {
                        *d = s * cov + *d * (1.0 - cov);
                    }
                }
            }
        }
    }

    Image::from_premultiplied_linear(width, height, &buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{build_draw_list, DrawItem};
    use orbe_core::Variant;
    use orbe_math::Transform2D;
    use orbe_scene::SceneTree;

    const WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);
    const BLACK: Color = Color::new(0.0, 0.0, 0.0, 1.0);

    fn rect_item(x: f32, y: f32, w: f32, h: f32, color: Color) -> DrawItem {
        DrawItem {
            world: Transform2D::from_rotation_scale_origin(0.0, Vec2::ONE, Vec2::new(x, y)),
            z: 0,
            shape: Shape::Rect(Vec2::new(w, h)),
            color,
        }
    }

    #[test]
    fn rejects_bad_sizes() {
        let l = DrawList::default();
        assert_eq!(render(&l, 0, 4, BLACK), Err(RenderError::EmptyImage));
        assert_eq!(
            render(&l, MAX_DIMENSION + 1, 1, BLACK),
            Err(RenderError::TooLarge)
        );
        assert_eq!(
            render(&l, 16_384, 16_384, BLACK),
            Err(RenderError::TooLarge)
        );
    }

    #[test]
    fn clear_and_exact_rect() {
        let l = DrawList {
            items: vec![rect_item(
                2.0,
                1.0,
                3.0,
                2.0,
                Color::new(1.0, 0.0, 0.0, 1.0),
            )],
            camera: None,
        };
        let img = render(&l, 8, 4, BLACK).unwrap();
        assert_eq!(img.pixel(0, 0), Some([0, 0, 0, 255]));
        assert_eq!(img.pixel(2, 1), Some([255, 0, 0, 255]));
        assert_eq!(img.pixel(4, 2), Some([255, 0, 0, 255]));
        assert_eq!(img.pixel(5, 1), Some([0, 0, 0, 255]));
        assert_eq!(img.pixel(2, 3), Some([0, 0, 0, 255]));
        assert_eq!(img.pixel(8, 0), None);
        assert_eq!(img.pixels().len(), 8 * 4 * 4);
    }

    #[test]
    fn partial_coverage_is_anti_aliased() {
        let l = DrawList {
            items: vec![rect_item(0.0, 0.0, 0.5, 1.0, WHITE)],
            camera: None,
        };
        let img = render(&l, 1, 1, BLACK).unwrap();
        // Half the pixel is white in linear light: 0.5 linear encodes to 188 in sRGB.
        assert_eq!(img.pixel(0, 0), Some([188, 188, 188, 255]));
    }

    #[test]
    fn alpha_blends_over_what_is_below() {
        let half = Color::new(1.0, 1.0, 1.0, 0.5);
        let l = DrawList {
            items: vec![rect_item(0.0, 0.0, 1.0, 1.0, half)],
            camera: None,
        };
        let img = render(&l, 1, 1, BLACK).unwrap();
        assert_eq!(img.pixel(0, 0), Some([188, 188, 188, 255]));
        let transparent = render(&l, 1, 1, Color::new(0.0, 0.0, 0.0, 0.0)).unwrap();
        assert_eq!(transparent.pixel(0, 0), Some([255, 255, 255, 128]));
    }

    #[test]
    fn camera_centres_and_zooms() {
        let mut t = SceneTree::new("Root", "Node2D").unwrap();
        let root = t.root();
        let r = t.add_child(root, "R", "ColorRect").unwrap();
        t.set_property(r, "size", Variant::Vec2(Vec2::new(2.0, 2.0)))
            .unwrap();
        t.set_property(r, "color", Variant::Color(Color::new(0.0, 1.0, 0.0, 1.0)))
            .unwrap();
        t.set_property(r, "position", Variant::Vec2(Vec2::new(-1.0, -1.0)))
            .unwrap();
        let cam = t.add_child(root, "Cam", "Camera2D").unwrap();
        t.set_property(cam, "zoom", Variant::Float(2.0)).unwrap();
        let img = render(&build_draw_list(&t), 8, 8, BLACK).unwrap();
        // The 2x2 square centred on the camera fills the middle 4x4 pixels at zoom 2.
        assert_eq!(img.pixel(2, 2), Some([0, 255, 0, 255]));
        assert_eq!(img.pixel(5, 5), Some([0, 255, 0, 255]));
        assert_eq!(img.pixel(1, 1), Some([0, 0, 0, 255]));
        assert_eq!(img.pixel(6, 6), Some([0, 0, 0, 255]));
    }

    #[test]
    fn triangles_and_rotation() {
        let tri = DrawItem {
            world: Transform2D::IDENTITY,
            z: 0,
            shape: Shape::Polygon(vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(8.0, 0.0),
                Vec2::new(0.0, 8.0),
            ]),
            color: WHITE,
        };
        let img = render(
            &DrawList {
                items: vec![tri],
                camera: None,
            },
            8,
            8,
            BLACK,
        )
        .unwrap();
        assert_eq!(img.pixel(1, 1), Some([255, 255, 255, 255]));
        assert_eq!(img.pixel(7, 7), Some([0, 0, 0, 255]));
        let mut spin = rect_item(4.0, 4.0, 2.0, 2.0, WHITE);
        spin.world = Transform2D::from_rotation_scale_origin(
            std::f32::consts::FRAC_PI_2,
            Vec2::ONE,
            Vec2::new(4.0, 4.0),
        );
        let img = render(
            &DrawList {
                items: vec![spin],
                camera: None,
            },
            8,
            8,
            BLACK,
        )
        .unwrap();
        // Rotated a quarter turn the square covers x in 2..4, y in 4..6.
        assert_eq!(img.pixel(3, 4), Some([255, 255, 255, 255]));
        assert_eq!(img.pixel(5, 5), Some([0, 0, 0, 255]));
    }

    #[test]
    fn hostile_items_do_not_panic() {
        let nasty = [f32::NAN, f32::INFINITY, -f32::INFINITY, 1e30, -1e30, 0.0];
        for &a in &nasty {
            for &b in &nasty {
                let mut item = rect_item(a, b, b, a, Color::new(a, b, a, b));
                item.world.x = Vec2::new(a, b);
                let l = DrawList {
                    items: vec![item],
                    camera: None,
                };
                let _ = render(&l, 4, 4, Color::new(a, b, a, b));
            }
        }
    }
}
