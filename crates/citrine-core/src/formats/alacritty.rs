use crate::color::Color;
use crate::contrast::relative_luminance;
use crate::formats::{FormatError, ThemeFormat};
use crate::palette::{Palette, Variant};

pub struct Alacritty;

const NAMES: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

impl ThemeFormat for Alacritty {
    fn id(&self) -> &'static str {
        "alacritty"
    }

    fn display_name(&self) -> &'static str {
        "Alacritty"
    }

    fn file_extension(&self) -> &'static str {
        "toml"
    }

    fn export(&self, p: &Palette) -> String {
        let q = |c: &Color| format!("\"{}\"", c.to_hex());
        let mut out = String::new();

        out.push_str("[colors.primary]\n");
        out.push_str(&format!("background = {}\n", q(&p.background)));
        out.push_str(&format!("foreground = {}\n", q(&p.foreground)));
        out.push('\n');

        out.push_str("[colors.cursor]\n");
        out.push_str(&format!("text = {}\n", q(&p.cursor_text)));
        out.push_str(&format!("cursor = {}\n", q(&p.cursor)));
        out.push('\n');

        out.push_str("[colors.selection]\n");
        out.push_str(&format!("text = {}\n", q(&p.selection_foreground)));
        out.push_str(&format!("background = {}\n", q(&p.selection_background)));
        out.push('\n');

        out.push_str("[colors.normal]\n");
        for (i, name) in NAMES.iter().enumerate() {
            out.push_str(&format!("{name} = {}\n", q(&p.ansi[i])));
        }
        out.push('\n');

        out.push_str("[colors.bright]\n");
        for (i, name) in NAMES.iter().enumerate() {
            out.push_str(&format!("{name} = {}\n", q(&p.ansi[i + 8])));
        }

        out
    }

    fn import(&self, text: &str) -> Result<Palette, FormatError> {
        let mut p = Palette::default();
        let mut section = String::new();

        for raw in text.lines() {
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some(inner) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                section = inner.trim().to_string();
                continue;
            }

            let Some((key, value)) = trimmed.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().trim_matches('"');

            let parse = |v: &str| {
                Color::from_hex(v).map_err(|e| {
                    FormatError::parse(format!("bad color for `{section}.{key}`: {e}"))
                })
            };

            match (section.as_str(), key) {
                ("colors.primary", "background") => p.background = parse(value)?,
                ("colors.primary", "foreground") => p.foreground = parse(value)?,
                ("colors.cursor", "text") => p.cursor_text = parse(value)?,
                ("colors.cursor", "cursor") => p.cursor = parse(value)?,
                ("colors.selection", "text") => p.selection_foreground = parse(value)?,
                ("colors.selection", "background") => p.selection_background = parse(value)?,
                ("colors.normal", name) => {
                    if let Some(i) = NAMES.iter().position(|n| *n == name) {
                        p.ansi[i] = parse(value)?;
                    }
                }
                ("colors.bright", name) => {
                    if let Some(i) = NAMES.iter().position(|n| *n == name) {
                        p.ansi[i + 8] = parse(value)?;
                    }
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
[colors.primary]
background = \"#f0e5ac\"
foreground = \"#5a5368\"

[colors.cursor]
text = \"#2b2820\"
cursor = \"#dd7714\"

[colors.selection]
text = \"#4b4656\"
background = \"#e6cf88\"

[colors.normal]
black = \"#4b4656\"
red = \"#b44c37\"
green = \"#30803f\"
yellow = \"#8d610c\"
blue = \"#335da8\"
magenta = \"#8d47ac\"
cyan = \"#1a847f\"
white = \"#cdc1ab\"

[colors.bright]
black = \"#6f6a80\"
red = \"#c85a44\"
green = \"#3a8f4a\"
yellow = \"#9e7013\"
blue = \"#3f6bb4\"
magenta = \"#9d54ba\"
cyan = \"#219a92\"
white = \"#eae0c6\"
";

    #[test]
    fn export_matches_golden() {
        assert_eq!(Alacritty.export(&Palette::default()), EXPECTED);
    }

    #[test]
    fn roundtrips_default() {
        let p = Palette::default();
        let back = Alacritty.import(&Alacritty.export(&p)).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn roundtrips_modified_colors() {
        let mut p = Palette::default();
        p.set(Slot::Ansi(0), Color::rgb(0x01, 0x02, 0x03));
        p.set(Slot::Ansi(15), Color::rgb(0xfe, 0xdc, 0xba));
        p.set(Slot::Cursor, Color::rgb(0x10, 0x20, 0x30));
        p.set(Slot::SelectionBg, Color::rgb(0x44, 0x55, 0x66));
        let back = Alacritty.import(&Alacritty.export(&p)).unwrap();
        for slot in p.slots() {
            assert_eq!(back.get(slot), p.get(slot), "slot {}", slot.label());
        }
    }

    #[test]
    fn cursor_and_selection_map_correctly() {
        let p = Alacritty.import(EXPECTED).unwrap();
        assert_eq!(p.cursor, Color::rgb(0xdd, 0x77, 0x14));
        assert_eq!(p.cursor_text, Color::rgb(0x2b, 0x28, 0x20));
        assert_eq!(p.selection_background, Color::rgb(0xe6, 0xcf, 0x88));
        assert_eq!(p.selection_foreground, Color::rgb(0x4b, 0x46, 0x56));
    }

    #[test]
    fn import_guesses_variant() {
        let dark = "[colors.primary]\nbackground = \"#0a0a0a\"\n";
        assert_eq!(Alacritty.import(dark).unwrap().variant, Variant::Dark);
    }

    #[test]
    fn import_rejects_bad_color() {
        let bad = "[colors.primary]\nbackground = \"#nothex\"\n";
        assert!(matches!(
            Alacritty.import(bad).unwrap_err(),
            FormatError::Parse(_)
        ));
    }

    #[test]
    fn metadata_is_correct() {
        assert_eq!(Alacritty.id(), "alacritty");
        assert_eq!(Alacritty.display_name(), "Alacritty");
        assert_eq!(Alacritty.file_extension(), "toml");
    }

    #[test]
    fn import_ignores_unknown_sections_keys_and_valueless_lines() {
        let text = concat!(
            "# comment\n",
            "\n",
            "[colors.indexed]\n",
            "0 = \"#123456\"\n",
            "[colors.primary]\n",
            "bright_foreground = \"#010203\"\n",
            "stray line without equals\n",
            "background = \"#222233\"\n",
        );
        let p = Alacritty.import(text).unwrap();
        assert_eq!(p.background, Color::rgb(0x22, 0x22, 0x33));
        assert_eq!(p.foreground, Palette::default().foreground);
    }

    #[test]
    fn import_ignores_unknown_ansi_color_names() {
        let text = concat!(
            "[colors.normal]\n",
            "orange = \"#ff8800\"\n",
            "red = \"#010101\"\n",
            "[colors.bright]\n",
            "pink = \"#ff88ff\"\n",
            "green = \"#020202\"\n",
        );
        let p = Alacritty.import(text).unwrap();
        assert_eq!(p.ansi[1], Color::rgb(0x01, 0x01, 0x01));
        assert_eq!(p.ansi[10], Color::rgb(0x02, 0x02, 0x02));
        assert_eq!(p.ansi[0], Palette::default().ansi[0]);
    }

    #[test]
    fn import_rejects_bad_color_in_normal_table() {
        let text = "[colors.normal]\nred = \"#nothex\"\n";
        assert!(matches!(
            Alacritty.import(text).unwrap_err(),
            FormatError::Parse(_)
        ));
    }

    #[test]
    fn import_rejects_bad_color_in_bright_table() {
        let text = "[colors.bright]\ngreen = \"#zz0000\"\n";
        assert!(matches!(
            Alacritty.import(text).unwrap_err(),
            FormatError::Parse(_)
        ));
    }
}
