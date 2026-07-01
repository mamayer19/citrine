use crate::color::Color;
use crate::palette::{Palette, Variant};

pub const REFERENCE_NAMES: &[&str] = &[
    "Rose Pine",
    "Rose Pine Moon",
    "Rose Pine Dawn",
    "Catppuccin Latte",
    "Catppuccin Frappe",
    "Catppuccin Macchiato",
    "Catppuccin Mocha",
    "Gruvbox Dark",
    "Gruvbox Light",
];

fn hex(s: &str) -> Color {
    Color::from_hex(s).expect("reference palette hex literal must be valid")
}

#[allow(clippy::too_many_arguments)]
fn make(
    name: &str,
    variant: Variant,
    background: &str,
    foreground: &str,
    cursor: &str,
    cursor_text: &str,
    selection_background: &str,
    selection_foreground: &str,
    ansi: [&str; 16],
) -> Palette {
    Palette {
        name: name.to_string(),
        author: None,
        variant,
        background: hex(background),
        foreground: hex(foreground),
        cursor: hex(cursor),
        cursor_text: hex(cursor_text),
        selection_background: hex(selection_background),
        selection_foreground: hex(selection_foreground),
        ansi: ansi.map(hex),
        minimum_contrast: None,
    }
}

pub fn references() -> Vec<Palette> {
    vec![
        make(
            "Rose Pine",
            Variant::Dark,
            "#191724",
            "#e0def4",
            "#e0def4",
            "#191724",
            "#403d52",
            "#e0def4",
            [
                "#26233a", "#eb6f92", "#31748f", "#f6c177", "#9ccfd8", "#c4a7e7", "#ebbcba",
                "#e0def4", "#6e6a86", "#eb6f92", "#31748f", "#f6c177", "#9ccfd8", "#c4a7e7",
                "#ebbcba", "#e0def4",
            ],
        ),
        make(
            "Rose Pine Moon",
            Variant::Dark,
            "#232136",
            "#e0def4",
            "#e0def4",
            "#232136",
            "#44415a",
            "#e0def4",
            [
                "#393552", "#eb6f92", "#3e8fb0", "#f6c177", "#9ccfd8", "#c4a7e7", "#ea9a97",
                "#e0def4", "#6e6a86", "#eb6f92", "#3e8fb0", "#f6c177", "#9ccfd8", "#c4a7e7",
                "#ea9a97", "#e0def4",
            ],
        ),
        make(
            "Rose Pine Dawn",
            Variant::Light,
            "#faf4ed",
            "#575279",
            "#575279",
            "#faf4ed",
            "#dfdad9",
            "#575279",
            [
                "#f2e9e1", "#b4637a", "#286983", "#ea9d34", "#56949f", "#907aa9", "#d7827e",
                "#575279", "#9893a5", "#b4637a", "#286983", "#ea9d34", "#56949f", "#907aa9",
                "#d7827e", "#575279",
            ],
        ),
        make(
            "Catppuccin Latte",
            Variant::Light,
            "#eff1f5",
            "#4c4f69",
            "#dc8a78",
            "#eff1f5",
            "#acb0be",
            "#4c4f69",
            [
                "#5c5f77", "#d20f39", "#40a02b", "#df8e1d", "#1e66f5", "#ea76cb", "#179299",
                "#acb0be", "#6c6f85", "#de293e", "#49af3d", "#eea02d", "#456eff", "#fe85d8",
                "#2d9fa8", "#bcc0cc",
            ],
        ),
        make(
            "Catppuccin Frappe",
            Variant::Dark,
            "#303446",
            "#c6d0f5",
            "#f2d5cf",
            "#303446",
            "#626880",
            "#c6d0f5",
            [
                "#51576d", "#e78284", "#a6d189", "#e5c890", "#8caaee", "#f4b8e4", "#81c8be",
                "#a5adce", "#626880", "#e67172", "#8ec772", "#d9ba73", "#7b9ef0", "#f2a4db",
                "#5abfb5", "#b5bfe2",
            ],
        ),
        make(
            "Catppuccin Macchiato",
            Variant::Dark,
            "#24273a",
            "#cad3f5",
            "#f4dbd6",
            "#24273a",
            "#5b6078",
            "#cad3f5",
            [
                "#494d64", "#ed8796", "#a6da95", "#eed49f", "#8aadf4", "#f5bde6", "#8bd5ca",
                "#a5adcb", "#5b6078", "#ec7486", "#8ccf7f", "#e1c682", "#78a1f6", "#f2a9dd",
                "#63cbc0", "#b8c0e0",
            ],
        ),
        make(
            "Catppuccin Mocha",
            Variant::Dark,
            "#1e1e2e",
            "#cdd6f4",
            "#f5e0dc",
            "#1e1e2e",
            "#585b70",
            "#cdd6f4",
            [
                "#45475a", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5",
                "#a6adc8", "#585b70", "#f37799", "#89d88b", "#ebd391", "#74a8fc", "#f2aede",
                "#6bd7ca", "#bac2de",
            ],
        ),
        make(
            "Gruvbox Dark",
            Variant::Dark,
            "#282828",
            "#ebdbb2",
            "#ebdbb2",
            "#282828",
            "#665c54",
            "#ebdbb2",
            [
                "#282828", "#cc241d", "#98971a", "#d79921", "#458588", "#b16286", "#689d6a",
                "#a89984", "#928374", "#fb4934", "#b8bb26", "#fabd2f", "#83a598", "#d3869b",
                "#8ec07c", "#ebdbb2",
            ],
        ),
        make(
            "Gruvbox Light",
            Variant::Light,
            "#fbf1c7",
            "#3c3836",
            "#3c3836",
            "#fbf1c7",
            "#3c3836",
            "#fbf1c7",
            [
                "#fbf1c7", "#cc241d", "#98971a", "#d79921", "#458588", "#b16286", "#689d6a",
                "#7c6f64", "#928374", "#9d0006", "#79740e", "#b57614", "#076678", "#8f3f71",
                "#427b58", "#3c3836",
            ],
        ),
    ]
}

pub fn reference_by_name(name: &str) -> Option<Palette> {
    references().into_iter().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(name: &str) -> Palette {
        reference_by_name(name).unwrap_or_else(|| panic!("missing reference palette: {name}"))
    }

    #[test]
    fn ships_nine_references() {
        assert_eq!(references().len(), 9);
    }

    #[test]
    fn names_match_declared_list_in_order() {
        let names: Vec<String> = references().into_iter().map(|p| p.name).collect();
        assert_eq!(names, REFERENCE_NAMES);
        assert_eq!(REFERENCE_NAMES.len(), 9);
    }

    #[test]
    fn exact_transcription_spot_checks() {
        assert_eq!(
            get("Rose Pine Dawn").background,
            Color::from_hex("#faf4ed").unwrap()
        );
        assert_eq!(
            get("Catppuccin Latte").ansi[1],
            Color::from_hex("#d20f39").unwrap()
        );
        assert!(reference_by_name("Gruvbox Dark").is_some());
    }

    #[test]
    fn more_transcription_spot_checks() {
        let rp = get("Rose Pine");
        assert_eq!(rp.background.to_hex(), "#191724");
        assert_eq!(rp.foreground.to_hex(), "#e0def4");
        assert_eq!(rp.cursor.to_hex(), "#e0def4");
        assert_eq!(rp.cursor_text.to_hex(), "#191724");
        assert_eq!(rp.selection_background.to_hex(), "#403d52");
        assert_eq!(rp.selection_foreground.to_hex(), "#e0def4");
        assert_eq!(rp.ansi[0].to_hex(), "#26233a");
        assert_eq!(rp.ansi[15].to_hex(), "#e0def4");

        assert_eq!(get("Catppuccin Latte").cursor.to_hex(), "#dc8a78");
        assert_eq!(get("Catppuccin Mocha").background.to_hex(), "#1e1e2e");
        assert_eq!(get("Catppuccin Mocha").ansi[9].to_hex(), "#f37799");

        let gl = get("Gruvbox Light");
        assert_eq!(gl.background.to_hex(), "#fbf1c7");
        assert_eq!(gl.ansi[0].to_hex(), "#fbf1c7");
        assert_eq!(gl.selection_background.to_hex(), "#3c3836");
        assert_eq!(gl.ansi[9].to_hex(), "#9d0006");

        assert_eq!(get("Gruvbox Dark").ansi[9].to_hex(), "#fb4934");
        assert_eq!(get("Rose Pine Moon").ansi[2].to_hex(), "#3e8fb0");
    }

    #[test]
    fn variants_are_correct() {
        for p in references() {
            let expected = match p.name.as_str() {
                "Rose Pine Dawn" | "Catppuccin Latte" | "Gruvbox Light" => Variant::Light,
                _ => Variant::Dark,
            };
            assert_eq!(p.variant, expected, "wrong variant for {}", p.name);
        }
    }

    #[test]
    fn all_have_no_minimum_contrast_and_no_author() {
        for p in references() {
            assert_eq!(
                p.minimum_contrast, None,
                "{} should have no minimum_contrast",
                p.name
            );
            assert_eq!(p.author, None, "{} should have no author", p.name);
        }
    }

    #[test]
    fn unknown_name_is_none() {
        assert!(reference_by_name("Not A Real Theme").is_none());
        assert!(reference_by_name("Rosé Pine").is_none());
    }

    #[test]
    fn every_declared_name_resolves() {
        for name in REFERENCE_NAMES {
            assert!(reference_by_name(name).is_some(), "no palette for {name}");
        }
    }
}
