use crate::palette::Palette;

mod alacritty;
mod base16;
mod css;
mod foot;
mod ghostty;
mod iterm2;
mod json;
mod kitty;
mod konsole;
mod rio;
mod wezterm;

pub use alacritty::Alacritty;
pub use base16::Base16;
pub use css::Css;
pub use foot::Foot;
pub use ghostty::Ghostty;
pub use iterm2::Iterm2;
pub use json::Json;
pub use kitty::Kitty;
pub use konsole::Konsole;
pub use rio::Rio;
pub use wezterm::WezTerm;

pub trait ThemeFormat {
    fn id(&self) -> &'static str;

    fn display_name(&self) -> &'static str;

    fn file_extension(&self) -> &'static str;

    fn export(&self, p: &Palette) -> String;

    fn import(&self, _t: &str) -> Result<Palette, FormatError> {
        Err(FormatError::ImportUnsupported)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    Parse(String),
    ImportUnsupported,
}

impl FormatError {
    pub fn parse(msg: impl Into<String>) -> Self {
        FormatError::Parse(msg.into())
    }
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::Parse(msg) => write!(f, "theme parse error: {msg}"),
            FormatError::ImportUnsupported => {
                write!(f, "this format does not support import")
            }
        }
    }
}

impl std::error::Error for FormatError {}

pub fn all_formats() -> Vec<Box<dyn ThemeFormat>> {
    vec![
        Box::new(Ghostty),
        Box::new(Kitty),
        Box::new(Alacritty),
        Box::new(WezTerm),
        Box::new(Iterm2),
        Box::new(Foot),
        Box::new(Rio),
        Box::new(Konsole),
        Box::new(Base16),
        Box::new(Css),
        Box::new(Json),
    ]
}

pub fn format_by_id(id: &str) -> Option<Box<dyn ThemeFormat>> {
    all_formats().into_iter().find(|f| f.id() == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Slot;

    #[test]
    fn registry_ids_are_unique_and_lookupable() {
        let formats = all_formats();
        let mut ids: Vec<&str> = formats.iter().map(|f| f.id()).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate format id in registry");

        for f in &formats {
            let found = format_by_id(f.id()).expect("registered id resolves");
            assert_eq!(found.id(), f.id());
        }
        assert!(format_by_id("nope").is_none());
    }

    #[test]
    fn every_format_exports_non_empty() {
        let p = Palette::default();
        for f in all_formats() {
            let out = f.export(&p);
            assert!(!out.is_empty(), "{} produced empty export", f.id());
        }
    }

    const LOSSLESS_IMPORT_IDS: [&str; 5] = ["ghostty", "kitty", "alacritty", "wezterm", "json"];

    #[test]
    fn lossless_formats_roundtrip_all_named_and_ansi_slots() {
        let p = Palette::default();
        for f in all_formats() {
            let out = f.export(&p);
            match f.import(&out) {
                Ok(back) => {
                    if LOSSLESS_IMPORT_IDS.contains(&f.id()) {
                        for slot in Slot::all() {
                            assert_eq!(
                                back.get(slot),
                                p.get(slot),
                                "{} lost slot {} on round-trip",
                                f.id(),
                                slot.label()
                            );
                        }
                    }
                }
                Err(FormatError::ImportUnsupported) => {}
                Err(e) => panic!("{} failed to re-import its own export: {e}", f.id()),
            }
        }
    }

    #[test]
    fn format_error_display_messages() {
        assert_eq!(
            FormatError::parse("boom").to_string(),
            "theme parse error: boom"
        );
        assert_eq!(
            FormatError::ImportUnsupported.to_string(),
            "this format does not support import"
        );
    }

    #[test]
    fn format_error_parse_constructor_builds_parse_variant() {
        assert_eq!(FormatError::parse("x"), FormatError::Parse("x".to_string()));
        assert_eq!(
            FormatError::parse(String::from("y")),
            FormatError::Parse("y".to_string())
        );
    }

    #[test]
    fn all_formats_exposes_every_expected_id_in_registry_order() {
        let ids: Vec<&str> = all_formats().iter().map(|f| f.id()).collect();
        assert_eq!(
            ids,
            [
                "ghostty",
                "kitty",
                "alacritty",
                "wezterm",
                "iterm2",
                "foot",
                "rio",
                "konsole",
                "base16",
                "css",
                "json",
            ]
        );
    }

    #[test]
    fn every_format_has_nonempty_display_name_resolvable_by_id() {
        for f in all_formats() {
            assert!(
                !f.display_name().is_empty(),
                "{} has empty display_name",
                f.id()
            );
            let by_id = format_by_id(f.id()).expect("registered id resolves");
            assert_eq!(by_id.display_name(), f.display_name());
        }
    }

    #[test]
    fn file_extensions_match_known_values() {
        let ext = |id: &str| format_by_id(id).unwrap().file_extension().to_string();
        assert_eq!(ext("ghostty"), "");
        assert_eq!(ext("kitty"), "conf");
        assert_eq!(ext("alacritty"), "toml");
        assert_eq!(ext("wezterm"), "toml");
        assert_eq!(ext("iterm2"), "json");
        assert_eq!(ext("foot"), "ini");
        assert_eq!(ext("rio"), "toml");
        assert_eq!(ext("konsole"), "colorscheme");
        assert_eq!(ext("base16"), "yaml");
        assert_eq!(ext("css"), "css");
        assert_eq!(ext("json"), "json");
    }

    #[test]
    fn export_only_formats_report_import_unsupported() {
        let p = Palette::default();
        for id in ["css", "iterm2"] {
            let f = format_by_id(id).unwrap();
            assert_eq!(
                f.import(&f.export(&p)).unwrap_err(),
                FormatError::ImportUnsupported,
                "{id} should not support import"
            );
        }
    }

    #[test]
    fn format_by_id_rejects_unknown_ids() {
        assert!(format_by_id("").is_none());
        assert!(format_by_id("toml").is_none());
        assert!(
            format_by_id("GHOSTTY").is_none(),
            "lookup is case-sensitive"
        );
    }
}
