use std::fmt::Write as _;

use crate::color::Color;
use crate::contrast::relative_luminance;
use crate::formats::{FormatError, ThemeFormat};
use crate::palette::{Palette, Variant};

pub struct Ghostty;

impl ThemeFormat for Ghostty {
    fn id(&self) -> &'static str {
        "ghostty"
    }

    fn display_name(&self) -> &'static str {
        "Ghostty"
    }

    fn file_extension(&self) -> &'static str {
        ""
    }

    fn export(&self, p: &Palette) -> String {
        let mut out = String::new();

        let _ = writeln!(out, "background = {}", p.background.to_hex());
        let _ = writeln!(out, "foreground = {}", p.foreground.to_hex());
        let _ = writeln!(out, "cursor-color = {}", p.cursor.to_hex());
        let _ = writeln!(out, "cursor-text = {}", p.cursor_text.to_hex());
        let _ = writeln!(
            out,
            "selection-background = {}",
            p.selection_background.to_hex()
        );
        let _ = writeln!(
            out,
            "selection-foreground = {}",
            p.selection_foreground.to_hex()
        );

        for (i, c) in p.ansi.iter().enumerate() {
            let _ = writeln!(out, "palette = {i}={}", c.to_hex());
        }

        if let Some(v) = p.minimum_contrast {
            let _ = writeln!(out, "minimum-contrast = {}", format_min_contrast(v));
        }

        out
    }

    fn import(&self, t: &str) -> Result<Palette, FormatError> {
        let mut p = Palette {
            name: "Imported (Ghostty)".to_string(),
            author: None,
            ..Palette::default()
        };

        for raw in t.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();

            match key {
                "background" => p.background = parse_hex(value)?,
                "foreground" => p.foreground = parse_hex(value)?,
                "cursor-color" => p.cursor = parse_hex(value)?,
                "cursor-text" => p.cursor_text = parse_hex(value)?,
                "selection-background" => p.selection_background = parse_hex(value)?,
                "selection-foreground" => p.selection_foreground = parse_hex(value)?,
                "palette" => {
                    let Some((idx_str, hex)) = value.split_once('=') else {
                        return Err(FormatError::parse(format!(
                            "malformed palette entry: {value:?}"
                        )));
                    };
                    let idx: usize = idx_str.trim().parse().map_err(|_| {
                        FormatError::parse(format!("invalid palette index: {:?}", idx_str.trim()))
                    })?;
                    if idx > 15 {
                        return Err(FormatError::parse(format!(
                            "palette index out of range (0..=15): {idx}"
                        )));
                    }
                    p.ansi[idx] = parse_hex(hex.trim())?;
                }
                "minimum-contrast" => {
                    let v: f32 = value.parse().map_err(|_| {
                        FormatError::parse(format!("invalid minimum-contrast: {value:?}"))
                    })?;
                    p.minimum_contrast = Some(v);
                }
                _ => {}
            }
        }

        p.variant = if relative_luminance(p.background) > 0.5 {
            Variant::Light
        } else {
            Variant::Dark
        };

        Ok(p)
    }
}

fn parse_hex(s: &str) -> Result<Color, FormatError> {
    Color::from_hex(s).map_err(|e| FormatError::parse(format!("invalid color {s:?}: {e}")))
}

fn format_min_contrast(v: f32) -> String {
    if v.is_finite() && (v - v.trunc()).abs() < f32::EPSILON {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Slot;

    const CITRUS_FIELD_DAWN: &str = concat!(
        "background = #f0e5ac\n",
        "foreground = #5a5368\n",
        "cursor-color = #dd7714\n",
        "cursor-text = #2b2820\n",
        "selection-background = #e6cf88\n",
        "selection-foreground = #4b4656\n",
        "palette = 0=#4b4656\n",
        "palette = 1=#b44c37\n",
        "palette = 2=#30803f\n",
        "palette = 3=#8d610c\n",
        "palette = 4=#335da8\n",
        "palette = 5=#8d47ac\n",
        "palette = 6=#1a847f\n",
        "palette = 7=#cdc1ab\n",
        "palette = 8=#6f6a80\n",
        "palette = 9=#c85a44\n",
        "palette = 10=#3a8f4a\n",
        "palette = 11=#9e7013\n",
        "palette = 12=#3f6bb4\n",
        "palette = 13=#9d54ba\n",
        "palette = 14=#219a92\n",
        "palette = 15=#eae0c6\n",
        "minimum-contrast = 3\n",
    );

    #[test]
    fn metadata() {
        assert_eq!(Ghostty.id(), "ghostty");
        assert_eq!(Ghostty.display_name(), "Ghostty");
        assert!(Ghostty.file_extension().is_empty());
    }

    #[test]
    fn export_default_is_byte_exact() {
        assert_eq!(Ghostty.export(&Palette::default()), CITRUS_FIELD_DAWN);
    }

    #[test]
    fn export_ends_with_newline() {
        let out = Ghostty.export(&Palette::default());
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn export_omits_minimum_contrast_when_none() {
        let p = Palette {
            minimum_contrast: None,
            ..Palette::default()
        };
        let out = Ghostty.export(&p);
        assert!(!out.contains("minimum-contrast"));
        assert!(out.ends_with("palette = 15=#eae0c6\n"));
    }

    #[test]
    fn export_minimum_contrast_fractional_keeps_decimal() {
        let p = Palette {
            minimum_contrast: Some(4.5),
            ..Palette::default()
        };
        assert!(Ghostty.export(&p).ends_with("minimum-contrast = 4.5\n"));
    }

    #[test]
    fn export_minimum_contrast_integral_drops_fraction() {
        let p = Palette {
            minimum_contrast: Some(7.0),
            ..Palette::default()
        };
        assert!(Ghostty.export(&p).ends_with("minimum-contrast = 7\n"));
    }

    #[test]
    fn export_non_finite_minimum_contrast_uses_default_float_format() {
        let p = Palette {
            minimum_contrast: Some(f32::INFINITY),
            ..Palette::default()
        };
        assert!(Ghostty.export(&p).ends_with("minimum-contrast = inf\n"));
    }

    #[test]
    fn import_default_export_roundtrips() {
        let original = Palette::default();
        let text = Ghostty.export(&original);
        let back = Ghostty.import(&text).unwrap();

        assert_eq!(back.name, "Imported (Ghostty)");
        assert_eq!(back.author, None);
        for slot in Slot::all() {
            assert_eq!(
                back.get(slot),
                original.get(slot),
                "slot {} mismatch",
                slot.label()
            );
        }
        assert_eq!(back.minimum_contrast, original.minimum_contrast);
        assert_eq!(back.variant, Variant::Light);
        assert_eq!(original.variant, Variant::Light);
    }

    #[test]
    fn import_parses_golden_string() {
        let p = Ghostty.import(CITRUS_FIELD_DAWN).unwrap();
        assert_eq!(p.background, Color::rgb(0xf0, 0xe5, 0xac));
        assert_eq!(p.foreground, Color::rgb(0x5a, 0x53, 0x68));
        assert_eq!(p.cursor, Color::rgb(0xdd, 0x77, 0x14));
        assert_eq!(p.cursor_text, Color::rgb(0x2b, 0x28, 0x20));
        assert_eq!(p.selection_background, Color::rgb(0xe6, 0xcf, 0x88));
        assert_eq!(p.selection_foreground, Color::rgb(0x4b, 0x46, 0x56));
        assert_eq!(p.ansi[0], Color::rgb(0x4b, 0x46, 0x56));
        assert_eq!(p.ansi[10], Color::rgb(0x3a, 0x8f, 0x4a));
        assert_eq!(p.ansi[15], Color::rgb(0xea, 0xe0, 0xc6));
        assert_eq!(p.minimum_contrast, Some(3.0));
        assert_eq!(p.variant, Variant::Light);
    }

    #[test]
    fn import_dark_background_guesses_dark_variant() {
        let text = "background = #101014\nforeground = #d0d0e0\n";
        let p = Ghostty.import(text).unwrap();
        assert_eq!(p.background, Color::rgb(0x10, 0x10, 0x14));
        assert_eq!(p.foreground, Color::rgb(0xd0, 0xd0, 0xe0));
        assert_eq!(p.variant, Variant::Dark);
    }

    #[test]
    fn import_ignores_comments_blanks_and_unknown_keys() {
        let text = concat!(
            "# a comment line\n",
            "\n",
            "   \n",
            "font-family = Fira Code\n",
            "background = #222233\n",
            "  # indented comment\n",
            "palette = 3=#abcdef\n",
        );
        let p = Ghostty.import(text).unwrap();
        assert_eq!(p.background, Color::rgb(0x22, 0x22, 0x33));
        assert_eq!(p.ansi[3], Color::rgb(0xab, 0xcd, 0xef));
    }

    #[test]
    fn import_short_hex_form_accepted() {
        let p = Ghostty.import("background = #abc\n").unwrap();
        assert_eq!(p.background, Color::rgb(0xaa, 0xbb, 0xcc));
    }

    #[test]
    fn import_fractional_minimum_contrast() {
        let p = Ghostty.import("minimum-contrast = 4.5\n").unwrap();
        assert_eq!(p.minimum_contrast, Some(4.5));
    }

    #[test]
    fn import_missing_colors_fall_back_to_defaults() {
        let p = Ghostty.import("foreground = #000000\n").unwrap();
        let base = Palette::default();
        assert_eq!(p.foreground, Color::rgb(0, 0, 0));
        assert_eq!(p.background, base.background);
        assert_eq!(p.ansi, base.ansi);
    }

    #[test]
    fn import_rejects_out_of_range_palette_index() {
        assert!(Ghostty.import("palette = 16=#ffffff\n").is_err());
    }

    #[test]
    fn import_rejects_bad_palette_index() {
        assert!(Ghostty.import("palette = x=#ffffff\n").is_err());
    }

    #[test]
    fn import_rejects_malformed_palette_entry() {
        assert!(Ghostty.import("palette = 0\n").is_err());
    }

    #[test]
    fn import_rejects_invalid_color() {
        assert!(Ghostty.import("background = #zzzzzz\n").is_err());
    }

    #[test]
    fn import_rejects_invalid_minimum_contrast() {
        assert!(Ghostty.import("minimum-contrast = high\n").is_err());
    }
}
