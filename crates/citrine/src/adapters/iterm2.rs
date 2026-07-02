use std::io;
use std::path::{Path, PathBuf};

use citrine_core::palette::Palette;
use serde_json::Value;

use super::{export_theme, write_file, write_run_script, ScaffoldManifest, TerminalAdapter};
use crate::config::{slugify, ImportError, Roots};

pub struct Iterm2;

impl TerminalAdapter for Iterm2 {
    fn id(&self) -> &'static str {
        "iterm2"
    }

    fn format_id(&self) -> &'static str {
        "iterm2"
    }

    fn file_extension(&self) -> &'static str {
        "json"
    }

    fn can_import(&self) -> bool {
        false
    }

    fn reload_hint(&self) -> &'static str {
        "iTerm2 auto-loads the Dynamic Profile; select it in Settings > Profiles."
    }

    fn theme_dir(&self, roots: &Roots) -> PathBuf {
        roots
            .home
            .join("Library")
            .join("Application Support")
            .join("iTerm2")
            .join("DynamicProfiles")
    }

    fn config_dir(&self, roots: &Roots) -> PathBuf {
        roots
            .home
            .join("Library")
            .join("Application Support")
            .join("iTerm2")
    }

    fn current_theme(&self, _roots: &Roots) -> Result<Palette, ImportError> {
        Err(ImportError::Unsupported(
            "import not supported for iterm2".to_string(),
        ))
    }

    fn scaffold(
        &self,
        roots: &Roots,
        dir: &Path,
        palette: &Palette,
        probe_cmd: &str,
    ) -> io::Result<ScaffoldManifest> {
        let run = dir.join("run.sh");
        write_run_script(&run, probe_cmd)?;

        let mut doc: Value =
            serde_json::from_str(&export_theme("iterm2", palette)).map_err(io::Error::other)?;
        doc["Profiles"][0]["Command"] = Value::String(run.display().to_string());
        doc["Profiles"][0]["Custom Command"] = Value::String("Yes".to_string());
        let mut text = serde_json::to_string_pretty(&doc).map_err(io::Error::other)?;
        text.push('\n');

        let profile = self
            .theme_dir(roots)
            .join(format!("{}.json", slugify(&palette.name)));
        write_file(&profile, &text)?;

        Ok(ScaffoldManifest {
            files: vec![run, profile.clone()],
            launch_env: Vec::new(),
            validator: None,
            main_config: Some(profile),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::testkit::{assert_executable, sentinel, temp_roots};
    use super::super::TerminalAdapter;
    use crate::config::{find, read_current_theme, ImportError, Roots};
    use std::fs;

    #[test]
    fn scaffold_writes_dynamic_profile_with_injected_command() {
        let (tmp, roots) = temp_roots();
        let dir = tmp.path().join("scratch");
        let probe = "exec \"/bin/citrine\" probe --checks ansi,fg,bg";
        let manifest = super::Iterm2
            .scaffold(&roots, &dir, &sentinel(), probe)
            .unwrap();

        let run = dir.join("run.sh");
        let profile = roots
            .home
            .join("Library")
            .join("Application Support")
            .join("iTerm2")
            .join("DynamicProfiles")
            .join("citrine-sentinel.json");

        assert_eq!(
            fs::read_to_string(&run).unwrap(),
            format!("#!/bin/sh\n{probe}\n")
        );
        assert_executable(&run);

        let text = fs::read_to_string(&profile).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
        let p = &doc["Profiles"][0];
        assert_eq!(p["Command"], run.display().to_string());
        assert_eq!(p["Custom Command"], "Yes");
        assert_eq!(p["Name"], "Citrine Sentinel");
        assert_eq!(p["Guid"], "citrine-citrine-sentinel");
        assert!(p["Background Color"].is_object());
        assert!(
            text.contains("\"Guid\""),
            "guid stays greppable for the script"
        );

        assert_eq!(manifest.files, vec![run, profile.clone()]);
        assert!(manifest.launch_env.is_empty());
        assert!(manifest.validator.is_none());
        assert_eq!(manifest.main_config, Some(profile));
    }

    #[test]
    fn iterm2_present_tracks_app_support_dir_and_writes_to_home() {
        let tmp = tempfile::tempdir().unwrap();
        let roots = Roots {
            home: tmp.path().to_path_buf(),
            config: tmp.path().join(".config"),
            data: tmp.path().join(".local").join("share"),
        };
        let iterm2 = find("iterm2").unwrap();

        assert_eq!(
            iterm2.theme_dir(&roots),
            tmp.path()
                .join("Library")
                .join("Application Support")
                .join("iTerm2")
                .join("DynamicProfiles")
        );

        let support = tmp
            .path()
            .join("Library")
            .join("Application Support")
            .join("iTerm2");
        assert_eq!(iterm2.config_dir(&roots), support);
        assert!(
            !iterm2.present(&roots),
            "absent before the support dir exists"
        );

        std::fs::create_dir_all(&support).unwrap();
        assert!(
            iterm2.present(&roots),
            "present once the support dir exists"
        );
    }

    #[test]
    fn iterm2_writes_dynamic_profile_into_app_support() {
        let (_tmp, roots) = temp_roots();
        let td = find("iterm2").unwrap().theme_dir(&roots);
        assert_eq!(
            td,
            roots
                .home
                .join("Library")
                .join("Application Support")
                .join("iTerm2")
                .join("DynamicProfiles")
        );
        assert_eq!(find("iterm2").unwrap().file_extension(), "json");
        assert!(!find("iterm2").unwrap().can_import());
    }

    #[test]
    fn current_iterm2_import_unsupported() {
        let (_tmp, roots) = temp_roots();
        assert!(matches!(
            read_current_theme(&roots, "iterm2").err().unwrap(),
            ImportError::Unsupported(_)
        ));
    }
}
