use crate::color::Color;
use crate::contrast::relative_luminance;
use crate::formats::{FormatError, ThemeFormat};
use crate::palette::{Palette, Variant};

pub struct Kitty;

impl ThemeFormat for Kitty {
    fn id(&self) -> &'static str {
        "kitty"
    }

    fn display_name(&self) -> &'static str {
        "Kitty"
    }

    fn file_extension(&self) -> &'static str {
        "conf"
    }

    fn export(&self, p: &Palette) -> String {
        let mut out = String::new();
        let mut line = |key: &str, color: &Color| {
            out.push_str(key);
            out.push(' ');
            out.push_str(&color.to_hex());
            out.push('\n');
        };

        line("background", &p.background);
        line("foreground", &p.foreground);
        line("cursor", &p.cursor);
        line("cursor_text_color", &p.cursor_text);
        line("selection_background", &p.selection_background);
        line("selection_foreground", &p.selection_foreground);
        for (i, color) in p.ansi.iter().enumerate() {
            line(&format!("color{i}"), color);
        }

        out
    }

    fn import(&self, text: &str) -> Result<Palette, FormatError> {
        let mut p = Palette::default();

        for raw in text.lines() {
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let mut parts = trimmed.split_whitespace();
            let Some(key) = parts.next() else {
                continue;
            };
            let Some(value) = parts.next() else {
                continue;
            };

            let parse = |v: &str| {
                Color::from_hex(v)
                    .map_err(|e| FormatError::parse(format!("bad color for `{key}`: {e}")))
            };

            match key {
                "background" => p.background = parse(value)?,
                "foreground" => p.foreground = parse(value)?,
                "cursor" => p.cursor = parse(value)?,
                "cursor_text_color" => p.cursor_text = parse(value)?,
                "selection_background" => p.selection_background = parse(value)?,
                "selection_foreground" => p.selection_foreground = parse(value)?,
                other if other.starts_with("color") => {
                    let idx: usize = other[5..].parse().map_err(|_| {
                        FormatError::parse(format!("invalid ansi index in `{other}`"))
                    })?;
                    if idx > 15 {
                        return Err(FormatError::parse(format!(
                            "ansi index out of range: {idx}"
                        )));
                    }
                    p.ansi[idx] = parse(value)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Slot;

    const EXPECTED: &str = "\
background #f0e5ac
foreground #5a5368
cursor #dd7714
cursor_text_color #2b2820
selection_background #e6cf88
selection_foreground #4b4656
color0 #4b4656
color1 #b44c37
color2 #30803f
color3 #8d610c
color4 #335da8
color5 #8d47ac
color6 #1a847f
color7 #cdc1ab
color8 #6f6a80
color9 #c85a44
color10 #3a8f4a
color11 #9e7013
color12 #3f6bb4
color13 #9d54ba
color14 #219a92
color15 #eae0c6
";

    #[test]
    fn export_matches_golden() {
        assert_eq!(Kitty.export(&Palette::default()), EXPECTED);
    }

    #[test]
    fn export_ends_with_newline() {
        assert!(Kitty.export(&Palette::default()).ends_with('\n'));
    }

    #[test]
    fn roundtrips_default() {
        let p = Palette::default();
        let back = Kitty.import(&Kitty.export(&p)).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn roundtrips_modified_colors() {
        let mut p = Palette::default();
        p.set(Slot::Ansi(3), Color::rgb(0x12, 0x34, 0x56));
        p.set(Slot::Cursor, Color::rgb(0xab, 0xcd, 0xef));
        p.set(Slot::SelectionFg, Color::rgb(0x00, 0x11, 0x22));
        let back = Kitty.import(&Kitty.export(&p)).unwrap();
        for slot in p.slots() {
            assert_eq!(back.get(slot), p.get(slot), "slot {}", slot.label());
        }
    }

    #[test]
    fn import_guesses_variant_from_background() {
        let dark = "background #101014\nforeground #eeeeee\n";
        assert_eq!(Kitty.import(dark).unwrap().variant, Variant::Dark);

        let light = "background #f0e5ac\nforeground #5a5368\n";
        assert_eq!(Kitty.import(light).unwrap().variant, Variant::Light);
    }

    #[test]
    fn import_ignores_comments_and_unknown_keys() {
        let text = "\
# a comment line
background #ffffff
font_size 12.0
color1 #ff0000
";
        let p = Kitty.import(text).unwrap();
        assert_eq!(p.background, Color::rgb(0xff, 0xff, 0xff));
        assert_eq!(p.ansi[1], Color::rgb(0xff, 0x00, 0x00));
    }

    #[test]
    fn import_rejects_bad_color() {
        let err = Kitty.import("background #zzzzzz\n").unwrap_err();
        assert!(matches!(err, FormatError::Parse(_)));
    }

    #[test]
    fn import_rejects_out_of_range_ansi() {
        let err = Kitty.import("color16 #ffffff\n").unwrap_err();
        assert!(matches!(err, FormatError::Parse(_)));
    }

    #[test]
    fn metadata_is_correct() {
        assert_eq!(Kitty.id(), "kitty");
        assert_eq!(Kitty.display_name(), "Kitty");
        assert_eq!(Kitty.file_extension(), "conf");
    }

    #[test]
    fn import_ignores_line_with_key_but_no_value() {
        let text = "background\ncolor1 #ff0000\n";
        let p = Kitty.import(text).unwrap();
        assert_eq!(p.background, Palette::default().background);
        assert_eq!(p.ansi[1], Color::rgb(0xff, 0x00, 0x00));
    }

    #[test]
    fn import_rejects_nonnumeric_color_index() {
        let err = Kitty.import("colorx #ffffff\n").unwrap_err();
        assert!(matches!(err, FormatError::Parse(_)));
    }

    #[test]
    fn import_rejects_bare_color_key_with_empty_index() {
        let err = Kitty.import("color #ffffff\n").unwrap_err();
        assert!(matches!(err, FormatError::Parse(_)));
    }
}
