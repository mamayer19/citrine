use crate::color::Color;
use crate::contrast::relative_luminance;
use crate::formats::{FormatError, ThemeFormat};
use crate::palette::{Palette, Variant};

pub struct Konsole;

impl ThemeFormat for Konsole {
    fn id(&self) -> &'static str {
        "konsole"
    }

    fn display_name(&self) -> &'static str {
        "Konsole"
    }

    fn file_extension(&self) -> &'static str {
        "colorscheme"
    }

    fn export(&self, p: &Palette) -> String {
        let mut out = String::new();

        out.push_str("[General]\n");
        out.push_str(&format!("Description={}\n", p.name));
        out.push_str("Opacity=1\n");
        out.push('\n');

        let mut section = |name: &str, c: &Color| {
            out.push_str(&format!("[{name}]\n"));
            out.push_str(&format!("Color={},{},{}\n", c.r(), c.g(), c.b()));
        };

        section("Background", &p.background);
        section("Foreground", &p.foreground);
        for i in 0..8 {
            section(&format!("Color{i}"), &p.ansi[i]);
        }
        for i in 0..8 {
            section(&format!("Color{i}Intense"), &p.ansi[i + 8]);
        }
        section("BackgroundIntense", &p.background);
        section("ForegroundIntense", &p.foreground);

        out
    }

    fn import(&self, text: &str) -> Result<Palette, FormatError> {
        let mut p = Palette::default();
        let mut section = String::new();

        for raw in text.lines() {
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }

            if let Some(name) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                section = name.to_string();
                continue;
            }

            let Some((key, value)) = trimmed.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();

            if section == "General" && key == "Description" {
                p.name = value.to_string();
                continue;
            }

            if key != "Color" {
                continue;
            }

            match section.as_str() {
                "Background" => p.background = parse_triple(value)?,
                "Foreground" => p.foreground = parse_triple(value)?,
                "BackgroundIntense" | "ForegroundIntense" => {}
                other => {
                    if let Some(idx) = ansi_index(other) {
                        p.ansi[idx] = parse_triple(value)?;
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

fn ansi_index(section: &str) -> Option<usize> {
    let rest = section.strip_prefix("Color")?;
    if let Some(n) = rest.strip_suffix("Intense") {
        let idx: usize = n.parse().ok()?;
        (idx <= 7).then_some(idx + 8)
    } else {
        let idx: usize = rest.parse().ok()?;
        (idx <= 7).then_some(idx)
    }
}

fn parse_triple(value: &str) -> Result<Color, FormatError> {
    let parts: Vec<&str> = value.split(',').map(str::trim).collect();
    if parts.len() != 3 {
        return Err(FormatError::parse(format!(
            "expected `R,G,B` triple, got `{value}`"
        )));
    }
    let channel = |s: &str| {
        s.parse::<u8>()
            .map_err(|_| FormatError::parse(format!("invalid color channel `{s}` in `{value}`")))
    };
    Ok(Color::from_rgb(
        channel(parts[0])?,
        channel(parts[1])?,
        channel(parts[2])?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Slot;

    const EXPECTED: &str = "\
[General]
Description=Citrus Field (Dawn)
Opacity=1

[Background]
Color=240,229,172
[Foreground]
Color=90,83,104
[Color0]
Color=75,70,86
[Color1]
Color=180,76,55
[Color2]
Color=48,128,63
[Color3]
Color=141,97,12
[Color4]
Color=51,93,168
[Color5]
Color=141,71,172
[Color6]
Color=26,132,127
[Color7]
Color=205,193,171
[Color0Intense]
Color=111,106,128
[Color1Intense]
Color=200,90,68
[Color2Intense]
Color=58,143,74
[Color3Intense]
Color=158,112,19
[Color4Intense]
Color=63,107,180
[Color5Intense]
Color=157,84,186
[Color6Intense]
Color=33,154,146
[Color7Intense]
Color=234,224,198
[BackgroundIntense]
Color=240,229,172
[ForegroundIntense]
Color=90,83,104
";

    #[test]
    fn export_matches_golden() {
        assert_eq!(Konsole.export(&Palette::default()), EXPECTED);
    }

    #[test]
    fn export_ends_with_newline() {
        assert!(Konsole.export(&Palette::default()).ends_with('\n'));
    }

    #[test]
    fn roundtrips_default() {
        let p = Palette::default();
        let back = Konsole.import(&Konsole.export(&p)).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn roundtrips_background_foreground_and_ansi_slots() {
        let mut p = Palette::default();
        p.set(Slot::Background, Color::rgb(0x10, 0x20, 0x30));
        p.set(Slot::Foreground, Color::rgb(0xa0, 0xb0, 0xc0));
        p.set(Slot::Ansi(3), Color::rgb(0x12, 0x34, 0x56));
        p.set(Slot::Ansi(11), Color::rgb(0x65, 0x43, 0x21));
        let back = Konsole.import(&Konsole.export(&p)).unwrap();
        assert_eq!(back.background, p.background);
        assert_eq!(back.foreground, p.foreground);
        for n in 0u8..16 {
            assert_eq!(back.ansi[n as usize], p.ansi[n as usize], "ansi {n}");
        }
    }

    #[test]
    fn import_maps_intense_groups_to_bright_ansi() {
        let text = "\
[Color0]
Color=1,2,3
[Color0Intense]
Color=4,5,6
[Color7Intense]
Color=7,8,9
";
        let p = Konsole.import(text).unwrap();
        assert_eq!(p.ansi[0], Color::rgb(1, 2, 3));
        assert_eq!(p.ansi[8], Color::rgb(4, 5, 6));
        assert_eq!(p.ansi[15], Color::rgb(7, 8, 9));
    }

    #[test]
    fn import_restores_name_from_description() {
        let text = "\
[General]
Description=My Scheme
Opacity=1
[Background]
Color=0,0,0
";
        assert_eq!(Konsole.import(text).unwrap().name, "My Scheme");
    }

    #[test]
    fn import_leaves_selection_and_cursor_at_defaults() {
        let text = "[Background]\nColor=1,1,1\n";
        let p = Konsole.import(text).unwrap();
        let d = Palette::default();
        assert_eq!(p.selection_background, d.selection_background);
        assert_eq!(p.selection_foreground, d.selection_foreground);
        assert_eq!(p.cursor, d.cursor);
        assert_eq!(p.cursor_text, d.cursor_text);
    }

    #[test]
    fn import_guesses_variant_from_background() {
        let dark = "[Background]\nColor=16,16,20\n";
        assert_eq!(Konsole.import(dark).unwrap().variant, Variant::Dark);
        let light = "[Background]\nColor=240,229,172\n";
        assert_eq!(Konsole.import(light).unwrap().variant, Variant::Light);
    }

    #[test]
    fn import_ignores_unknown_and_intense_bgfg_groups() {
        let text = "\
[BackgroundIntense]
Color=1,2,3
[ForegroundIntense]
Color=4,5,6
[Color0Faint]
Color=7,8,9
[Wallpaper]
Path=/tmp/x.png
";
        let p = Konsole.import(text).unwrap();
        assert_eq!(p, Palette::default());
    }

    #[test]
    fn import_rejects_non_numeric_channel() {
        let err = Konsole.import("[Background]\nColor=1,zz,3\n").unwrap_err();
        assert!(matches!(err, FormatError::Parse(_)));
    }

    #[test]
    fn import_rejects_wrong_arity_triple() {
        let err = Konsole.import("[Background]\nColor=1,2\n").unwrap_err();
        assert!(matches!(err, FormatError::Parse(_)));
    }

    #[test]
    fn import_rejects_out_of_range_channel() {
        let err = Konsole.import("[Foreground]\nColor=256,0,0\n").unwrap_err();
        assert!(matches!(err, FormatError::Parse(_)));
    }

    #[test]
    fn metadata_is_correct() {
        assert_eq!(Konsole.id(), "konsole");
        assert_eq!(Konsole.display_name(), "Konsole");
        assert_eq!(Konsole.file_extension(), "colorscheme");
    }
}
