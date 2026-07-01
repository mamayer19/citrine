use crate::color::Color;
use crate::contrast::relative_luminance;
use crate::formats::{FormatError, ThemeFormat};
use crate::palette::{Palette, Variant};

pub struct WezTerm;

impl ThemeFormat for WezTerm {
    fn id(&self) -> &'static str {
        "wezterm"
    }

    fn display_name(&self) -> &'static str {
        "WezTerm"
    }

    fn file_extension(&self) -> &'static str {
        "toml"
    }

    fn export(&self, p: &Palette) -> String {
        let ansi = hex_array(&p.ansi[0..8]);
        let brights = hex_array(&p.ansi[8..16]);

        let mut out = String::new();
        out.push_str("[colors]\n");
        out.push_str(&format!("background = \"{}\"\n", p.background.to_hex()));
        out.push_str(&format!("foreground = \"{}\"\n", p.foreground.to_hex()));
        out.push_str(&format!("cursor_bg = \"{}\"\n", p.cursor.to_hex()));
        out.push_str(&format!("cursor_fg = \"{}\"\n", p.cursor_text.to_hex()));
        out.push_str(&format!("cursor_border = \"{}\"\n", p.cursor.to_hex()));
        out.push_str(&format!(
            "selection_bg = \"{}\"\n",
            p.selection_background.to_hex()
        ));
        out.push_str(&format!(
            "selection_fg = \"{}\"\n",
            p.selection_foreground.to_hex()
        ));
        out.push_str(&format!("ansi = [{ansi}]\n"));
        out.push_str(&format!("brights = [{brights}]\n"));
        out.push('\n');
        out.push_str("[metadata]\n");
        out.push_str(&format!("name = \"{}\"\n", p.name));
        out
    }

    fn import(&self, text: &str) -> Result<Palette, FormatError> {
        let mut p = Palette::default();

        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
                continue;
            }
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let val = val.trim();

            match key {
                "background" => p.background = parse_hex_str(val)?,
                "foreground" => p.foreground = parse_hex_str(val)?,
                "cursor_bg" => p.cursor = parse_hex_str(val)?,
                "cursor_fg" => p.cursor_text = parse_hex_str(val)?,
                "cursor_border" => {}
                "selection_bg" => p.selection_background = parse_hex_str(val)?,
                "selection_fg" => p.selection_foreground = parse_hex_str(val)?,
                "ansi" => {
                    for (i, c) in parse_hex_array(val)?.into_iter().enumerate().take(8) {
                        p.ansi[i] = c;
                    }
                }
                "brights" => {
                    for (i, c) in parse_hex_array(val)?.into_iter().enumerate().take(8) {
                        p.ansi[8 + i] = c;
                    }
                }
                "name" => p.name = val.trim_matches('"').to_string(),
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

fn hex_array(colors: &[Color]) -> String {
    colors
        .iter()
        .map(|c| format!("\"{}\"", c.to_hex()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_hex_str(val: &str) -> Result<Color, FormatError> {
    let s = val.trim().trim_matches('"');
    Color::from_hex(s).map_err(|e| FormatError::parse(e.to_string()))
}

fn parse_hex_array(val: &str) -> Result<Vec<Color>, FormatError> {
    let open = val
        .find('[')
        .ok_or_else(|| FormatError::parse("expected '[' in color array"))?;
    let close = val
        .rfind(']')
        .ok_or_else(|| FormatError::parse("expected ']' in color array"))?;
    if close < open {
        return Err(FormatError::parse("malformed color array"));
    }

    let mut colors = Vec::new();
    for part in val[open + 1..close].split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        colors.push(parse_hex_str(part)?);
    }
    Ok(colors)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CITRUS_FIELD_DAWN: &str = r##"[colors]
background = "#f0e5ac"
foreground = "#5a5368"
cursor_bg = "#dd7714"
cursor_fg = "#2b2820"
cursor_border = "#dd7714"
selection_bg = "#e6cf88"
selection_fg = "#4b4656"
ansi = ["#4b4656", "#b44c37", "#30803f", "#8d610c", "#335da8", "#8d47ac", "#1a847f", "#cdc1ab"]
brights = ["#6f6a80", "#c85a44", "#3a8f4a", "#9e7013", "#3f6bb4", "#9d54ba", "#219a92", "#eae0c6"]

[metadata]
name = "Citrus Field (Dawn)"
"##;

    #[test]
    fn export_matches_golden() {
        assert_eq!(WezTerm.export(&Palette::default()), CITRUS_FIELD_DAWN);
    }

    #[test]
    fn registry_identity() {
        assert_eq!(WezTerm.id(), "wezterm");
        assert_eq!(WezTerm.display_name(), "WezTerm");
        assert_eq!(WezTerm.file_extension(), "toml");
    }

    #[test]
    fn import_round_trips_default() {
        let original = Palette::default();
        let text = WezTerm.export(&original);
        let back = WezTerm.import(&text).expect("import should succeed");
        assert_eq!(back, original);
    }

    #[test]
    fn import_recovers_named_and_ansi_slots() {
        let text = WezTerm.export(&Palette::default());
        let p = WezTerm.import(&text).unwrap();
        assert_eq!(p.name, "Citrus Field (Dawn)");
        assert_eq!(p.background.to_hex(), "#f0e5ac");
        assert_eq!(p.foreground.to_hex(), "#5a5368");
        assert_eq!(p.cursor.to_hex(), "#dd7714");
        assert_eq!(p.cursor_text.to_hex(), "#2b2820");
        assert_eq!(p.selection_background.to_hex(), "#e6cf88");
        assert_eq!(p.selection_foreground.to_hex(), "#4b4656");
        assert_eq!(p.ansi[0].to_hex(), "#4b4656");
        assert_eq!(p.ansi[7].to_hex(), "#cdc1ab");
        assert_eq!(p.ansi[8].to_hex(), "#6f6a80");
        assert_eq!(p.ansi[15].to_hex(), "#eae0c6");
    }

    #[test]
    fn import_guesses_variant_from_background() {
        let light = WezTerm.export(&Palette::default());
        assert_eq!(WezTerm.import(&light).unwrap().variant, Variant::Light);

        let dark_src = "[colors]\nbackground = \"#101010\"\nforeground = \"#eeeeee\"\n";
        assert_eq!(WezTerm.import(dark_src).unwrap().variant, Variant::Dark);
    }

    #[test]
    fn import_rejects_bad_hex() {
        let bad = "[colors]\nbackground = \"#zzzzzz\"\n";
        assert!(matches!(WezTerm.import(bad), Err(FormatError::Parse(_))));
    }

    #[test]
    fn import_ignores_unknown_keys_and_valueless_lines() {
        let text = concat!(
            "[colors]\n",
            "background = \"#202030\"\n",
            "scrollback = \"#010101\"\n",
            "just a comment without equals\n",
            "cursor_border = \"#abcabc\"\n",
            "foreground = \"#e0e0e0\"\n",
        );
        let p = WezTerm.import(text).unwrap();
        assert_eq!(p.background, Color::rgb(0x20, 0x20, 0x30));
        assert_eq!(p.foreground, Color::rgb(0xe0, 0xe0, 0xe0));
    }

    #[test]
    fn import_partial_ansi_array_keeps_defaults_for_missing_slots() {
        let text = "[colors]\nansi = [\"#010101\", \"#020202\"]\n";
        let p = WezTerm.import(text).unwrap();
        assert_eq!(p.ansi[0], Color::rgb(0x01, 0x01, 0x01));
        assert_eq!(p.ansi[1], Color::rgb(0x02, 0x02, 0x02));
        assert_eq!(p.ansi[2], Palette::default().ansi[2]);
    }

    #[test]
    fn import_array_skips_empty_elements() {
        let text = "[colors]\nbrights = [\"#111111\", , \"#222222\"]\n";
        let p = WezTerm.import(text).unwrap();
        assert_eq!(p.ansi[8], Color::rgb(0x11, 0x11, 0x11));
        assert_eq!(p.ansi[9], Color::rgb(0x22, 0x22, 0x22));
    }

    #[test]
    fn import_rejects_array_without_brackets() {
        let text = "[colors]\nansi = \"#ffffff\"\n";
        assert!(matches!(
            WezTerm.import(text).unwrap_err(),
            FormatError::Parse(_)
        ));
    }

    #[test]
    fn import_rejects_array_with_reversed_brackets() {
        let text = "[colors]\nansi = ][\n";
        assert!(matches!(
            WezTerm.import(text).unwrap_err(),
            FormatError::Parse(_)
        ));
    }

    #[test]
    fn import_rejects_bad_color_inside_array() {
        let text = "[colors]\nansi = [\"#zzzzzz\"]\n";
        assert!(matches!(
            WezTerm.import(text).unwrap_err(),
            FormatError::Parse(_)
        ));
    }
}
