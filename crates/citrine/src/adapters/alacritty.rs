use std::io;
use std::path::{Path, PathBuf};

use citrine_core::palette::Palette;

use super::{export_theme, parse_active, write_file, ScaffoldManifest, TerminalAdapter};
use crate::config::{ImportError, Roots};

pub struct Alacritty;

impl TerminalAdapter for Alacritty {
    fn id(&self) -> &'static str {
        "alacritty"
    }

    fn format_id(&self) -> &'static str {
        "alacritty"
    }

    fn file_extension(&self) -> &'static str {
        "toml"
    }

    fn can_import(&self) -> bool {
        true
    }

    fn reload_hint(&self) -> &'static str {
        "Add the file to `[general] import` in alacritty.toml (live-reloads)."
    }

    fn theme_dir(&self, roots: &Roots) -> PathBuf {
        roots.config.join("alacritty").join("themes")
    }

    fn config_dir(&self, roots: &Roots) -> PathBuf {
        roots.config.join("alacritty")
    }

    fn current_theme(&self, roots: &Roots) -> Result<Palette, ImportError> {
        parse_active(
            self.config_dir(roots).join("alacritty.toml"),
            "alacritty",
            "alacritty",
        )
    }

    fn scaffold(
        &self,
        _roots: &Roots,
        dir: &Path,
        palette: &Palette,
        _probe_cmd: &str,
    ) -> io::Result<ScaffoldManifest> {
        let theme = dir.join("theme.toml");
        let conf = dir.join("alacritty.toml");

        write_file(&theme, &export_theme("alacritty", palette))?;
        let theme_path = theme.display();
        write_file(
            &conf,
            &format!("import = [\"{theme_path}\"]\n\n[general]\nimport = [\"{theme_path}\"]\n"),
        )?;

        Ok(ScaffoldManifest {
            files: vec![theme, conf.clone()],
            launch_env: Vec::new(),
            validator: None,
            main_config: Some(conf),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::testkit::{sentinel, temp_roots};
    use super::super::TerminalAdapter;
    use crate::config::read_current_theme;
    use citrine_core::formats::format_by_id;
    use citrine_core::palette::Palette;
    use std::fs;

    #[test]
    fn scaffold_writes_verify_layout() {
        let (tmp, roots) = temp_roots();
        let dir = tmp.path().join("scratch");
        let manifest = super::Alacritty
            .scaffold(&roots, &dir, &sentinel(), "exec probe")
            .unwrap();

        let theme = dir.join("theme.toml");
        let conf = dir.join("alacritty.toml");
        assert!(fs::read_to_string(&theme)
            .unwrap()
            .contains("background = \"#101317\"\n"));
        assert_eq!(
            fs::read_to_string(&conf).unwrap(),
            format!(
                "import = [\"{t}\"]\n\n[general]\nimport = [\"{t}\"]\n",
                t = theme.display()
            )
        );
        assert_eq!(manifest.files, vec![theme, conf.clone()]);
        assert!(manifest.launch_env.is_empty());
        assert!(manifest.validator.is_none());
        assert_eq!(manifest.main_config, Some(conf));
    }

    #[test]
    fn current_alacritty_parses_active_toml() {
        let (_tmp, roots) = temp_roots();
        let ala = roots.config.join("alacritty");
        fs::create_dir_all(&ala).unwrap();
        let text = format_by_id("alacritty")
            .unwrap()
            .export(&Palette::default());
        fs::write(ala.join("alacritty.toml"), &text).unwrap();

        let palette = read_current_theme(&roots, "alacritty").ok().unwrap();
        assert_eq!(palette.background.to_hex(), "#f0e5ac");
    }
}
