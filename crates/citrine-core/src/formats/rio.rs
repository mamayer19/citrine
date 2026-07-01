use crate::color::Color;
use crate::contrast::relative_luminance;
use crate::formats::{FormatError, ThemeFormat};
use crate::palette::{Palette, Variant};

pub struct Rio;

const NAMES: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

impl ThemeFormat for Rio {
    fn id(&self) -> &'static str {
        "rio"
    }

    fn display_name(&self) -> &'static str {
        "Rio"
    }

    fn file_extension(&self) -> &'static str {
        "toml"
    }

    fn export(&self, p: &Palette) -> String {
        let q = |c: &Color| format!("\"{}\"", c.to_hex());
        let mut out = String::new();

        out.push_str("[colors]\n");
        out.push_str(&format!("background = {}\n", q(&p.background)));
        out.push_str(&format!("foreground = {}\n", q(&p.foreground)));
        out.push_str(&format!("cursor = {}\n", q(&p.cursor)));
        out.push_str(&format!(
            "selection-background = {}\n",
            q(&p.selection_background)
        ));
        out.push_str(&format!(
            "selection-foreground = {}\n",
            q(&p.selection_foreground)
        ));

        for (i, name) in NAMES.iter().enumerate() {
            out.push_str(&format!("{name} = {}\n", q(&p.ansi[i])));
        }
        for (i, name) in NAMES.iter().enumerate() {
            out.push_str(&format!("light-{name} = {}\n", q(&p.ansi[i + 8])));
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

            if section != "colors" {
                continue;
            }

            let Some((key, value)) = trimmed.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().trim_matches('"');

            let parse = |v: &str| {
                Color::from_hex(v)
                    .map_err(|e| FormatError::parse(format!("bad color for `{key}`: {e}")))
            };

            match key {
                "background" => p.background = parse(value)?,
                "foreground" => p.foreground = parse(value)?,
                "cursor" => p.cursor = parse(value)?,
                "selection-background" => p.selection_background = parse(value)?,
                "selection-foreground" => p.selection_foreground = parse(value)?,
                _ => {
                    if let Some(bright) = key.strip_prefix("light-") {
                        if let Some(i) = NAMES.iter().position(|n| *n == bright) {
                            p.ansi[i + 8] = parse(value)?;
                        }
                    } else if let Some(i) = NAMES.iter().position(|n| *n == key) {
                        p.ansi[i] = parse(value)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Slot;

    const EXPECTED: &str = "\
[colors]
background = \"#f0e5ac\"
foreground = \"#5a5368\"
cursor = \"#dd7714\"
selection-background = \"#e6cf88\"
selection-foreground = \"#4b4656\"
black = \"#4b4656\"
red = \"#b44c37\"
green = \"#30803f\"
yellow = \"#8d610c\"
blue = \"#335da8\"
magenta = \"#8d47ac\"
cyan = \"#1a847f\"
white = \"#cdc1ab\"
light-black = \"#6f6a80\"
light-red = \"#c85a44\"
light-green = \"#3a8f4a\"
light-yellow = \"#9e7013\"
light-blue = \"#3f6bb4\"
light-magenta = \"#9d54ba\"
light-cyan = \"#219a92\"
light-white = \"#eae0c6\"
";

    #[test]
    fn export_matches_golden() {
        assert_eq!(Rio.export(&Palette::default()), EXPECTED);
    }

    #[test]
    fn roundtrips_all_emitted_slots() {
        let p = Palette::default();
        let back = Rio.import(&Rio.export(&p)).unwrap();
        assert_eq!(back.background, p.background);
        assert_eq!(back.foreground, p.foreground);
        assert_eq!(back.cursor, p.cursor);
        assert_eq!(back.selection_background, p.selection_background);
        assert_eq!(back.selection_foreground, p.selection_foreground);
        for n in 0..16 {
            assert_eq!(back.get(Slot::Ansi(n)), p.get(Slot::Ansi(n)), "ANSI {n}");
        }
    }

    #[test]
    fn roundtrips_modified_colors() {
        let mut p = Palette::default();
        p.set(Slot::Ansi(0), Color::rgb(0x01, 0x02, 0x03));
        p.set(Slot::Ansi(15), Color::rgb(0xfe, 0xdc, 0xba));
        p.set(Slot::Cursor, Color::rgb(0x10, 0x20, 0x30));
        p.set(Slot::SelectionBg, Color::rgb(0x44, 0x55, 0x66));
        let back = Rio.import(&Rio.export(&p)).unwrap();
        assert_eq!(back.get(Slot::Ansi(0)), Color::rgb(0x01, 0x02, 0x03));
        assert_eq!(back.get(Slot::Ansi(15)), Color::rgb(0xfe, 0xdc, 0xba));
        assert_eq!(back.cursor, Color::rgb(0x10, 0x20, 0x30));
        assert_eq!(back.selection_background, Color::rgb(0x44, 0x55, 0x66));
    }

    #[test]
    fn import_maps_named_and_ansi_slots() {
        let p = Rio.import(EXPECTED).unwrap();
        assert_eq!(p.background, Color::rgb(0xf0, 0xe5, 0xac));
        assert_eq!(p.foreground, Color::rgb(0x5a, 0x53, 0x68));
        assert_eq!(p.cursor, Color::rgb(0xdd, 0x77, 0x14));
        assert_eq!(p.selection_background, Color::rgb(0xe6, 0xcf, 0x88));
        assert_eq!(p.selection_foreground, Color::rgb(0x4b, 0x46, 0x56));
        assert_eq!(p.ansi[0], Color::rgb(0x4b, 0x46, 0x56));
        assert_eq!(p.ansi[7], Color::rgb(0xcd, 0xc1, 0xab));
        assert_eq!(p.ansi[8], Color::rgb(0x6f, 0x6a, 0x80));
        assert_eq!(p.ansi[15], Color::rgb(0xea, 0xe0, 0xc6));
    }

    #[test]
    fn import_guesses_variant() {
        let dark = "[colors]\nbackground = \"#0a0a0a\"\n";
        assert_eq!(Rio.import(dark).unwrap().variant, Variant::Dark);
        let light = "[colors]\nbackground = \"#f0e5ac\"\n";
        assert_eq!(Rio.import(light).unwrap().variant, Variant::Light);
    }

    #[test]
    fn import_rejects_bad_named_color() {
        let bad = "[colors]\nbackground = \"#nothex\"\n";
        assert!(matches!(
            Rio.import(bad).unwrap_err(),
            FormatError::Parse(_)
        ));
    }

    #[test]
    fn import_rejects_bad_ansi_color() {
        let bad = "[colors]\nlight-green = \"#zz0000\"\n";
        assert!(matches!(
            Rio.import(bad).unwrap_err(),
            FormatError::Parse(_)
        ));
    }

    #[test]
    fn import_ignores_keys_outside_colors_section_and_unknown_keys() {
        let text = concat!(
            "# a comment\n",
            "\n",
            "[general]\n",
            "background = \"#111111\"\n",
            "[colors]\n",
            "orange = \"#ff8800\"\n",
            "light-orange = \"#ff9900\"\n",
            "stray line without equals\n",
            "background = \"#222233\"\n",
        );
        let p = Rio.import(text).unwrap();
        assert_eq!(p.background, Color::rgb(0x22, 0x22, 0x33));
        assert_eq!(p.foreground, Palette::default().foreground);
    }

    #[test]
    fn metadata_is_correct() {
        assert_eq!(Rio.id(), "rio");
        assert_eq!(Rio.display_name(), "Rio");
        assert_eq!(Rio.file_extension(), "toml");
    }
}
