use std::io;
use std::path::{Path, PathBuf};

use citrine_core::palette::Palette;

use super::{export_theme, parse_active, write_file, ScaffoldManifest, TerminalAdapter};
use crate::config::{ImportError, Roots};

pub struct Kitty;

impl TerminalAdapter for Kitty {
    fn id(&self) -> &'static str {
        "kitty"
    }

    fn format_id(&self) -> &'static str {
        "kitty"
    }

    fn file_extension(&self) -> &'static str {
        "conf"
    }

    fn can_import(&self) -> bool {
        true
    }

    fn reload_hint(&self) -> &'static str {
        "Add `include themes/<file>` to kitty.conf, then restart or `kitty @ set-colors -a themes/<file>`."
    }

    fn theme_dir(&self, roots: &Roots) -> PathBuf {
        roots.config.join("kitty").join("themes")
    }

    fn config_dir(&self, roots: &Roots) -> PathBuf {
        roots.config.join("kitty")
    }

    fn current_theme(&self, roots: &Roots) -> Result<Palette, ImportError> {
        parse_active(self.config_dir(roots).join("kitty.conf"), "kitty", "kitty")
    }

    fn scaffold(
        &self,
        _roots: &Roots,
        dir: &Path,
        palette: &Palette,
        _probe_cmd: &str,
    ) -> io::Result<ScaffoldManifest> {
        let theme = dir.join("theme.conf");
        let conf = dir.join("kitty.conf");

        write_file(&theme, &export_theme("kitty", palette))?;
        write_file(&conf, "include theme.conf\nallow_remote_control yes\n")?;

        Ok(ScaffoldManifest {
            files: vec![theme, conf.clone()],
            launch_env: Vec::new(),
            validator: Some(vec![
                "kitty".to_string(),
                "--config".to_string(),
                conf.display().to_string(),
                "--debug-config".to_string(),
            ]),
            main_config: Some(conf),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::testkit::{sentinel, temp_roots};
    use super::super::TerminalAdapter;
    use crate::config::{read_current_theme, ImportError};
    use citrine_core::formats::format_by_id;
    use citrine_core::palette::Palette;
    use std::fs;

    #[test]
    fn scaffold_writes_verify_layout() {
        let (tmp, roots) = temp_roots();
        let dir = tmp.path().join("scratch");
        let manifest = super::Kitty
            .scaffold(&roots, &dir, &sentinel(), "exec probe")
            .unwrap();

        let theme = dir.join("theme.conf");
        let conf = dir.join("kitty.conf");
        assert!(fs::read_to_string(&theme)
            .unwrap()
            .contains("color0 #2040b0\n"));
        assert_eq!(
            fs::read_to_string(&conf).unwrap(),
            "include theme.conf\nallow_remote_control yes\n"
        );
        assert_eq!(manifest.files, vec![theme, conf.clone()]);
        assert!(manifest.launch_env.is_empty());
        assert_eq!(
            manifest.validator,
            Some(vec![
                "kitty".to_string(),
                "--config".to_string(),
                conf.display().to_string(),
                "--debug-config".to_string(),
            ])
        );
        assert_eq!(manifest.main_config, Some(conf));
    }

    #[test]
    fn current_kitty_parses_active_conf() {
        let (_tmp, roots) = temp_roots();
        let kitty = roots.config.join("kitty");
        fs::create_dir_all(&kitty).unwrap();
        let text = format_by_id("kitty").unwrap().export(&Palette::default());
        fs::write(kitty.join("kitty.conf"), &text).unwrap();

        let palette = read_current_theme(&roots, "kitty").ok().unwrap();
        assert_eq!(palette.background.to_hex(), "#f0e5ac");
    }

    #[test]
    fn current_kitty_missing_conf_is_not_found() {
        let (_tmp, roots) = temp_roots();
        let err = read_current_theme(&roots, "kitty").err().unwrap();
        assert!(matches!(err, ImportError::NotFound(_)));
    }

    #[test]
    fn current_malformed_theme_is_parse_error() {
        let (_tmp, roots) = temp_roots();
        let kitty = roots.config.join("kitty");
        fs::create_dir_all(&kitty).unwrap();
        fs::write(kitty.join("kitty.conf"), "background not-a-hex-color\n").unwrap();

        let err = read_current_theme(&roots, "kitty").err().unwrap();
        assert!(matches!(err, ImportError::Parse(_)));
    }
}
