use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseColorError {
    InvalidLength(usize),
    InvalidDigit(char),
}

impl fmt::Display for ParseColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseColorError::InvalidLength(n) => {
                write!(
                    f,
                    "invalid hex color length: {n} (expected 3 or 6 hex digits)"
                )
            }
            ParseColorError::InvalidDigit(c) => write!(f, "invalid hex digit: {c:?}"),
        }
    }
}

impl std::error::Error for ParseColorError {}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn r(&self) -> u8 {
        self.r
    }

    pub const fn g(&self) -> u8 {
        self.g
    }

    pub const fn b(&self) -> u8 {
        self.b
    }

    pub fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    pub fn from_hex(hex: &str) -> Result<Self, ParseColorError> {
        let trimmed = hex.trim();
        let body = trimmed.strip_prefix('#').unwrap_or(trimmed);
        let digits: Vec<char> = body.chars().collect();

        match digits.len() {
            3 => {
                let r = nibble(digits[0])?;
                let g = nibble(digits[1])?;
                let b = nibble(digits[2])?;
                Ok(Self::rgb(r * 17, g * 17, b * 17))
            }
            6 => {
                let r = byte(digits[0], digits[1])?;
                let g = byte(digits[2], digits[3])?;
                let b = byte(digits[4], digits[5])?;
                Ok(Self::rgb(r, g, b))
            }
            n => Err(ParseColorError::InvalidLength(n)),
        }
    }

    pub fn to_hsl(&self) -> (f64, f64, f64) {
        let r = self.r as f64 / 255.0;
        let g = self.g as f64 / 255.0;
        let b = self.b as f64 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let l = (max + min) / 2.0;

        if delta == 0.0 {
            return (0.0, 0.0, l);
        }

        let s = delta / (1.0 - (2.0 * l - 1.0).abs());
        let h = if max == r {
            60.0 * ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };

        (h.rem_euclid(360.0), s, l)
    }

    pub fn from_hsl(h: f64, s: f64, l: f64) -> Self {
        let h = h.rem_euclid(360.0);
        let s = s.clamp(0.0, 1.0);
        let l = l.clamp(0.0, 1.0);

        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let h_prime = h / 60.0;
        let x = c * (1.0 - (h_prime.rem_euclid(2.0) - 1.0).abs());

        let (r1, g1, b1) = if h_prime < 1.0 {
            (c, x, 0.0)
        } else if h_prime < 2.0 {
            (x, c, 0.0)
        } else if h_prime < 3.0 {
            (0.0, c, x)
        } else if h_prime < 4.0 {
            (0.0, x, c)
        } else if h_prime < 5.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        let m = l - c / 2.0;
        Self::rgb(to_u8(r1 + m), to_u8(g1 + m), to_u8(b1 + m))
    }

    pub fn to_oklch(&self) -> (f64, f64, f64) {
        let (l, a, b) = self.to_oklab();
        let c = (a * a + b * b).sqrt();
        let h = b.atan2(a).to_degrees().rem_euclid(360.0);
        (l, c, h)
    }

    pub fn from_oklch(l: f64, c: f64, h: f64) -> Self {
        let h_rad = h.to_radians();
        let a = c * h_rad.cos();
        let b = c * h_rad.sin();
        Self::from_oklab(l, a, b)
    }

    pub(crate) fn to_oklab(self) -> (f64, f64, f64) {
        let r = srgb_to_linear(self.r as f64 / 255.0);
        let g = srgb_to_linear(self.g as f64 / 255.0);
        let b = srgb_to_linear(self.b as f64 / 255.0);

        let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
        let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
        let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

        let l_ = l.cbrt();
        let m_ = m.cbrt();
        let s_ = s.cbrt();

        let ll = 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_;
        let aa = 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_;
        let bb = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_;

        (ll, aa, bb)
    }

    pub(crate) fn from_oklab(l: f64, a: f64, b: f64) -> Self {
        let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
        let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
        let s_ = l - 0.0894841775 * a - 1.2914855480 * b;

        let l3 = l_ * l_ * l_;
        let m3 = m_ * m_ * m_;
        let s3 = s_ * s_ * s_;

        let r = 4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3;
        let g = -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3;
        let b = -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3;

        Self::rgb(
            to_u8(linear_to_srgb(r)),
            to_u8(linear_to_srgb(g)),
            to_u8(linear_to_srgb(b)),
        )
    }
}

fn nibble(c: char) -> Result<u8, ParseColorError> {
    c.to_digit(16)
        .map(|v| v as u8)
        .ok_or(ParseColorError::InvalidDigit(c))
}

fn byte(hi: char, lo: char) -> Result<u8, ParseColorError> {
    Ok(nibble(hi)? * 16 + nibble(lo)?)
}

fn to_u8(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ColorVisitor;

        impl Visitor<'_> for ColorVisitor {
            type Value = Color;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(r##"a hex color string like "#rrggbb""##)
            }

            fn visit_str<E>(self, v: &str) -> Result<Color, E>
            where
                E: de::Error,
            {
                Color::from_hex(v).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(ColorVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) {
        assert!((a - b).abs() <= eps, "expected {a} ≈ {b} (±{eps})");
    }

    #[test]
    fn rgb_to_hex_roundtrips_formatting() {
        assert_eq!(Color::rgb(0xE6, 0xB4, 0x22).to_hex(), "#e6b422");
        assert_eq!(Color::rgb(0, 0, 0).to_hex(), "#000000");
        assert_eq!(Color::rgb(255, 255, 255).to_hex(), "#ffffff");
    }

    #[test]
    fn from_rgb_matches_rgb() {
        assert_eq!(Color::from_rgb(1, 2, 3), Color::rgb(1, 2, 3));
        let c = Color::rgb(10, 20, 30);
        assert_eq!((c.r(), c.g(), c.b()), (10, 20, 30));
    }

    #[test]
    fn hex_parse_forms() {
        assert_eq!(
            Color::from_hex("#e6b422").unwrap(),
            Color::rgb(0xE6, 0xB4, 0x22)
        );
        assert_eq!(
            Color::from_hex("e6b422").unwrap(),
            Color::rgb(0xE6, 0xB4, 0x22)
        );
        assert_eq!(
            Color::from_hex("#ABC").unwrap(),
            Color::rgb(0xAA, 0xBB, 0xCC)
        );
        assert_eq!(
            Color::from_hex("abc").unwrap(),
            Color::rgb(0xAA, 0xBB, 0xCC)
        );
        assert_eq!(
            Color::from_hex("  #FFFFFF  ").unwrap(),
            Color::rgb(255, 255, 255)
        );
    }

    #[test]
    fn hex_parse_errors() {
        assert_eq!(
            Color::from_hex("#12"),
            Err(ParseColorError::InvalidLength(2))
        );
        assert_eq!(
            Color::from_hex("#1234"),
            Err(ParseColorError::InvalidLength(4))
        );
        assert!(matches!(
            Color::from_hex("#gg0000"),
            Err(ParseColorError::InvalidDigit('g'))
        ));
    }

    #[test]
    fn hex_roundtrips() {
        for c in [
            Color::rgb(0, 0, 0),
            Color::rgb(255, 255, 255),
            Color::rgb(0xE6, 0xB4, 0x22),
            Color::rgb(18, 52, 86),
        ] {
            assert_eq!(Color::from_hex(&c.to_hex()).unwrap(), c);
        }
    }

    #[test]
    fn hsl_known_references() {
        let (h, s, l) = Color::rgb(255, 0, 0).to_hsl();
        close(h, 0.0, 1e-9);
        close(s, 1.0, 1e-9);
        close(l, 0.5, 1e-9);

        let (_, s_w, l_w) = Color::rgb(255, 255, 255).to_hsl();
        close(s_w, 0.0, 1e-9);
        close(l_w, 1.0, 1e-9);

        assert_eq!(Color::from_hsl(0.0, 1.0, 0.5), Color::rgb(255, 0, 0));
    }

    #[test]
    fn hsl_roundtrips() {
        for c in [
            Color::rgb(0xE6, 0xB4, 0x22),
            Color::rgb(51, 93, 168),
            Color::rgb(180, 76, 55),
            Color::rgb(26, 132, 127),
            Color::rgb(240, 229, 172),
        ] {
            let (h, s, l) = c.to_hsl();
            let back = Color::from_hsl(h, s, l);
            assert_eq!(back, c, "HSL round-trip failed for {}", c.to_hex());
        }
    }

    #[test]
    fn oklch_known_references() {
        let (l, c, _h) = Color::rgb(255, 255, 255).to_oklch();
        close(l, 1.000, 0.01);
        close(c, 0.000, 0.01);

        let (l, c, h) = Color::rgb(255, 0, 0).to_oklch();
        close(l, 0.6279, 0.01);
        close(c, 0.2577, 0.01);
        close(h, 29.23, 0.5);

        let (l, c, h) = Color::rgb(0, 255, 0).to_oklch();
        close(l, 0.8664, 0.01);
        close(c, 0.2948, 0.01);
        close(h, 142.50, 0.5);

        let (l, c, h) = Color::rgb(0, 0, 255).to_oklch();
        close(l, 0.4520, 0.01);
        close(c, 0.3132, 0.01);
        close(h, 264.05, 0.5);
    }

    #[test]
    fn oklch_srgb_roundtrip_within_one_channel() {
        for c in [
            Color::rgb(0xE6, 0xB4, 0x22),
            Color::rgb(51, 93, 168),
            Color::rgb(180, 76, 55),
            Color::rgb(26, 132, 127),
            Color::rgb(240, 229, 172),
            Color::rgb(90, 83, 104),
        ] {
            let (l, ch, h) = c.to_oklch();
            let back = Color::from_oklch(l, ch, h);
            let dr = (back.r as i32 - c.r as i32).abs();
            let dg = (back.g as i32 - c.g as i32).abs();
            let db = (back.b as i32 - c.b as i32).abs();
            assert!(
                dr <= 1 && dg <= 1 && db <= 1,
                "OKLCH round-trip drifted for {}: got {} (Δ {dr},{dg},{db})",
                c.to_hex(),
                back.to_hex()
            );
        }
    }

    #[test]
    fn serde_is_hex_string() {
        let c = Color::rgb(0xE6, 0xB4, 0x22);
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, "\"#e6b422\"");
        let back: Color = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn hex_parse_empty_and_hash_only_are_zero_length() {
        assert_eq!(Color::from_hex(""), Err(ParseColorError::InvalidLength(0)));
        assert_eq!(Color::from_hex("#"), Err(ParseColorError::InvalidLength(0)));
        assert_eq!(
            Color::from_hex("   "),
            Err(ParseColorError::InvalidLength(0))
        );
    }

    #[test]
    fn hex_parse_bad_digit_in_each_position() {
        assert_eq!(
            Color::from_hex("#0g0000"),
            Err(ParseColorError::InvalidDigit('g'))
        );
        assert_eq!(
            Color::from_hex("#g00"),
            Err(ParseColorError::InvalidDigit('g'))
        );
        assert_eq!(
            Color::from_hex("zzz"),
            Err(ParseColorError::InvalidDigit('z'))
        );
    }

    #[test]
    fn parse_error_display_and_is_std_error() {
        let len = ParseColorError::InvalidLength(4);
        assert_eq!(
            len.to_string(),
            "invalid hex color length: 4 (expected 3 or 6 hex digits)"
        );
        let dig = ParseColorError::InvalidDigit('g');
        assert_eq!(dig.to_string(), "invalid hex digit: 'g'");
        let _boxed: Box<dyn std::error::Error> = Box::new(len);
    }

    #[test]
    fn to_hsl_covers_grey_and_green_blue_maxima() {
        let (h, s, l) = Color::rgb(128, 128, 128).to_hsl();
        close(h, 0.0, 1e-12);
        close(s, 0.0, 1e-12);
        close(l, 128.0 / 255.0, 1e-9);
        let (hg, sg, lg) = Color::rgb(0, 255, 0).to_hsl();
        close(hg, 120.0, 1e-9);
        close(sg, 1.0, 1e-9);
        close(lg, 0.5, 1e-9);
        let (hb, _, _) = Color::rgb(0, 0, 255).to_hsl();
        close(hb, 240.0, 1e-9);
    }

    #[test]
    fn from_hsl_covers_all_six_hue_segments() {
        assert_eq!(Color::from_hsl(0.0, 1.0, 0.5), Color::rgb(255, 0, 0));
        assert_eq!(Color::from_hsl(60.0, 1.0, 0.5), Color::rgb(255, 255, 0));
        assert_eq!(Color::from_hsl(120.0, 1.0, 0.5), Color::rgb(0, 255, 0));
        assert_eq!(Color::from_hsl(180.0, 1.0, 0.5), Color::rgb(0, 255, 255));
        assert_eq!(Color::from_hsl(240.0, 1.0, 0.5), Color::rgb(0, 0, 255));
        assert_eq!(Color::from_hsl(300.0, 1.0, 0.5), Color::rgb(255, 0, 255));
    }

    #[test]
    fn from_hsl_clamps_and_wraps_out_of_range() {
        assert_eq!(Color::from_hsl(0.0, 2.0, 0.5), Color::rgb(255, 0, 0));
        assert_eq!(Color::from_hsl(0.0, 1.0, 2.0), Color::rgb(255, 255, 255));
        assert_eq!(Color::from_hsl(0.0, 1.0, -1.0), Color::rgb(0, 0, 0));
        assert_eq!(Color::from_hsl(0.0, -1.0, 0.5), Color::rgb(128, 128, 128));
        assert_eq!(Color::from_hsl(-60.0, 1.0, 0.5), Color::rgb(255, 0, 255));
    }

    #[test]
    fn oklch_black_is_zero() {
        let (l, c, _h) = Color::rgb(0, 0, 0).to_oklch();
        close(l, 0.0, 1e-9);
        close(c, 0.0, 1e-9);
    }

    #[test]
    fn from_oklch_clamps_out_of_gamut_extremes() {
        assert_eq!(Color::from_oklch(1.5, 0.0, 0.0), Color::rgb(255, 255, 255));
        assert_eq!(Color::from_oklch(-0.5, 0.0, 0.0), Color::rgb(0, 0, 0));
        let c = Color::from_oklch(0.6, 5.0, 29.0);
        assert_eq!(Color::from_hex(&c.to_hex()).unwrap(), c);
    }

    #[test]
    fn serde_rejects_invalid_hex() {
        assert!(serde_json::from_str::<Color>("\"nothex\"").is_err());
        assert!(serde_json::from_str::<Color>("\"#12\"").is_err());
    }
}
