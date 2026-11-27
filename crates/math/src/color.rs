// SPDX-License-Identifier: MIT
/// A linear RGBA colour, each channel nominally in `0.0..=1.0`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color {
    /// Red.
    pub r: f32,
    /// Green.
    pub g: f32,
    /// Blue.
    pub b: f32,
    /// Alpha (opacity).
    pub a: f32,
}

impl Color {
    /// Builds a colour from its channels.
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Parses `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa` (the `#` is optional).
    pub fn from_hex(s: &str) -> Option<Color> {
        let s = s.strip_prefix('#').unwrap_or(s);
        if !s.is_ascii() {
            return None;
        }
        let nibble = |c: u8| (c as char).to_digit(16).map(|d| d as f32);
        let bytes = s.as_bytes();
        let (chan, has_alpha): (Vec<f32>, bool) = match bytes.len() {
            3 | 4 => (
                bytes
                    .iter()
                    .map(|&c| nibble(c).map(|d| d * 17.0))
                    .collect::<Option<_>>()?,
                bytes.len() == 4,
            ),
            6 | 8 => (
                bytes
                    .chunks(2)
                    .map(|p| Some(nibble(*p.first()?)? * 16.0 + nibble(*p.get(1)?)?))
                    .collect::<Option<_>>()?,
                bytes.len() == 8,
            ),
            _ => return None,
        };
        let c = |i: usize| chan.get(i).map(|v| v / 255.0);
        Some(Color::new(
            c(0)?,
            c(1)?,
            c(2)?,
            if has_alpha { c(3)? } else { 1.0 },
        ))
    }

    /// The colour as `#rrggbbaa`, clamping each channel.
    pub fn to_hex(self) -> String {
        let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            b(self.r),
            b(self.g),
            b(self.b),
            b(self.a)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_forms() {
        assert_eq!(
            Color::from_hex("#fff"),
            Some(Color::new(1.0, 1.0, 1.0, 1.0))
        );
        assert_eq!(
            Color::from_hex("DDA10A").map(|c| c.to_hex()),
            Some("#dda10aff".to_string())
        );
        assert_eq!(
            Color::from_hex("#00000000"),
            Some(Color::new(0.0, 0.0, 0.0, 0.0))
        );
    }

    #[test]
    fn rejects_garbage() {
        for bad in ["", "#", "#12", "#12345", "#gggggg", "#ñññ", "#1234567"] {
            assert!(Color::from_hex(bad).is_none(), "{bad}");
        }
    }
}
