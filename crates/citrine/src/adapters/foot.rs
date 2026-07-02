use std::io;
use std::path::{Path, PathBuf};

use citrine_core::palette::Palette;

use super::{export_theme, parse_active, write_file, ScaffoldManifest, TerminalAdapter};
use crate::config::{ImportError, Roots};

pub struct Foot;

impl TerminalAdapter for Foot {
    fn id(&self) -> &'static str {
        "foot"
    }

    fn format_id(&self) -> &'static str {
        "foot"
    }

    fn file_extension(&self) -> &'static str {
        "ini"
    }

    fn can_import(&self) -> bool {
        true
    }

    fn reload_hint(&self) -> &'static str {
        "Add `include=themes/<file>` to foot.ini."
    }

    fn theme_dir(&self, roots: &Roots) -> PathBuf {
        roots.config.join("foot").join("themes")
    }

    fn config_dir(&self, roots: &Roots) -> PathBuf {
        roots.config.join("foot")
    }

    fn current_theme(&self, roots: &Roots) -> Result<Palette, ImportError> {
        parse_active(self.config_dir(roots).join("foot.ini"), "foot", "foot")
    }

    fn scaffold(
        &self,
        _roots: &Roots,
        dir: &Path,
        palette: &Palette,
        _probe_cmd: &str,
    ) -> io::Result<ScaffoldManifest> {
        let theme = dir.join("theme.ini");
        let conf = dir.join("foot.ini");

        write_file(&theme, &export_theme("foot", palette))?;
        write_file(&conf, &format!("include={}\n", theme.display()))?;

        Ok(ScaffoldManifest {
            files: vec![theme, conf.clone()],
            launch_env: Vec::new(),
            validator: Some(vec![
                "foot".to_string(),
                "--config".to_string(),
                conf.display().to_string(),
                "--check-config".to_string(),
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
    use std::fs;

    #[test]
    fn scaffold_writes_verify_layout() {
        let (tmp, roots) = temp_roots();
        let dir = tmp.path().join("scratch");
        let manifest = super::Foot
            .scaffold(&roots, &dir, &sentinel(), "exec probe")
            .unwrap();

        let theme = dir.join("theme.ini");
        let conf = dir.join("foot.ini");
        assert!(fs::read_to_string(&theme)
            .unwrap()
            .contains("background=101317\n"));
        assert_eq!(
            fs::read_to_string(&conf).unwrap(),
            format!("include={}\n", theme.display())
        );
        assert_eq!(manifest.files, vec![theme, conf.clone()]);
        assert!(manifest.launch_env.is_empty());
        assert_eq!(
            manifest.validator,
            Some(vec![
                "foot".to_string(),
                "--config".to_string(),
                conf.display().to_string(),
                "--check-config".to_string(),
            ])
        );
        assert_eq!(manifest.main_config, Some(conf));
    }

    #[test]
    fn current_foot_missing_config_is_not_found() {
        let (_tmp, roots) = temp_roots();
        assert!(matches!(
            read_current_theme(&roots, "foot").err().unwrap(),
            ImportError::NotFound(_)
        ));
    }
}
