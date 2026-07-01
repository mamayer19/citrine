use std::fmt::Write as _;

use crate::color::Color;
use crate::contrast::relative_luminance;
use crate::formats::{FormatError, ThemeFormat};
use crate::palette::{Palette, Variant};

pub struct Foot;

impl ThemeFormat for Foot {
    fn id(&self) -> &'static str {
        "foot"
    }

    fn display_name(&self) -> &'static str {
        "Foot"
    }

    fn file_extension(&self) -> &'static str {
        "ini"
    }

    fn export(&self, p: &Palette) -> String {
        let mut out = String::new();
        out.push_str("[colors]\n");
        let _ = writeln!(out, "background={}", bare_hex(&p.background));
        let _ = writeln!(out, "foreground={}", bare_hex(&p.foreground));

        for (i, c) in p.ansi[..8].iter().enumerate() {
            let _ = writeln!(out, "regular{i}={}", bare_hex(c));
        }
        for (i, c) in p.ansi[8..].iter().enumerate() {
            let _ = writeln!(out, "bright{i}={}", bare_hex(c));
        }

        let _ = writeln!(
            out,
            "selection-foreground={}",
            bare_hex(&p.selection_foreground)
        );
        let _ = writeln!(
            out,
            "selection-background={}",
            bare_hex(&p.selection_background)
        );

        let _ = writeln!(
            out,
            "cursor={} {}",
            bare_hex(&p.background),
            bare_hex(&p.cursor)
        );

        out
    }

    fn import(&self, text: &str) -> Result<Palette, FormatError> {
        let mut p = Palette {
            name: "Imported (Foot)".to_string(),
            author: None,
            ..Palette::default()
        };

        let mut in_colors = false;

        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            if let Some(inner) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                in_colors = inner.trim().eq_ignore_ascii_case("colors");
                continue;
            }

            if !in_colors {
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
                "selection-foreground" => p.selection_foreground = parse_hex(value)?,
                "selection-background" => p.selection_background = parse_hex(value)?,
                "cursor" => {
                    let parts: Vec<&str> = value.split_whitespace().collect();
                    if parts.len() >= 2 {
                        p.cursor_text = parse_hex(parts[0])?;
                        p.cursor = parse_hex(parts[1])?;
                    } else if parts.len() == 1 {
                        p.cursor = parse_hex(parts[0])?;
                    }
                }
                other => {
                    if let Some(rest) = other.strip_prefix("regular") {
                        match rest.parse::<usize>() {
                            Ok(idx) if idx <= 7 => p.ansi[idx] = parse_hex(value)?,
                            Ok(idx) => {
                                return Err(FormatError::parse(format!(
                                    "regular index out of range (0..=7): {idx}"
                                )));
                            }
                            Err(_) => {}
                        }
                    } else if let Some(rest) = other.strip_prefix("bright") {
                        match rest.parse::<usize>() {
                            Ok(idx) if idx <= 7 => p.ansi[8 + idx] = parse_hex(value)?,
                            Ok(idx) => {
                                return Err(FormatError::parse(format!(
                                    "bright index out of range (0..=7): {idx}"
                                )));
                            }
                            Err(_) => {}
                        }
                    }
                }
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

fn bare_hex(c: &Color) -> String {
    format!("{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn parse_hex(s: &str) -> Result<Color, FormatError> {
    Color::from_hex(s).map_err(|e| FormatError::parse(format!("invalid color {s:?}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Slot;

    const CITRUS_FIELD_DAWN: &str = concat!(
        "[colors]\n",
        "background=f0e5ac\n",
        "foreground=5a5368\n",
        "regular0=4b4656\n",
        "regular1=b44c37\n",
        "regular2=30803f\n",
        "regular3=8d610c\n",
        "regular4=335da8\n",
        "regular5=8d47ac\n",
        "regular6=1a847f\n",
        "regular7=cdc1ab\n",
        "bright0=6f6a80\n",
        "bright1=c85a44\n",
        "bright2=3a8f4a\n",
        "bright3=9e7013\n",
        "bright4=3f6bb4\n",
        "bright5=9d54ba\n",
        "bright6=219a92\n",
        "bright7=eae0c6\n",
        "selection-foreground=4b4656\n",
        "selection-background=e6cf88\n",
        "cursor=f0e5ac dd7714\n",
    );

    #[test]
    fn metadata() {
        assert_eq!(Foot.id(), "foot");
        assert_eq!(Foot.display_name(), "Foot");
        assert_eq!(Foot.file_extension(), "ini");
    }

    #[test]
    fn export_matches_golden() {
        assert_eq!(Foot.export(&Palette::default()), CITRUS_FIELD_DAWN);
    }

    #[test]
    fn export_ends_with_newline() {
        assert!(Foot.export(&Palette::default()).ends_with('\n'));
    }

    #[test]
    fn export_uses_bare_hex_without_hash() {
        let out = Foot.export(&Palette::default());
        assert!(!out.contains('#'), "foot hex must be bare (no `#`)");
    }

    #[test]
    fn import_parses_golden_string() {
        let p = Foot.import(CITRUS_FIELD_DAWN).unwrap();
        assert_eq!(p.background, Color::rgb(0xf0, 0xe5, 0xac));
        assert_eq!(p.foreground, Color::rgb(0x5a, 0x53, 0x68));
        assert_eq!(p.selection_foreground, Color::rgb(0x4b, 0x46, 0x56));
        assert_eq!(p.selection_background, Color::rgb(0xe6, 0xcf, 0x88));
        assert_eq!(p.ansi[0], Color::rgb(0x4b, 0x46, 0x56));
        assert_eq!(p.ansi[7], Color::rgb(0xcd, 0xc1, 0xab));
        assert_eq!(p.ansi[8], Color::rgb(0x6f, 0x6a, 0x80));
        assert_eq!(p.ansi[15], Color::rgb(0xea, 0xe0, 0xc6));
        assert_eq!(p.cursor_text, Color::rgb(0xf0, 0xe5, 0xac));
        assert_eq!(p.cursor, Color::rgb(0xdd, 0x77, 0x14));
        assert_eq!(p.variant, Variant::Light);
    }

    #[test]
    fn import_reimports_own_export_for_named_and_ansi_slots() {
        let original = Palette::default();
        let back = Foot.import(&Foot.export(&original)).unwrap();

        assert_eq!(back.name, "Imported (Foot)");
        assert_eq!(back.background, original.background);
        assert_eq!(back.foreground, original.foreground);
        assert_eq!(back.selection_background, original.selection_background);
        assert_eq!(back.selection_foreground, original.selection_foreground);
        assert_eq!(back.cursor, original.cursor);
        assert_eq!(back.ansi, original.ansi);
        assert_eq!(back.variant, Variant::Light);
    }

    #[test]
    fn import_ignores_other_sections_and_unknown_keys() {
        let text = concat!(
            "; a comment\n",
            "[main]\n",
            "font=monospace:size=11\n",
            "background=000000\n",
            "\n",
            "[colors]\n",
            "alpha=0.9\n",
            "background=222233\n",
            "regular3=abcdef\n",
            "flash=ffffff\n",
            "[cursor]\n",
            "blink=yes\n",
        );
        let p = Foot.import(text).unwrap();
        assert_eq!(p.background, Color::rgb(0x22, 0x22, 0x33));
        assert_eq!(p.ansi[3], Color::rgb(0xab, 0xcd, 0xef));
    }

    #[test]
    fn import_single_token_cursor_sets_cursor_color() {
        let p = Foot.import("[colors]\ncursor=dd7714\n").unwrap();
        assert_eq!(p.cursor, Color::rgb(0xdd, 0x77, 0x14));
    }

    #[test]
    fn import_guesses_dark_variant_from_background() {
        let p = Foot.import("[colors]\nbackground=101014\n").unwrap();
        assert_eq!(p.background, Color::rgb(0x10, 0x10, 0x14));
        assert_eq!(p.variant, Variant::Dark);
    }

    #[test]
    fn import_missing_keys_fall_back_to_defaults() {
        let base = Palette::default();
        let p = Foot.import("[colors]\nforeground=000000\n").unwrap();
        assert_eq!(p.foreground, Color::rgb(0, 0, 0));
        assert_eq!(p.background, base.background);
        assert_eq!(p.ansi, base.ansi);
    }

    #[test]
    fn import_rejects_bad_color() {
        assert!(Foot.import("[colors]\nbackground=zzzzzz\n").is_err());
    }

    #[test]
    fn import_rejects_out_of_range_regular_index() {
        assert!(Foot.import("[colors]\nregular8=ffffff\n").is_err());
    }

    #[test]
    fn import_rejects_out_of_range_bright_index() {
        assert!(Foot.import("[colors]\nbright8=ffffff\n").is_err());
    }

    #[test]
    fn import_rejects_bad_ansi_color_value() {
        assert!(Foot.import("[colors]\nregular2=nothex\n").is_err());
    }

    #[test]
    fn import_ignores_prefix_keys_with_nonnumeric_tail() {
        let p = Foot
            .import("[colors]\nbrightness=high\nregularity=none\nregular1=ff0000\n")
            .unwrap();
        assert_eq!(p.ansi[1], Color::rgb(0xff, 0x00, 0x00));
    }

    #[test]
    fn every_named_and_ansi_slot_survives_a_full_edit_roundtrip() {
        let mut p = Palette::default();
        p.set(Slot::Background, Color::rgb(0x20, 0x21, 0x22));
        p.set(Slot::Cursor, Color::rgb(0xab, 0xcd, 0xef));
        p.set(Slot::Ansi(5), Color::rgb(0x12, 0x34, 0x56));
        p.set(Slot::Ansi(12), Color::rgb(0x65, 0x43, 0x21));
        p.set(Slot::SelectionBg, Color::rgb(0x0a, 0x0b, 0x0c));
        let back = Foot.import(&Foot.export(&p)).unwrap();
        for slot in [
            Slot::Background,
            Slot::Foreground,
            Slot::Cursor,
            Slot::SelectionBg,
            Slot::SelectionFg,
            Slot::Ansi(5),
            Slot::Ansi(12),
        ] {
            assert_eq!(back.get(slot), p.get(slot), "slot {}", slot.label());
        }
    }
}
