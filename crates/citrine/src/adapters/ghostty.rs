use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use citrine_core::palette::Palette;

use super::{
    export_theme, last_theme_value, parse_text, write_file, write_run_script, ScaffoldManifest,
    TerminalAdapter,
};
use crate::config::{slugify, ImportError, Roots};

pub struct Ghostty;

impl TerminalAdapter for Ghostty {
    fn id(&self) -> &'static str {
        "ghostty"
    }

    fn format_id(&self) -> &'static str {
        "ghostty"
    }

    fn file_extension(&self) -> &'static str {
        ""
    }

    fn can_import(&self) -> bool {
        true
    }

    fn reload_hint(&self) -> &'static str {
        "Reload Ghostty: Cmd+Shift+, (or restart)."
    }

    fn theme_dir(&self, roots: &Roots) -> PathBuf {
        roots.config.join("ghostty").join("themes")
    }

    fn config_dir(&self, roots: &Roots) -> PathBuf {
        roots.config.join("ghostty")
    }

    fn current_theme(&self, roots: &Roots) -> Result<Palette, ImportError> {
        let ghostty = self.config_dir(roots);
        let config = ghostty.join("config");

        let text = fs::read_to_string(&config)
            .map_err(|_| ImportError::NotFound("no ghostty config found".to_string()))?;
        let theme = last_theme_value(&text).ok_or_else(|| {
            ImportError::NotFound("no active theme set in ghostty config".to_string())
        })?;

        let user_theme = ghostty.join("themes").join(&theme);
        let app_theme =
            PathBuf::from("/Applications/Ghostty.app/Contents/Resources/ghostty/themes")
                .join(&theme);

        let theme_text = fs::read_to_string(&user_theme)
            .or_else(|_| fs::read_to_string(&app_theme))
            .map_err(|_| ImportError::NotFound(format!("theme file not found: {theme}")))?;

        parse_text(&theme_text, "ghostty")
    }

    fn scaffold(
        &self,
        _roots: &Roots,
        dir: &Path,
        palette: &Palette,
        probe_cmd: &str,
    ) -> io::Result<ScaffoldManifest> {
        let slug = slugify(&palette.name);
        let xdg = dir.join("xdg");
        let theme = xdg.join("ghostty").join("themes").join(&slug);
        let run = dir.join("run.sh");
        let config = xdg.join("ghostty").join("config");

        write_file(&theme, &export_theme("ghostty", palette))?;
        write_run_script(&run, probe_cmd)?;
        write_file(
            &config,
            &format!(
                "theme = {slug}\ncommand = {run}\nconfirm-close-surface = false\nquit-after-last-window-closed = true\nwindow-save-state = never\n",
                run = run.display()
            ),
        )?;

        Ok(ScaffoldManifest {
            files: vec![theme, run, config.clone()],
            launch_env: vec![("XDG_CONFIG_HOME".to_string(), xdg.display().to_string())],
            validator: None,
            main_config: Some(config),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::testkit::{assert_executable, sentinel, temp_roots};
    use super::super::TerminalAdapter;
    use crate::config::{read_current_theme, ImportError};
    use citrine_core::formats::format_by_id;
    use citrine_core::palette::Palette;
    use std::fs;

    #[test]
    fn scaffold_writes_verify_layout() {
        let (tmp, roots) = temp_roots();
        let dir = tmp.path().join("scratch");
        let probe = "exec \"/bin/citrine\" probe --checks ansi,fg,bg";
        let manifest = super::Ghostty
            .scaffold(&roots, &dir, &sentinel(), probe)
            .unwrap();

        let theme = dir
            .join("xdg")
            .join("ghostty")
            .join("themes")
            .join("citrine-sentinel");
        let run = dir.join("run.sh");
        let config = dir.join("xdg").join("ghostty").join("config");

        assert!(fs::read_to_string(&theme)
            .unwrap()
            .contains("background = #101317\n"));
        assert_eq!(
            fs::read_to_string(&run).unwrap(),
            format!("#!/bin/sh\n{probe}\n")
        );
        assert_executable(&run);
        assert_eq!(
            fs::read_to_string(&config).unwrap(),
            format!(
                "theme = citrine-sentinel\ncommand = {}\nconfirm-close-surface = false\nquit-after-last-window-closed = true\nwindow-save-state = never\n",
                run.display()
            )
        );
        assert_eq!(manifest.files, vec![theme, run, config.clone()]);
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
    fn current_ghostty_parses_active_theme() {
        let (_tmp, roots) = temp_roots();
        let ghostty = roots.config.join("ghostty");
        fs::create_dir_all(ghostty.join("themes")).unwrap();
        fs::write(
            ghostty.join("config"),
            "# my ghostty config\nfont-family = Fira Code\ntheme = citrus\n",
        )
        .unwrap();
        let theme_text = format_by_id("ghostty").unwrap().export(&Palette::default());
        fs::write(ghostty.join("themes").join("citrus"), &theme_text).unwrap();

        let palette = read_current_theme(&roots, "ghostty").ok().unwrap();
        let expected = format_by_id("ghostty")
            .unwrap()
            .import(&theme_text)
            .unwrap();
        assert_eq!(palette, expected);
        assert_eq!(palette.background.to_hex(), "#f0e5ac");
        assert_eq!(palette.ansi[1].to_hex(), "#b44c37");
    }

    #[test]
    fn current_ghostty_missing_config_is_not_found() {
        let (_tmp, roots) = temp_roots();
        let err = read_current_theme(&roots, "ghostty").err().unwrap();
        assert!(matches!(err, ImportError::NotFound(_)));
    }

    #[test]
    fn current_ghostty_theme_file_missing_is_not_found() {
        let (_tmp, roots) = temp_roots();
        let ghostty = roots.config.join("ghostty");
        fs::create_dir_all(&ghostty).unwrap();
        fs::write(ghostty.join("config"), "theme = nonexistent-theme-xyz\n").unwrap();

        let err = read_current_theme(&roots, "ghostty").err().unwrap();
        match err {
            ImportError::NotFound(msg) => assert!(msg.contains("nonexistent-theme-xyz")),
            _ => panic!("expected NotFound"),
        }
    }

    #[test]
    fn current_ghostty_no_theme_line_is_not_found() {
        let (_tmp, roots) = temp_roots();
        let ghostty = roots.config.join("ghostty");
        fs::create_dir_all(&ghostty).unwrap();
        fs::write(
            ghostty.join("config"),
            "# theme = commented\nfont-family = Fira\ntheme =\n",
        )
        .unwrap();

        let err = read_current_theme(&roots, "ghostty").err().unwrap();
        match err {
            ImportError::NotFound(msg) => assert!(msg.contains("no active theme set")),
            _ => panic!("expected NotFound"),
        }
    }
}
