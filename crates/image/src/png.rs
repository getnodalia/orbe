// SPDX-License-Identifier: MIT
//! PNG decoding from the specification (W3C PNG, third edition).

use crate::inflate::zlib_decompress;
use crate::{Image, ImageError, MAX_DIMENSION, MAX_PIXELS};

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for part in parts {
        for &b in *part {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
    }
    !crc
}

fn be32(b: &[u8]) -> Option<u32> {
    match b {
        [a, b, c, d, ..] => Some(u32::from_be_bytes([*a, *b, *c, *d])),
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct Header {
    width: u32,
    height: u32,
    depth: u8,
    color: u8,
}

impl Header {
    fn channels(self) -> usize {
        match self.color {
            0 | 3 => 1,
            4 => 2,
            2 => 3,
            _ => 4,
        }
    }

    fn row_bytes(self) -> usize {
        (self.width as usize * self.channels() * usize::from(self.depth)).div_ceil(8)
    }
}

fn parse_header(data: &[u8]) -> Result<Header, ImageError> {
    if data.len() != 13 {
        return Err(ImageError::Corrupt("bad IHDR"));
    }
    let width = be32(data).ok_or(ImageError::Truncated)?;
    let height = data.get(4..).and_then(be32).ok_or(ImageError::Truncated)?;
    let (Some(&depth), Some(&color), Some(&comp), Some(&filter), Some(&interlace)) = (
        data.get(8),
        data.get(9),
        data.get(10),
        data.get(11),
        data.get(12),
    ) else {
        return Err(ImageError::Truncated);
    };
    if width == 0 || height == 0 {
        return Err(ImageError::Corrupt("empty image"));
    }
    if width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err(ImageError::TooLarge);
    }
    let depth_ok = match color {
        0 => matches!(depth, 1 | 2 | 4 | 8 | 16),
        3 => matches!(depth, 1 | 2 | 4 | 8),
        2 | 4 | 6 => matches!(depth, 8 | 16),
        _ => return Err(ImageError::Corrupt("unknown colour type")),
    };
    if !depth_ok {
        return Err(ImageError::Corrupt("bit depth not allowed for colour type"));
    }
    if comp != 0 || filter != 0 {
        return Err(ImageError::Corrupt("unknown compression or filter method"));
    }
    match interlace {
        0 => {}
        1 => return Err(ImageError::Unsupported("interlaced PNG")),
        _ => return Err(ImageError::Corrupt("unknown interlace method")),
    }
    Ok(Header {
        width,
        height,
        depth,
        color,
    })
}

fn at(v: &[u8], i: usize) -> u8 {
    v.get(i).copied().unwrap_or(0)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (ia, ib, ic) = (i32::from(a), i32::from(b), i32::from(c));
    let p = ia + ib - ic;
    let (pa, pb, pc) = ((p - ia).abs(), (p - ib).abs(), (p - ic).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn unfilter(kind: u8, cur: &mut [u8], prev: &[u8], bpp: usize) -> Result<(), ImageError> {
    for i in 0..cur.len() {
        let left = if i >= bpp { at(cur, i - bpp) } else { 0 };
        let up = at(prev, i);
        let up_left = if i >= bpp { at(prev, i - bpp) } else { 0 };
        let add = match kind {
            0 => 0,
            1 => left,
            2 => up,
            3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
            4 => paeth(left, up, up_left),
            _ => return Err(ImageError::Corrupt("unknown filter type")),
        };
        if let Some(slot) = cur.get_mut(i) {
            *slot = slot.wrapping_add(add);
        }
    }
    Ok(())
}

/// The `n`th sample of a row, at the image's bit depth.
fn sample(row: &[u8], depth: u8, n: usize) -> u16 {
    match depth {
        8 => u16::from(at(row, n)),
        16 => (u16::from(at(row, n * 2)) << 8) | u16::from(at(row, n * 2 + 1)),
        d => {
            let d = usize::from(d);
            let bit = n * d;
            let byte = at(row, bit / 8);
            let shift = 8 - d - (bit % 8);
            u16::from((byte >> shift) & ((1u8 << d) - 1))
        }
    }
}

fn to8(value: u16, depth: u8) -> u8 {
    match depth {
        16 => (value >> 8) as u8,
        8 => value as u8,
        d => {
            let max = (1u32 << d) - 1;
            (u32::from(value) * 255 / max) as u8
        }
    }
}

struct Extras {
    palette: Vec<[u8; 3]>,
    /// Palette alpha values, or the colour-key sample(s) for grey and RGB images.
    trns: Vec<u8>,
}

fn convert_row(
    h: Header,
    row: &[u8],
    extras: &Extras,
    out: &mut Vec<u8>,
) -> Result<(), ImageError> {
    let key = |n: usize| -> u16 {
        (u16::from(at(&extras.trns, n * 2)) << 8) | u16::from(at(&extras.trns, n * 2 + 1))
    };
    for x in 0..h.width as usize {
        let px: [u8; 4] = match h.color {
            0 => {
                let g = sample(row, h.depth, x);
                let a = if extras.trns.len() >= 2 && g == key(0) {
                    0
                } else {
                    255
                };
                let v = to8(g, h.depth);
                [v, v, v, a]
            }
            2 => {
                let (r, g, b) = (
                    sample(row, h.depth, x * 3),
                    sample(row, h.depth, x * 3 + 1),
                    sample(row, h.depth, x * 3 + 2),
                );
                let a = if extras.trns.len() >= 6 && r == key(0) && g == key(1) && b == key(2) {
                    0
                } else {
                    255
                };
                [to8(r, h.depth), to8(g, h.depth), to8(b, h.depth), a]
            }
            3 => {
                let i = usize::from(sample(row, h.depth, x));
                let [r, g, b] = *extras
                    .palette
                    .get(i)
                    .ok_or(ImageError::Corrupt("palette index out of range"))?;
                [r, g, b, extras.trns.get(i).copied().unwrap_or(255)]
            }
            4 => {
                let v = to8(sample(row, h.depth, x * 2), h.depth);
                [v, v, v, to8(sample(row, h.depth, x * 2 + 1), h.depth)]
            }
            _ => [
                to8(sample(row, h.depth, x * 4), h.depth),
                to8(sample(row, h.depth, x * 4 + 1), h.depth),
                to8(sample(row, h.depth, x * 4 + 2), h.depth),
                to8(sample(row, h.depth, x * 4 + 3), h.depth),
            ],
        };
        out.extend_from_slice(&px);
    }
    Ok(())
}

/// Decodes a PNG file into 8-bit RGBA.
pub fn decode_png(bytes: &[u8]) -> Result<Image, ImageError> {
    if bytes.get(..8) != Some(&SIGNATURE[..]) {
        return Err(ImageError::Corrupt("not a PNG file"));
    }
    let mut pos = 8usize;
    let mut header: Option<Header> = None;
    let mut extras = Extras {
        palette: Vec::new(),
        trns: Vec::new(),
    };
    let mut idat = Vec::new();
    let mut ended = false;
    while !ended {
        let len = bytes
            .get(pos..)
            .and_then(be32)
            .ok_or(ImageError::Truncated)? as usize;
        let kind = bytes.get(pos + 4..pos + 8).ok_or(ImageError::Truncated)?;
        let data = bytes
            .get(pos + 8..pos + 8 + len)
            .ok_or(ImageError::Truncated)?;
        let crc = bytes
            .get(pos + 8 + len..)
            .and_then(be32)
            .ok_or(ImageError::Truncated)?;
        if crc != crc32(&[kind, data]) {
            return Err(ImageError::Corrupt("chunk checksum mismatch"));
        }
        pos += 12 + len;
        match kind {
            b"IHDR" => {
                if header.is_some() {
                    return Err(ImageError::Corrupt("two IHDR chunks"));
                }
                header = Some(parse_header(data)?);
            }
            _ if header.is_none() => return Err(ImageError::Corrupt("IHDR is not first")),
            b"PLTE" => {
                if data.len() % 3 != 0 || data.len() > 768 {
                    return Err(ImageError::Corrupt("bad palette"));
                }
                extras.palette = data
                    .chunks_exact(3)
                    .map(|c| [at(c, 0), at(c, 1), at(c, 2)])
                    .collect();
            }
            b"tRNS" => extras.trns = data.to_vec(),
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => ended = true,
            _ => {}
        }
    }
    let h = header.ok_or(ImageError::Corrupt("missing IHDR"))?;
    if h.color == 3 && extras.palette.is_empty() {
        return Err(ImageError::Corrupt("missing palette"));
    }
    let row_bytes = h.row_bytes();
    let expected = (row_bytes + 1)
        .checked_mul(h.height as usize)
        .ok_or(ImageError::TooLarge)?;
    let raw = zlib_decompress(&idat, expected)?;
    if raw.len() != expected {
        return Err(ImageError::Corrupt("image data has the wrong length"));
    }
    let bpp = (h.channels() * usize::from(h.depth)).div_ceil(8).max(1);
    let mut rgba = Vec::with_capacity(h.width as usize * h.height as usize * 4);
    let mut prev = vec![0u8; row_bytes];
    for line in raw.chunks_exact(row_bytes + 1) {
        let Some((&kind, data)) = line.split_first() else {
            break;
        };
        let mut cur = data.to_vec();
        unfilter(kind, &mut cur, &prev, bpp)?;
        convert_row(h, &cur, &extras, &mut rgba)?;
        prev = cur;
    }
    Ok(Image {
        width: h.width,
        height: h.height,
        rgba,
    })
}
