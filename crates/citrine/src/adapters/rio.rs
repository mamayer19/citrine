use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use citrine_core::palette::Palette;

use super::{
    export_theme, last_theme_value, parse_text, write_file, ScaffoldManifest, TerminalAdapter,
};
use crate::config::{slugify, ImportError, Roots};

pub struct Rio;

impl TerminalAdapter for Rio {
    fn id(&self) -> &'static str {
        "rio"
    }

    fn format_id(&self) -> &'static str {
        "rio"
    }

    fn file_extension(&self) -> &'static str {
        "toml"
    }

    fn can_import(&self) -> bool {
        true
    }

    fn reload_hint(&self) -> &'static str {
        "Set `theme = \"<name>\"` in rio config."
    }

    fn theme_dir(&self, roots: &Roots) -> PathBuf {
        roots.config.join("rio").join("themes")
    }

    fn config_dir(&self, roots: &Roots) -> PathBuf {
        roots.config.join("rio")
    }

    fn current_theme(&self, roots: &Roots) -> Result<Palette, ImportError> {
        let rio = self.config_dir(roots);
        let config = rio.join("config.toml");

        let text = fs::read_to_string(&config)
            .map_err(|_| ImportError::NotFound("no rio config found".to_string()))?;

        if let Some(theme) = rio_theme_value(&text) {
            let theme_file = rio.join("themes").join(format!("{theme}.toml"));
            let theme_text = fs::read_to_string(&theme_file)
                .map_err(|_| ImportError::NotFound(format!("theme file not found: {theme}")))?;
            return parse_text(&theme_text, "rio");
        }

        parse_text(&text, "rio")
    }

    fn scaffold(
        &self,
        _roots: &Roots,
        dir: &Path,
        palette: &Palette,
        _probe_cmd: &str,
    ) -> io::Result<ScaffoldManifest> {
        let slug = slugify(&palette.name);
        let xdg = dir.join("xdg");
        let theme = xdg.join("rio").join("themes").join(format!("{slug}.toml"));
        let config = xdg.join("rio").join("config.toml");

        write_file(&theme, &export_theme("rio", palette))?;
        write_file(&config, &format!("theme = \"{slug}\"\n"))?;

        Ok(ScaffoldManifest {
            files: vec![theme, config.clone()],
            launch_env: vec![("XDG_CONFIG_HOME".to_string(), xdg.display().to_string())],
            validator: None,
            main_config: Some(config),
        })
    }
}

fn rio_theme_value(text: &str) -> Option<String> {
    let raw = last_theme_value(text)?;
    let trimmed = raw.trim_matches(|c| c == '"' || c == '\'');
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::super::testkit::{sentinel, temp_roots};
    use super::super::TerminalAdapter;
    use super::rio_theme_value;
    use crate::config::{read_current_theme, ImportError};
    use citrine_core::formats::format_by_id;
    use citrine_core::palette::Palette;
    use std::fs;

    #[test]
    fn scaffold_writes_verify_layout() {
        let (tmp, roots) = temp_roots();
        let dir = tmp.path().join("scratch");
        let manifest = super::Rio
            .scaffold(&roots, &dir, &sentinel(), "exec probe")
            .unwrap();

        let theme = dir
            .join("xdg")
            .join("rio")
            .join("themes")
            .join("citrine-sentinel.toml");
        let config = dir.join("xdg").join("rio").join("config.toml");
        assert!(theme.is_file());
        assert_eq!(
            fs::read_to_string(&config).unwrap(),
            "theme = \"citrine-sentinel\"\n"
        );
        assert_eq!(manifest.files, vec![theme, config.clone()]);
        assert_eq!(
            manifest.launch_env,
            vec![(
                "XDG_CONFIG_HOME".to_string(),
                dir.join("xdg").display().to_string()
            )]
        );
        assert!(manifest.validator.is_none());
        assert_eq!(manifest.main_config, Some(config));
    }

    #[test]
    fn current_rio_follows_theme_reference() {
        let (_tmp, roots) = temp_roots();
        let rio = roots.config.join("rio");
        fs::create_dir_all(rio.join("themes")).unwrap();
        fs::write(rio.join("config.toml"), "theme = \"citrus\"\n").unwrap();
        let text = format_by_id("rio").unwrap().export(&Palette::default());
        fs::write(rio.join("themes").join("citrus.toml"), &text).unwrap();

        let palette = read_current_theme(&roots, "rio").ok().unwrap();
        assert_eq!(palette.background.to_hex(), "#f0e5ac");
    }

    #[test]
    fn current_rio_parses_inline_colors_without_theme_ref() {
        let (_tmp, roots) = temp_roots();
        let rio = roots.config.join("rio");
        fs::create_dir_all(&rio).unwrap();
        let text = format_by_id("rio").unwrap().export(&Palette::default());
        fs::write(rio.join("config.toml"), &text).unwrap();

        let palette = read_current_theme(&roots, "rio").ok().unwrap();
        assert_eq!(palette.background.to_hex(), "#f0e5ac");
    }

    #[test]
    fn current_rio_missing_config_is_not_found() {
        let (_tmp, roots) = temp_roots();
        assert!(matches!(
            read_current_theme(&roots, "rio").err().unwrap(),
            ImportError::NotFound(_)
        ));
    }

    #[test]
    fn current_rio_missing_referenced_theme_is_not_found() {
        let (_tmp, roots) = temp_roots();
        let rio = roots.config.join("rio");
        fs::create_dir_all(&rio).unwrap();
        fs::write(rio.join("config.toml"), "theme = \"ghost\"\n").unwrap();
        match read_current_theme(&roots, "rio").err().unwrap() {
            ImportError::NotFound(msg) => assert!(msg.contains("ghost")),
            _ => panic!("expected NotFound"),
        }
    }

    #[test]
    fn rio_theme_value_strips_quotes() {
        assert_eq!(
            rio_theme_value("theme = \"citrus\"\n"),
            Some("citrus".to_string())
        );
        assert_eq!(rio_theme_value("theme = 'x'\n"), Some("x".to_string()));
        assert_eq!(rio_theme_value("theme = \"\"\n"), None);
        assert_eq!(rio_theme_value("font = x\n"), None);
    }
}
