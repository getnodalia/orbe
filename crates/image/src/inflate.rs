// SPDX-License-Identifier: MIT
//! Inflate (RFC 1951) and the zlib wrapper (RFC 1950), written from the specifications.
//!
//! The decoder reads one bit at a time through a canonical Huffman walk. That is slower than a
//! table-driven decoder; it is small and easy to prove safe, and a faster one can replace it
//! behind the same function.

use crate::ImageError;

const MAX_BITS: usize = 15;

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    bit: u32,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Bits {
            data,
            pos: 0,
            bit: 0,
        }
    }

    fn bit(&mut self) -> Result<i32, ImageError> {
        let byte = *self.data.get(self.pos).ok_or(ImageError::Truncated)?;
        let v = i32::from((byte >> self.bit) & 1);
        self.bit += 1;
        if self.bit == 8 {
            self.bit = 0;
            self.pos += 1;
        }
        Ok(v)
    }

    fn bits(&mut self, n: u32) -> Result<u32, ImageError> {
        let mut v = 0u32;
        for i in 0..n {
            v |= (self.bit()? as u32) << i;
        }
        Ok(v)
    }

    /// Skips to the next byte boundary.
    fn align(&mut self) {
        if self.bit != 0 {
            self.bit = 0;
            self.pos += 1;
        }
    }
}

struct Huffman {
    count: [u16; MAX_BITS + 1],
    symbol: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Result<Huffman, ImageError> {
        let mut count = [0u16; MAX_BITS + 1];
        for &l in lengths {
            let slot = count
                .get_mut(usize::from(l))
                .ok_or(ImageError::Corrupt("code length too long"))?;
            *slot += 1;
        }
        // Reject over-subscribed codes; incomplete ones are allowed (a lone distance code).
        let mut left: i32 = 1;
        for len in 1..=MAX_BITS {
            left <<= 1;
            left -= i32::from(count.get(len).copied().unwrap_or(0));
            if left < 0 {
                return Err(ImageError::Corrupt("over-subscribed Huffman code"));
            }
        }
        let mut offsets = [0u16; MAX_BITS + 2];
        for len in 1..=MAX_BITS {
            let next = offsets
                .get(len)
                .copied()
                .unwrap_or(0)
                .wrapping_add(count.get(len).copied().unwrap_or(0));
            if let Some(slot) = offsets.get_mut(len + 1) {
                *slot = next;
            }
        }
        let mut symbol = vec![0u16; lengths.len()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                if let Some(off) = offsets.get_mut(usize::from(l)) {
                    if let Some(slot) = symbol.get_mut(usize::from(*off)) {
                        *slot = sym as u16;
                    }
                    *off += 1;
                }
            }
        }
        Ok(Huffman { count, symbol })
    }

    fn decode(&self, bits: &mut Bits) -> Result<u16, ImageError> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for len in 1..=MAX_BITS {
            code |= bits.bit()?;
            let count = i32::from(self.count.get(len).copied().unwrap_or(0));
            if code - count < first {
                let at = usize::try_from(index + (code - first))
                    .map_err(|_| ImageError::Corrupt("bad Huffman symbol"))?;
                return self
                    .symbol
                    .get(at)
                    .copied()
                    .ok_or(ImageError::Corrupt("bad Huffman symbol"));
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err(ImageError::Corrupt("invalid Huffman code"))
    }
}

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const CODE_LENGTH_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

fn inflate_codes(
    bits: &mut Bits,
    lit: &Huffman,
    dist: &Huffman,
    out: &mut Vec<u8>,
    limit: usize,
) -> Result<(), ImageError> {
    loop {
        let sym = usize::from(lit.decode(bits)?);
        if sym < 256 {
            if out.len() >= limit {
                return Err(ImageError::TooLarge);
            }
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let li = sym - 257;
            let base = *LENGTH_BASE
                .get(li)
                .ok_or(ImageError::Corrupt("bad length symbol"))?;
            let extra = *LENGTH_EXTRA.get(li).unwrap_or(&0);
            let len = usize::from(base) + bits.bits(u32::from(extra))? as usize;
            let ds = usize::from(dist.decode(bits)?);
            let dbase = *DIST_BASE
                .get(ds)
                .ok_or(ImageError::Corrupt("bad distance symbol"))?;
            let dextra = *DIST_EXTRA.get(ds).unwrap_or(&0);
            let d = usize::from(dbase) + bits.bits(u32::from(dextra))? as usize;
            if d > out.len() {
                return Err(ImageError::Corrupt("distance too far back"));
            }
            if out.len() + len > limit {
                return Err(ImageError::TooLarge);
            }
            let start = out.len() - d;
            for i in 0..len {
                let b = out.get(start + i).copied().unwrap_or(0);
                out.push(b);
            }
        }
    }
}

fn fixed_tables() -> Result<(Huffman, Huffman), ImageError> {
    let mut l = [0u8; 288];
    for (i, v) in l.iter_mut().enumerate() {
        *v = match i {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    Ok((Huffman::new(&l)?, Huffman::new(&[5u8; 30])?))
}

fn dynamic_tables(bits: &mut Bits) -> Result<(Huffman, Huffman), ImageError> {
    let nlen = bits.bits(5)? as usize + 257;
    let ndist = bits.bits(5)? as usize + 1;
    let ncode = bits.bits(4)? as usize + 4;
    if nlen > 286 || ndist > 30 {
        return Err(ImageError::Corrupt("too many codes"));
    }
    let mut cl = [0u8; 19];
    for &slot in CODE_LENGTH_ORDER.iter().take(ncode) {
        if let Some(v) = cl.get_mut(slot) {
            *v = bits.bits(3)? as u8;
        }
    }
    let clh = Huffman::new(&cl)?;
    let mut lengths = vec![0u8; nlen + ndist];
    let mut i = 0usize;
    while i < nlen + ndist {
        let sym = clh.decode(bits)?;
        if sym < 16 {
            if let Some(v) = lengths.get_mut(i) {
                *v = sym as u8;
            }
            i += 1;
        } else {
            let (value, repeat) = match sym {
                16 => {
                    let prev = if i == 0 {
                        return Err(ImageError::Corrupt("repeat with no previous length"));
                    } else {
                        lengths.get(i - 1).copied().unwrap_or(0)
                    };
                    (prev, 3 + bits.bits(2)? as usize)
                }
                17 => (0, 3 + bits.bits(3)? as usize),
                _ => (0, 11 + bits.bits(7)? as usize),
            };
            if i + repeat > nlen + ndist {
                return Err(ImageError::Corrupt("code lengths overflow"));
            }
            for _ in 0..repeat {
                if let Some(v) = lengths.get_mut(i) {
                    *v = value;
                }
                i += 1;
            }
        }
    }
    let (ll, dl) = lengths.split_at(nlen);
    if ll.get(256).copied().unwrap_or(0) == 0 {
        return Err(ImageError::Corrupt("no end-of-block code"));
    }
    Ok((Huffman::new(ll)?, Huffman::new(dl)?))
}

/// Inflates a raw deflate stream; returns the output and how many input bytes it used. Output
/// larger than `limit` bytes is refused.
pub fn inflate(data: &[u8], limit: usize) -> Result<(Vec<u8>, usize), ImageError> {
    let mut bits = Bits::new(data);
    let mut out = Vec::new();
    loop {
        let last = bits.bit()?;
        match bits.bits(2)? {
            0 => {
                bits.align();
                let b = |off: usize| data.get(bits.pos + off).copied().map(usize::from);
                let (Some(l0), Some(l1), Some(n0), Some(n1)) = (b(0), b(1), b(2), b(3)) else {
                    return Err(ImageError::Truncated);
                };
                let len = l0 | (l1 << 8);
                let nlen = n0 | (n1 << 8);
                if len != (!nlen & 0xffff) {
                    return Err(ImageError::Corrupt("stored block length mismatch"));
                }
                let start = bits.pos + 4;
                let chunk = data.get(start..start + len).ok_or(ImageError::Truncated)?;
                if out.len() + len > limit {
                    return Err(ImageError::TooLarge);
                }
                out.extend_from_slice(chunk);
                bits.pos = start + len;
            }
            1 => {
                let (l, d) = fixed_tables()?;
                inflate_codes(&mut bits, &l, &d, &mut out, limit)?;
            }
            2 => {
                let (l, d) = dynamic_tables(&mut bits)?;
                inflate_codes(&mut bits, &l, &d, &mut out, limit)?;
            }
            _ => return Err(ImageError::Corrupt("reserved block type")),
        }
        if last == 1 {
            break;
        }
    }
    bits.align();
    Ok((out, bits.pos))
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += u32::from(x);
            b += a;
        }
        a %= 65_521;
        b %= 65_521;
    }
    (b << 16) | a
}

/// Decompresses a zlib stream (RFC 1950), checking its header and Adler-32.
pub fn zlib_decompress(data: &[u8], limit: usize) -> Result<Vec<u8>, ImageError> {
    let (Some(&cmf), Some(&flg)) = (data.first(), data.get(1)) else {
        return Err(ImageError::Truncated);
    };
    if cmf & 0x0f != 8 || (u32::from(cmf) * 256 + u32::from(flg)) % 31 != 0 {
        return Err(ImageError::Corrupt("bad zlib header"));
    }
    if flg & 0x20 != 0 {
        return Err(ImageError::Unsupported("zlib preset dictionary"));
    }
    let body = data.get(2..).ok_or(ImageError::Truncated)?;
    let (out, used) = inflate(body, limit)?;
    let sum = body
        .get(used..used + 4)
        .ok_or(ImageError::Truncated)?
        .iter()
        .fold(0u32, |acc, &b| (acc << 8) | u32::from(b));
    if sum != adler32(&out) {
        return Err(ImageError::Corrupt("zlib checksum mismatch"));
    }
    Ok(out)
}
