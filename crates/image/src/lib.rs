// SPDX-License-Identifier: MIT
//! Orbe's own image decoding. Today that is PNG (all colour types and bit depths, not
//! interlaced), through a built-in inflate. Every input is hostile: sizes are capped before
//! anything is allocated, every checksum is verified, and nothing panics.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod inflate;
mod png;

pub use inflate::{inflate, zlib_decompress};
pub use png::decode_png;

/// Largest width or height accepted.
pub const MAX_DIMENSION: u32 = 16_384;
/// Most pixels accepted.
pub const MAX_PIXELS: u64 = 64 * 1024 * 1024;

/// A decoded image: 8-bit sRGB RGBA with straight alpha, rows from the top.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// Why an image could not be decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageError {
    /// The data ends before the image does.
    Truncated,
    /// The data is not what its format says, with the reason.
    Corrupt(&'static str),
    /// A valid feature this decoder does not handle yet.
    Unsupported(&'static str),
    /// The image or its data exceed the size limits.
    TooLarge,
}

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageError::Truncated => write!(f, "the image data is cut short"),
            ImageError::Corrupt(why) => write!(f, "the image data is corrupt: {why}"),
            ImageError::Unsupported(what) => write!(f, "not supported yet: {what}"),
            ImageError::TooLarge => write!(f, "the image is too large"),
        }
    }
}

impl std::error::Error for ImageError {}

#[cfg(test)]
mod tests;
