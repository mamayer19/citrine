use crate::color::Color;
use crate::contrast::relative_luminance;
use crate::formats::{FormatError, ThemeFormat};
use crate::palette::{Palette, Variant};

pub struct Base16;

fn base_colors(p: &Palette) -> [Color; 16] {
    [
        p.background,
        p.ansi[0],
        p.selection_background,
        p.ansi[8],
        p.ansi[7],
        p.foreground,
        p.ansi[15],
        p.ansi[15],
        p.ansi[1],
        p.ansi[9],
        p.ansi[3],
        p.ansi[2],
        p.ansi[6],
        p.ansi[4],
        p.ansi[5],
        p.ansi[9],
    ]
}

fn variant_token(v: Variant) -> &'static str {
    match v {
        Variant::Light => "light",
        Variant::Dark => "dark",
    }
}

fn yaml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    let inner = if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        &t[1..t.len() - 1]
    } else {
        return t.to_string();
    };
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn guess_variant(background: Color) -> Variant {
    if relative_luminance(background) > 0.5 {
        Variant::Light
    } else {
        Variant::Dark
    }
}

impl ThemeFormat for Base16 {
    fn id(&self) -> &'static str {
        "base16"
    }

    fn display_name(&self) -> &'static str {
        "base16"
    }

    fn file_extension(&self) -> &'static str {
        "yaml"
    }

    fn export(&self, p: &Palette) -> String {
        let author = p.author.as_deref().unwrap_or("Citrine");
        let mut out = String::new();
        out.push_str("system: \"base16\"\n");
        out.push_str(&format!("name: \"{}\"\n", yaml_escape(&p.name)));
        out.push_str(&format!("author: \"{}\"\n", yaml_escape(author)));
        out.push_str(&format!("variant: \"{}\"\n", variant_token(p.variant)));
        out.push_str("palette:\n");
        for (i, color) in base_colors(p).iter().enumerate() {
            let bare = &color.to_hex()[1..];
            out.push_str(&format!("  base{i:02X}: \"{bare}\"\n"));
        }
        out
    }

    fn import(&self, text: &str) -> Result<Palette, FormatError> {
        let mut bases: [Option<Color>; 16] = [None; 16];
        let mut name: Option<String> = None;
        let mut author: Option<String> = None;
        let mut variant: Option<Variant> = None;
        let mut saw_base = false;

        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, val)) = line.split_once(':') else {
                continue;
            };
            let key = key.trim();
            let val = unquote(val);

            if let Some(rest) = key.strip_prefix("base") {
                if let Ok(idx) = u8::from_str_radix(rest, 16) {
                    if (idx as usize) < 16 && !val.is_empty() {
                        let color = Color::from_hex(&val)
                            .map_err(|e| FormatError::parse(format!("base{rest}: {e}")))?;
                        bases[idx as usize] = Some(color);
                        saw_base = true;
                    }
                }
                continue;
            }

            match key {
                "name" if !val.is_empty() => name = Some(val),
                "author" if !val.is_empty() => author = Some(val),
                "variant" => {
                    variant = match val.to_ascii_lowercase().as_str() {
                        "light" => Some(Variant::Light),
                        "dark" => Some(Variant::Dark),
                        _ => variant,
                    }
                }
                _ => {}
            }
        }

        if !saw_base {
            return Err(FormatError::parse(
                "no base16 palette entries (base00..base0F) found",
            ));
        }

        let mut p = Palette::default();
        if let Some(c) = bases[0x00] {
            p.background = c;
            p.ansi[0] = c;
        }
        if let Some(c) = bases[0x05] {
            p.foreground = c;
        }
        if let Some(c) = bases[0x08] {
            p.ansi[1] = c;
        }
        if let Some(c) = bases[0x0B] {
            p.ansi[2] = c;
        }
        if let Some(c) = bases[0x0A] {
            p.ansi[3] = c;
        }
        if let Some(c) = bases[0x0D] {
            p.ansi[4] = c;
        }
        if let Some(c) = bases[0x0E] {
            p.ansi[5] = c;
        }
        if let Some(c) = bases[0x0C] {
            p.ansi[6] = c;
        }
        if let Some(c) = bases[0x03] {
            p.ansi[8] = c;
        }

        if let Some(n) = name {
            p.name = n;
        }
        p.author = author;
        p.variant = variant.unwrap_or_else(|| guess_variant(p.background));

        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOLDEN: &str = concat!(
        "system: \"base16\"\n",
        "name: \"Citrus Field (Dawn)\"\n",
        "author: \"Citrine\"\n",
        "variant: \"light\"\n",
        "palette:\n",
        "  base00: \"f0e5ac\"\n",
        "  base01: \"4b4656\"\n",
        "  base02: \"e6cf88\"\n",
        "  base03: \"6f6a80\"\n",
        "  base04: \"cdc1ab\"\n",
        "  base05: \"5a5368\"\n",
        "  base06: \"eae0c6\"\n",
        "  base07: \"eae0c6\"\n",
        "  base08: \"b44c37\"\n",
        "  base09: \"c85a44\"\n",
        "  base0A: \"8d610c\"\n",
        "  base0B: \"30803f\"\n",
        "  base0C: \"1a847f\"\n",
        "  base0D: \"335da8\"\n",
        "  base0E: \"8d47ac\"\n",
        "  base0F: \"c85a44\"\n",
    );

    #[test]
    fn golden_default_export() {
        assert_eq!(Base16.export(&Palette::default()), GOLDEN);
    }

    #[test]
    fn export_values_are_bare_lowercase_hex() {
        let out = Base16.export(&Palette::default());
        assert!(!out.contains('#'), "base16 values must not carry '#'");
        assert!(out.contains("  base00: \"f0e5ac\"\n"));
    }

    #[test]
    fn round_trip_preserves_mapped_slots() {
        let p = Palette::default();
        let text = Base16.export(&p);
        let back = Base16.import(&text).expect("import own export");

        assert_eq!(back.background, p.background);
        assert_eq!(back.foreground, p.foreground);
        assert_eq!(back.variant, p.variant);
        assert_eq!(back.name, p.name);
        assert_eq!(back.ansi[1], p.ansi[1]);
        assert_eq!(back.ansi[2], p.ansi[2]);
        assert_eq!(back.ansi[3], p.ansi[3]);
        assert_eq!(back.ansi[4], p.ansi[4]);
        assert_eq!(back.ansi[5], p.ansi[5]);
        assert_eq!(back.ansi[6], p.ansi[6]);
        assert_eq!(back.ansi[8], p.ansi[8]);
        assert_eq!(back.ansi[0], p.background);
    }

    #[test]
    fn import_rejects_input_without_base_entries() {
        let err = Base16.import("hello: world\n").unwrap_err();
        assert!(matches!(err, FormatError::Parse(_)));
    }

    #[test]
    fn import_reports_bad_hex_as_parse_error() {
        let text = "palette:\n  base00: \"nothex\"\n";
        let err = Base16.import(text).unwrap_err();
        assert!(matches!(err, FormatError::Parse(_)));
    }

    #[test]
    fn import_guesses_dark_variant_from_background() {
        let text = "palette:\n  base00: \"1a1a1a\"\n  base05: \"eeeeee\"\n";
        let p = Base16.import(text).expect("import minimal dark scheme");
        assert_eq!(p.variant, Variant::Dark);
        assert_eq!(p.background, Color::rgb(0x1a, 0x1a, 0x1a));
        assert_eq!(p.foreground, Color::rgb(0xee, 0xee, 0xee));
    }

    #[test]
    fn import_honors_explicit_variant_and_bare_hex() {
        let text = "variant: dark\npalette:\n  base00: f0e5ac\n";
        let p = Base16.import(text).expect("import bare-hex scheme");
        assert_eq!(p.variant, Variant::Dark);
        assert_eq!(p.background, Color::rgb(0xf0, 0xe5, 0xac));
    }

    #[test]
    fn import_reads_name_and_author_header() {
        let text = concat!(
            "name: \"Nord\"\n",
            "author: \"arcticicestudio\"\n",
            "variant: \"dark\"\n",
            "palette:\n",
            "  base00: \"2e3440\"\n",
        );
        let p = Base16.import(text).expect("import named scheme");
        assert_eq!(p.name, "Nord");
        assert_eq!(p.author.as_deref(), Some("arcticicestudio"));
    }

    #[test]
    fn metadata_is_correct() {
        assert_eq!(Base16.id(), "base16");
        assert_eq!(Base16.display_name(), "base16");
        assert_eq!(Base16.file_extension(), "yaml");
    }

    #[test]
    fn export_uses_supplied_author() {
        let p = Palette {
            author: Some("Ada".to_string()),
            ..Palette::default()
        };
        assert!(Base16.export(&p).contains("author: \"Ada\"\n"));
    }

    #[test]
    fn export_escapes_quotes_and_backslashes_in_name() {
        let p = Palette {
            name: r#"a"b\c"#.to_string(),
            ..Palette::default()
        };
        assert!(Base16.export(&p).contains(r#"name: "a\"b\\c""#));
    }

    #[test]
    fn import_ignores_comments_blanks_and_unknown_top_level_keys() {
        let text = concat!(
            "# scheme comment\n",
            "\n",
            "   \n",
            "slug: nord\n",
            "palette:\n",
            "  # inner comment\n",
            "  base00: \"1a1a1a\"\n",
            "  base05: \"eeeeee\"\n",
        );
        let p = Base16.import(text).expect("import despite noise");
        assert_eq!(p.background, Color::rgb(0x1a, 0x1a, 0x1a));
        assert_eq!(p.foreground, Color::rgb(0xee, 0xee, 0xee));
    }

    #[test]
    fn import_unknown_variant_token_falls_back_to_luminance_guess() {
        let text = "variant: \"sepia\"\npalette:\n  base00: \"f0e5ac\"\n";
        let p = Base16.import(text).expect("import with bogus variant");
        assert_eq!(p.variant, Variant::Light);
    }

    #[test]
    fn import_ignores_empty_name_and_author_values() {
        let text = concat!(
            "name: \"\"\n",
            "author: \"\"\n",
            "palette:\n",
            "  base00: \"101010\"\n",
        );
        let p = Base16
            .import(text)
            .expect("import with empty header values");
        assert_eq!(p.name, Palette::default().name);
        assert_eq!(p.author, None);
    }

    #[test]
    fn import_skips_out_of_range_and_nonhex_base_indices() {
        let text = "palette:\n  base10: \"ffffff\"\n  basezz: \"000000\"\n";
        assert!(matches!(
            Base16.import(text).unwrap_err(),
            FormatError::Parse(_)
        ));
    }

    #[test]
    fn import_treats_empty_base_value_as_absent() {
        let text = "palette:\n  base00: \"\"\n";
        assert!(matches!(
            Base16.import(text).unwrap_err(),
            FormatError::Parse(_)
        ));
    }

    #[test]
    fn import_unescapes_quoted_name() {
        let text = concat!("name: \"a\\\"b\"\n", "palette:\n", "  base00: \"111111\"\n",);
        let p = Base16.import(text).expect("import escaped name");
        assert_eq!(p.name, "a\"b");
    }

    #[test]
    fn import_tolerates_trailing_backslash_in_quoted_scalar() {
        let text = concat!("name: \"ab\\\"\n", "palette:\n", "  base00: \"111111\"\n",);
        let p = Base16.import(text).expect("import trailing-backslash name");
        assert_eq!(p.name, "ab");
    }

    #[test]
    fn import_bare_hex_round_trips_recoverable_slots() {
        let text = concat!(
            "variant: dark\n",
            "palette:\n",
            "  base00: 101018\n",
            "  base05: eeeeee\n",
            "  base08: ff5555\n",
            "  base0B: 55ff55\n",
        );
        let p = Base16.import(text).expect("import bare-hex scheme");
        assert_eq!(p.background, Color::rgb(0x10, 0x10, 0x18));
        assert_eq!(p.ansi[0], Color::rgb(0x10, 0x10, 0x18));
        assert_eq!(p.foreground, Color::rgb(0xee, 0xee, 0xee));
        assert_eq!(p.ansi[1], Color::rgb(0xff, 0x55, 0x55));
        assert_eq!(p.ansi[2], Color::rgb(0x55, 0xff, 0x55));
        assert_eq!(p.variant, Variant::Dark);
    }
}
