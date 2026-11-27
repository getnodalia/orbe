// SPDX-License-Identifier: MIT
//! Textures: images a sprite is painted with.

use crate::raster::{RenderError, MAX_DIMENSION, MAX_PIXELS};
use std::collections::BTreeMap;
use std::sync::Arc;

/// How a texture is sampled between texels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    /// The nearest texel; crisp pixels.
    #[default]
    Nearest,
    /// A blend of the four nearest texels.
    Linear,
}

/// An RGBA image in linear colour with straight (not premultiplied) alpha, rows from the top.
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    width: u32,
    height: u32,
    texels: Vec<[f32; 4]>,
}

fn srgb_to_linear(byte: u8) -> f32 {
    let c = f32::from(byte) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

impl Texture {
    /// Decodes 8-bit sRGB RGBA bytes (`width * height * 4`, straight alpha), the format image
    /// files hold. The size must be within the limits of [`crate::render`].
    pub fn from_rgba8(width: u32, height: u32, bytes: &[u8]) -> Result<Texture, RenderError> {
        if width == 0 || height == 0 {
            return Err(RenderError::EmptyImage);
        }
        if width > MAX_DIMENSION
            || height > MAX_DIMENSION
            || u64::from(width) * u64::from(height) > MAX_PIXELS
        {
            return Err(RenderError::TooLarge);
        }
        if bytes.len() as u64 != u64::from(width) * u64::from(height) * 4 {
            return Err(RenderError::WrongLength);
        }
        let texels = bytes
            .chunks_exact(4)
            .map(|c| match c {
                [r, g, b, a] => [
                    srgb_to_linear(*r),
                    srgb_to_linear(*g),
                    srgb_to_linear(*b),
                    f32::from(*a) / 255.0,
                ],
                _ => [0.0; 4],
            })
            .collect();
        Ok(Texture {
            width,
            height,
            texels,
        })
    }

    /// Decodes a PNG file into a texture.
    pub fn from_png(bytes: &[u8]) -> Result<Texture, RenderError> {
        use orbe_image::ImageError;
        let img = orbe_image::decode_png(bytes).map_err(|e| match e {
            ImageError::TooLarge => RenderError::TooLarge,
            ImageError::Truncated => RenderError::Undecodable("the image file is cut short"),
            ImageError::Corrupt(why) | ImageError::Unsupported(why) => {
                RenderError::Undecodable(why)
            }
        })?;
        Texture::from_rgba8(img.width, img.height, &img.rgba)
    }

    /// Width in texels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in texels.
    pub fn height(&self) -> u32 {
        self.height
    }

    fn texel(&self, x: i64, y: i64) -> [f32; 4] {
        let x = x.clamp(0, i64::from(self.width) - 1) as usize;
        let y = y.clamp(0, i64::from(self.height) - 1) as usize;
        self.texels
            .get(y * self.width as usize + x)
            .copied()
            .unwrap_or([0.0; 4])
    }

    /// The linear straight-alpha colour at `(u, v)` in `0..=1` across the texture, with the edges
    /// clamped. Non-finite coordinates sample the top-left texel.
    pub fn sample(&self, u: f32, v: f32, filter: Filter) -> [f32; 4] {
        let (u, v) = if u.is_finite() && v.is_finite() {
            (u.clamp(0.0, 1.0), v.clamp(0.0, 1.0))
        } else {
            (0.0, 0.0)
        };
        let fx = u * self.width as f32;
        let fy = v * self.height as f32;
        match filter {
            Filter::Nearest => self.texel(fx.floor() as i64, fy.floor() as i64),
            Filter::Linear => {
                let (gx, gy) = (fx - 0.5, fy - 0.5);
                let (x0, y0) = (gx.floor(), gy.floor());
                let (tx, ty) = (gx - x0, gy - y0);
                let (x0, y0) = (x0 as i64, y0 as i64);
                let a = self.texel(x0, y0);
                let b = self.texel(x0 + 1, y0);
                let c = self.texel(x0, y0 + 1);
                let d = self.texel(x0 + 1, y0 + 1);
                let mut out = [0.0; 4];
                for (i, o) in out.iter_mut().enumerate() {
                    let at = |t: &[f32; 4]| t.get(i).copied().unwrap_or(0.0);
                    let top = at(&a) + (at(&b) - at(&a)) * tx;
                    let bottom = at(&c) + (at(&d) - at(&c)) * tx;
                    *o = top + (bottom - top) * ty;
                }
                out
            }
        }
    }
}

/// The textures a scene can name, looked up by the path its `texture` property holds.
#[derive(Clone, Debug, Default)]
pub struct Textures {
    map: BTreeMap<String, Arc<Texture>>,
}

/// Most textures one store may hold.
pub const MAX_TEXTURES: usize = 65_536;

impl Textures {
    /// An empty store.
    pub fn new() -> Textures {
        Textures::default()
    }

    /// Adds or replaces the texture at `path`. Returns `false` when the store is full.
    pub fn insert(&mut self, path: &str, texture: Texture) -> bool {
        if self.map.len() >= MAX_TEXTURES && !self.map.contains_key(path) {
            return false;
        }
        self.map.insert(path.to_string(), Arc::new(texture));
        true
    }

    /// The texture at `path`.
    pub fn get(&self, path: &str) -> Option<Arc<Texture>> {
        self.map.get(path).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_by_one() -> Texture {
        Texture::from_rgba8(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255]).unwrap()
    }

    #[test]
    fn nearest_picks_texels_and_clamps() {
        let t = two_by_one();
        assert_eq!(t.sample(0.25, 0.5, Filter::Nearest), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(t.sample(0.75, 0.5, Filter::Nearest), [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(t.sample(9.0, -3.0, Filter::Nearest), [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(
            t.sample(f32::NAN, 0.0, Filter::Nearest),
            [1.0, 0.0, 0.0, 1.0]
        );
    }

    #[test]
    fn linear_blends_at_the_seam() {
        let t = two_by_one();
        let mid = t.sample(0.5, 0.5, Filter::Linear);
        assert!((mid[0] - 0.5).abs() < 1e-5 && (mid[2] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn bad_sizes_are_refused() {
        assert_eq!(Texture::from_rgba8(0, 1, &[]), Err(RenderError::EmptyImage));
        assert!(Texture::from_png(b"nope").is_err());
        let png = [
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0xf4, 0x22, 0x7f, 0x8a, 0x00, 0x00, 0x00, 0x0e, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x42, 0xff, 0x01, 0x0f, 0xf9, 0x03, 0xfd, 0x85,
            0x11, 0x99, 0x76, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60,
            0x82,
        ];
        let tex = Texture::from_png(&png).unwrap();
        assert_eq!((tex.width(), tex.height()), (2, 1));
        assert_eq!(tex.sample(0.25, 0.5, Filter::Nearest), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(
            Texture::from_rgba8(1, 1, &[0; 3]),
            Err(RenderError::WrongLength)
        );
        assert_eq!(
            Texture::from_rgba8(MAX_DIMENSION + 1, 1, &[]),
            Err(RenderError::TooLarge)
        );
    }

    #[test]
    fn store_inserts_and_finds() {
        let mut s = Textures::new();
        assert!(s.insert("res://a.png", two_by_one()));
        assert!(s.get("res://a.png").is_some());
        assert!(s.get("res://b.png").is_none());
    }
}
