use std::io;
use std::path::{Path, PathBuf};

use citrine_core::palette::Palette;

use super::{export_theme, write_file, ScaffoldManifest, TerminalAdapter};
use crate::config::{ImportError, Roots};

pub struct WezTerm;

impl TerminalAdapter for WezTerm {
    fn id(&self) -> &'static str {
        "wezterm"
    }

    fn format_id(&self) -> &'static str {
        "wezterm"
    }

    fn file_extension(&self) -> &'static str {
        "toml"
    }

    fn can_import(&self) -> bool {
        false
    }

    fn reload_hint(&self) -> &'static str {
        "Set `color_scheme` / load the file in wezterm.lua (live-reloads)."
    }

    fn theme_dir(&self, roots: &Roots) -> PathBuf {
        roots.config.join("wezterm").join("colors")
    }

    fn config_dir(&self, roots: &Roots) -> PathBuf {
        roots.config.join("wezterm")
    }

    fn current_theme(&self, _roots: &Roots) -> Result<Palette, ImportError> {
        Err(ImportError::Unsupported(
            "import not supported for wezterm".to_string(),
        ))
    }

    fn scaffold(
        &self,
        _roots: &Roots,
        dir: &Path,
        palette: &Palette,
        probe_cmd: &str,
    ) -> io::Result<ScaffoldManifest> {
        let scheme = palette.name.as_str();
        let colors = dir.join("colors");
        let theme = colors.join(format!("{scheme}.toml"));
        let lua = dir.join("wezterm.lua");

        write_file(&theme, &export_theme("wezterm", palette))?;
        write_file(
            &lua,
            &format!(
                "return {{\n  color_scheme_dirs = {{ \"{colors}\" }},\n  color_scheme = \"{scheme}\",\n  default_prog = {{ \"/bin/sh\", \"-c\", [[{probe_cmd}]] }},\n  enable_wayland = false,\n  front_end = \"Software\",\n}}\n",
                colors = colors.display()
            ),
        )?;

        Ok(ScaffoldManifest {
            files: vec![theme, lua.clone()],
            launch_env: Vec::new(),
            validator: Some(vec![
                "wezterm".to_string(),
                "--config-file".to_string(),
                lua.display().to_string(),
                "ls-fonts".to_string(),
            ]),
            main_config: Some(lua),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::testkit::{sentinel, temp_roots};
    use super::super::TerminalAdapter;
    use crate::config::{read_current_theme, ImportError};
    use std::fs;

    #[test]
    fn scaffold_writes_verify_layout() {
        let (tmp, roots) = temp_roots();
        let dir = tmp.path().join("scratch");
        let probe = "exec \"/bin/citrine\" probe --checks ansi,fg,bg";
        let manifest = super::WezTerm
            .scaffold(&roots, &dir, &sentinel(), probe)
            .unwrap();

        let theme = dir.join("colors").join("Citrine Sentinel.toml");
        let lua = dir.join("wezterm.lua");
        assert!(theme.is_file());
        assert_eq!(
            fs::read_to_string(&lua).unwrap(),
            format!(
                "return {{\n  color_scheme_dirs = {{ \"{colors}\" }},\n  color_scheme = \"Citrine Sentinel\",\n  default_prog = {{ \"/bin/sh\", \"-c\", [[{probe}]] }},\n  enable_wayland = false,\n  front_end = \"Software\",\n}}\n",
                colors = dir.join("colors").display()
            )
        );
        assert_eq!(manifest.files, vec![theme, lua.clone()]);
        assert!(manifest.launch_env.is_empty());
        assert_eq!(
            manifest.validator,
            Some(vec![
                "wezterm".to_string(),
                "--config-file".to_string(),
                lua.display().to_string(),
                "ls-fonts".to_string(),
            ])
        );
        assert_eq!(manifest.main_config, Some(lua));
    }

    #[test]
    fn current_import_unsupported_terminal() {
        let (_tmp, roots) = temp_roots();
        let err = read_current_theme(&roots, "wezterm").err().unwrap();
        assert!(matches!(err, ImportError::Unsupported(_)));
    }
}
