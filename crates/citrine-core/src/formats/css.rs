use crate::formats::ThemeFormat;
use crate::palette::Palette;

pub struct Css;

impl ThemeFormat for Css {
    fn id(&self) -> &'static str {
        "css"
    }

    fn display_name(&self) -> &'static str {
        "CSS Variables"
    }

    fn file_extension(&self) -> &'static str {
        "css"
    }

    fn export(&self, p: &Palette) -> String {
        let mut out = String::from(":root {\n");
        out.push_str(&format!(
            "  --term-background: {};\n",
            p.background.to_hex()
        ));
        out.push_str(&format!(
            "  --term-foreground: {};\n",
            p.foreground.to_hex()
        ));
        out.push_str(&format!("  --term-cursor: {};\n", p.cursor.to_hex()));
        out.push_str(&format!(
            "  --term-cursor-text: {};\n",
            p.cursor_text.to_hex()
        ));
        out.push_str(&format!(
            "  --term-selection-background: {};\n",
            p.selection_background.to_hex()
        ));
        out.push_str(&format!(
            "  --term-selection-foreground: {};\n",
            p.selection_foreground.to_hex()
        ));
        for (i, color) in p.ansi.iter().enumerate() {
            out.push_str(&format!("  --term-ansi-{}: {};\n", i, color.to_hex()));
        }
        out.push_str("}\n");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::FormatError;

    const GOLDEN: &str = concat!(
        ":root {\n",
        "  --term-background: #f0e5ac;\n",
        "  --term-foreground: #5a5368;\n",
        "  --term-cursor: #dd7714;\n",
        "  --term-cursor-text: #2b2820;\n",
        "  --term-selection-background: #e6cf88;\n",
        "  --term-selection-foreground: #4b4656;\n",
        "  --term-ansi-0: #4b4656;\n",
        "  --term-ansi-1: #b44c37;\n",
        "  --term-ansi-2: #30803f;\n",
        "  --term-ansi-3: #8d610c;\n",
        "  --term-ansi-4: #335da8;\n",
        "  --term-ansi-5: #8d47ac;\n",
        "  --term-ansi-6: #1a847f;\n",
        "  --term-ansi-7: #cdc1ab;\n",
        "  --term-ansi-8: #6f6a80;\n",
        "  --term-ansi-9: #c85a44;\n",
        "  --term-ansi-10: #3a8f4a;\n",
        "  --term-ansi-11: #9e7013;\n",
        "  --term-ansi-12: #3f6bb4;\n",
        "  --term-ansi-13: #9d54ba;\n",
        "  --term-ansi-14: #219a92;\n",
        "  --term-ansi-15: #eae0c6;\n",
        "}\n",
    );

    #[test]
    fn golden_default_export() {
        assert_eq!(Css.export(&Palette::default()), GOLDEN);
    }

    #[test]
    fn export_is_wrapped_and_newline_terminated() {
        let out = Css.export(&Palette::default());
        assert!(out.starts_with(":root {\n"));
        assert!(out.ends_with("}\n"));
        assert_eq!(out.matches("--term-ansi-").count(), 16);
    }

    #[test]
    fn import_is_unsupported() {
        assert_eq!(
            Css.import(":root {}").unwrap_err(),
            FormatError::ImportUnsupported
        );
        assert_eq!(
            Css.import(&Css.export(&Palette::default())).unwrap_err(),
            FormatError::ImportUnsupported
        );
    }

    #[test]
    fn export_declares_all_22_custom_properties() {
        let out = Css.export(&Palette::default());
        for var in [
            "--term-background",
            "--term-foreground",
            "--term-cursor",
            "--term-cursor-text",
            "--term-selection-background",
            "--term-selection-foreground",
        ] {
            assert!(out.contains(&format!("{var}:")), "missing {var}");
        }
        for i in 0..16 {
            assert!(
                out.contains(&format!("--term-ansi-{i}:")),
                "missing --term-ansi-{i}"
            );
        }
        assert_eq!(out.matches("--term-").count(), 22);
    }

    #[test]
    fn metadata_is_correct() {
        assert_eq!(Css.id(), "css");
        assert_eq!(Css.display_name(), "CSS Variables");
        assert_eq!(Css.file_extension(), "css");
    }
}
