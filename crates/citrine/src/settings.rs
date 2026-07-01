use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Roots;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_palette: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_terminal: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub paths: HashMap<String, PathBuf>,
}

pub fn settings_path(roots: &Roots) -> PathBuf {
    roots.config.join("citrine").join("config.toml")
}

impl Settings {
    pub fn load(roots: &Roots) -> Self {
        match fs::read_to_string(settings_path(roots)) {
            Ok(text) => toml::from_str(&text).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self, roots: &Roots) -> io::Result<PathBuf> {
        let path = settings_path(roots);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(&path, text)?;
        Ok(path)
    }

    pub fn apply_override(&self, terminal_id: &str) -> Option<&Path> {
        self.paths.get(terminal_id).map(PathBuf::as_path)
    }

    pub fn set_override(&mut self, terminal_id: &str, path: &str) -> bool {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            self.paths.remove(terminal_id);
            false
        } else {
            self.paths
                .insert(terminal_id.to_string(), PathBuf::from(trimmed));
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_roots() -> (tempfile::TempDir, Roots) {
        let tmp = tempfile::tempdir().unwrap();
        let roots = Roots {
            home: tmp.path().to_path_buf(),
            config: tmp.path().join(".config"),
            data: tmp.path().join(".local").join("share"),
        };
        (tmp, roots)
    }

    #[test]
    fn settings_path_lives_under_the_config_root() {
        let base = std::env::temp_dir().join("citrine-settings");
        let home = base.clone().into_os_string();
        let roots = Roots::resolve(move |k| match k {
            "CITRINE_HOME" => Some(home.clone()),
            _ => None,
        });
        assert_eq!(
            settings_path(&roots),
            base.join(".config").join("citrine").join("config.toml")
        );
    }

    #[test]
    fn load_defaults_when_file_absent() {
        let (_tmp, roots) = temp_roots();
        let s = Settings::load(&roots);
        assert_eq!(s, Settings::default());
        assert!(s.last_palette.is_none());
        assert!(s.last_terminal.is_none());
        assert!(s.paths.is_empty());
        assert!(
            !settings_path(&roots).exists(),
            "load must not create the file"
        );
    }

    #[test]
    fn save_then_load_round_trips() {
        let (_tmp, roots) = temp_roots();
        let mut paths = HashMap::new();
        let custom = std::env::temp_dir().join("kitty").join("themes");
        paths.insert("kitty".to_string(), custom);
        let s = Settings {
            last_palette: Some("Citrus Field (Dawn)".to_string()),
            last_terminal: Some("kitty".to_string()),
            paths,
        };

        let path = s.save(&roots).unwrap();
        assert_eq!(path, settings_path(&roots));
        assert!(path.is_file());

        let back = Settings::load(&roots);
        assert_eq!(back, s);
    }

    #[test]
    fn empty_default_round_trips() {
        let (_tmp, roots) = temp_roots();
        Settings::default().save(&roots).unwrap();
        assert_eq!(Settings::load(&roots), Settings::default());
    }

    #[test]
    fn malformed_toml_degrades_to_defaults() {
        let (_tmp, roots) = temp_roots();
        let path = settings_path(&roots);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "this = = not valid toml").unwrap();
        assert_eq!(Settings::load(&roots), Settings::default());
    }

    #[test]
    fn apply_override_reflects_paths_map() {
        let mut s = Settings::default();
        assert!(s.apply_override("kitty").is_none());
        let themes = std::env::temp_dir().join("themes");
        s.paths.insert("kitty".to_string(), themes.clone());
        assert_eq!(s.apply_override("kitty"), Some(themes.as_path()));
        assert!(s.apply_override("ghostty").is_none());
    }

    #[test]
    fn set_override_sets_and_clears() {
        let mut s = Settings::default();
        let themes = std::env::temp_dir().join("rio-themes");
        let padded = format!("  {}  ", themes.to_str().unwrap());
        assert!(s.set_override("rio", &padded), "returns true when set");
        assert_eq!(s.apply_override("rio"), Some(themes.as_path()));

        assert!(!s.set_override("rio", "   "), "blank clears the override");
        assert!(s.apply_override("rio").is_none());
    }

    #[test]
    fn override_survives_a_save_load_cycle_and_changes_resolved_path() {
        let (_tmp, roots) = temp_roots();
        let mut s = Settings::load(&roots);
        assert!(s.apply_override("kitty").is_none());

        let themes = std::env::temp_dir().join("custom-themes");
        s.set_override("kitty", themes.to_str().unwrap());
        s.save(&roots).unwrap();

        let reloaded = Settings::load(&roots);
        assert_eq!(
            reloaded.apply_override("kitty"),
            Some(themes.as_path()),
            "override persists and is what the apply path would resolve to"
        );
    }
}
