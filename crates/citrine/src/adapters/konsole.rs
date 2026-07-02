use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use citrine_core::palette::Palette;

use super::{export_theme, parse_text, write_file, ScaffoldManifest, TerminalAdapter};
use crate::config::{ImportError, Roots};

pub struct Konsole;

impl TerminalAdapter for Konsole {
    fn id(&self) -> &'static str {
        "konsole"
    }

    fn format_id(&self) -> &'static str {
        "konsole"
    }

    fn file_extension(&self) -> &'static str {
        "colorscheme"
    }

    fn can_import(&self) -> bool {
        true
    }

    fn reload_hint(&self) -> &'static str {
        "Pick the scheme in Konsole profile > Appearance."
    }

    fn theme_dir(&self, roots: &Roots) -> PathBuf {
        roots.data.join("konsole")
    }

    fn config_dir(&self, roots: &Roots) -> PathBuf {
        roots.config.join("konsole")
    }

    fn current_theme(&self, roots: &Roots) -> Result<Palette, ImportError> {
        let data = roots.data.join("konsole");
        let konsolerc = self.config_dir(roots).join("konsolerc");

        let rc = fs::read_to_string(&konsolerc)
            .map_err(|_| ImportError::NotFound("no konsole config found".to_string()))?;
        let profile = ini_value(&rc, "Desktop Entry", "DefaultProfile")
            .ok_or_else(|| ImportError::NotFound("no default konsole profile set".to_string()))?;

        let profile_text = fs::read_to_string(data.join(&profile))
            .map_err(|_| ImportError::NotFound(format!("konsole profile not found: {profile}")))?;
        let scheme = ini_value(&profile_text, "Appearance", "ColorScheme").ok_or_else(|| {
            ImportError::NotFound("no color scheme set in konsole profile".to_string())
        })?;

        let scheme_path = data.join(format!("{scheme}.colorscheme"));
        let text = fs::read_to_string(&scheme_path)
            .map_err(|_| ImportError::NotFound(format!("konsole scheme not found: {scheme}")))?;
        parse_text(&text, "konsole")
    }

    fn scaffold(
        &self,
        roots: &Roots,
        _dir: &Path,
        palette: &Palette,
        _probe_cmd: &str,
    ) -> io::Result<ScaffoldManifest> {
        let scheme = scheme_name(&palette.name);
        let data = roots.data.join("konsole");
        let colorscheme = data.join(format!("{scheme}.colorscheme"));
        let profile = data.join("citrine.profile");
        let konsolerc = roots.config.join("konsolerc");

        write_file(&colorscheme, &export_theme("konsole", palette))?;
        write_file(
            &profile,
            &format!(
                "[Appearance]\nColorScheme={scheme}\n\n[General]\nName=citrine\nParent=FALLBACK/\n"
            ),
        )?;
        write_file(
            &konsolerc,
            "[Desktop Entry]\nDefaultProfile=citrine.profile\n",
        )?;

        Ok(ScaffoldManifest {
            files: vec![colorscheme, profile.clone(), konsolerc],
            launch_env: Vec::new(),
            validator: None,
            main_config: Some(profile),
        })
    }
}

fn scheme_name(name: &str) -> String {
    name.chars().filter(char::is_ascii_alphanumeric).collect()
}

fn ini_value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            in_section = name.trim() == section;
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                let v = v.trim();
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::testkit::{sentinel, temp_roots};
    use super::super::TerminalAdapter;
    use super::{ini_value, scheme_name};
    use crate::config::{read_current_theme, ImportError};
    use std::fs;

    #[test]
    fn scaffold_writes_verify_layout_into_real_roots() {
        let (tmp, roots) = temp_roots();
        let dir = tmp.path().join("scratch");
        let manifest = super::Konsole
            .scaffold(&roots, &dir, &sentinel(), "exec probe")
            .unwrap();

        let data = roots.data.join("konsole");
        let colorscheme = data.join("CitrineSentinel.colorscheme");
        let profile = data.join("citrine.profile");
        let konsolerc = roots.config.join("konsolerc");
        assert!(fs::read_to_string(&colorscheme)
            .unwrap()
            .contains("[Background]"));
        assert_eq!(
            fs::read_to_string(&profile).unwrap(),
            "[Appearance]\nColorScheme=CitrineSentinel\n\n[General]\nName=citrine\nParent=FALLBACK/\n"
        );
        assert_eq!(
            fs::read_to_string(&konsolerc).unwrap(),
            "[Desktop Entry]\nDefaultProfile=citrine.profile\n"
        );
        assert!(
            !dir.exists(),
            "konsole scaffold writes nothing to the scratch dir"
        );
        assert_eq!(
            manifest.files,
            vec![colorscheme, profile.clone(), konsolerc]
        );
        assert!(manifest.launch_env.is_empty());
        assert!(manifest.validator.is_none());
        assert_eq!(manifest.main_config, Some(profile));
    }

    #[test]
    fn scheme_name_keeps_alphanumerics_only() {
        assert_eq!(scheme_name("Citrine Sentinel"), "CitrineSentinel");
        assert_eq!(scheme_name("Citrus Field (Dawn)"), "CitrusFieldDawn");
    }

    #[test]
    fn current_konsole_missing_config_is_not_found() {
        let (_tmp, roots) = temp_roots();
        assert!(matches!(
            read_current_theme(&roots, "konsole").err().unwrap(),
            ImportError::NotFound(_)
        ));
    }

    #[test]
    fn current_konsole_no_default_profile_is_not_found() {
        let (_tmp, roots) = temp_roots();
        let konsole = roots.config.join("konsole");
        fs::create_dir_all(&konsole).unwrap();
        fs::write(konsole.join("konsolerc"), "[General]\nfoo=bar\n").unwrap();
        match read_current_theme(&roots, "konsole").err().unwrap() {
            ImportError::NotFound(msg) => assert!(msg.contains("default konsole profile")),
            _ => panic!("expected NotFound"),
        }
    }

    #[test]
    fn ini_value_reads_section_scoped_key() {
        let text =
            "[Desktop Entry]\nDefaultProfile=Citrus.profile\n\n[Appearance]\nColorScheme=Citrus\n";
        assert_eq!(
            ini_value(text, "Desktop Entry", "DefaultProfile"),
            Some("Citrus.profile".to_string())
        );
        assert_eq!(
            ini_value(text, "Appearance", "ColorScheme"),
            Some("Citrus".to_string())
        );
        assert_eq!(ini_value(text, "Desktop Entry", "ColorScheme"), None);
        assert_eq!(ini_value(text, "Appearance", "Missing"), None);
        assert_eq!(ini_value(text, "Nope", "ColorScheme"), None);
    }
}
