use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use citrine_core::formats::format_by_id;
use citrine_core::palette::Palette;
use serde::Serialize;

use crate::config::{ImportError, Roots};

mod alacritty;
mod foot;
mod ghostty;
mod iterm2;
mod kitty;
mod konsole;
mod rio;
mod wezterm;

#[derive(Debug, Serialize)]
pub struct ScaffoldManifest {
    pub files: Vec<PathBuf>,
    pub launch_env: Vec<(String, String)>,
    pub validator: Option<Vec<String>>,
    pub main_config: Option<PathBuf>,
}

pub trait TerminalAdapter: Sync {
    fn id(&self) -> &'static str;

    fn format_id(&self) -> &'static str;

    fn file_extension(&self) -> &'static str;

    fn can_import(&self) -> bool;

    fn reload_hint(&self) -> &'static str;

    fn theme_dir(&self, roots: &Roots) -> PathBuf;

    fn config_dir(&self, roots: &Roots) -> PathBuf;

    fn present(&self, roots: &Roots) -> bool {
        self.config_dir(roots).exists()
    }

    fn current_theme(&self, roots: &Roots) -> Result<Palette, ImportError>;

    fn scaffold(
        &self,
        roots: &Roots,
        dir: &Path,
        palette: &Palette,
        probe_cmd: &str,
    ) -> io::Result<ScaffoldManifest>;

    fn display_name(&self) -> String {
        format_by_id(self.format_id())
            .expect("terminal format_id is a registered core format")
            .display_name()
            .to_string()
    }
}

static ADAPTERS: [&dyn TerminalAdapter; 8] = [
    &ghostty::Ghostty,
    &kitty::Kitty,
    &alacritty::Alacritty,
    &wezterm::WezTerm,
    &iterm2::Iterm2,
    &foot::Foot,
    &rio::Rio,
    &konsole::Konsole,
];

pub fn adapters() -> &'static [&'static dyn TerminalAdapter] {
    &ADAPTERS
}

pub fn adapter_by_id(id: &str) -> Option<&'static dyn TerminalAdapter> {
    adapters().iter().copied().find(|a| a.id() == id)
}

fn parse_active(path: PathBuf, format_id: &str, terminal_id: &str) -> Result<Palette, ImportError> {
    let text = fs::read_to_string(&path)
        .map_err(|_| ImportError::NotFound(format!("no active theme found for {terminal_id}")))?;
    parse_text(&text, format_id)
}

fn parse_text(text: &str, format_id: &str) -> Result<Palette, ImportError> {
    let format = format_by_id(format_id).expect("terminal format_id is a registered core format");
    format
        .import(text)
        .map_err(|e| ImportError::Parse(e.to_string()))
}

fn last_theme_value(text: &str) -> Option<String> {
    let mut found = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            if key.trim() == "theme" {
                let value = value.trim();
                if !value.is_empty() {
                    found = Some(value.to_string());
                }
            }
        }
    }
    found
}

fn export_theme(format_id: &str, palette: &Palette) -> String {
    format_by_id(format_id)
        .expect("terminal format_id is a registered core format")
        .export(palette)
}

fn write_file(path: &Path, content: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}

fn write_run_script(path: &Path, probe_cmd: &str) -> io::Result<()> {
    write_file(path, &format!("#!/bin/sh\n{probe_cmd}\n"))?;
    make_executable(path)
}

#[cfg(unix)]
fn make_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod testkit {
    use std::path::Path;

    use citrine_core::palette::Palette;

    use crate::config::Roots;

    pub fn sentinel() -> Palette {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ci/sentinel.json");
        let text = std::fs::read_to_string(path).expect("read ci/sentinel.json");
        serde_json::from_str(&text).expect("sentinel deserializes to Palette")
    }

    #[cfg(unix)]
    pub fn assert_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path).unwrap().permissions().mode();
        assert_ne!(mode & 0o111, 0, "{path:?} is not executable");
    }

    #[cfg(not(unix))]
    pub fn assert_executable(_path: &Path) {}

    pub fn temp_roots() -> (tempfile::TempDir, Roots) {
        let tmp = tempfile::tempdir().unwrap();
        let roots = Roots {
            home: tmp.path().to_path_buf(),
            config: tmp.path().join(".config"),
            data: tmp.path().join(".local").join("share"),
        };
        (tmp, roots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_has_eight_unique_adapters_with_valid_formats_and_home_scoped_dirs() {
        let list = adapters();
        assert_eq!(list.len(), 8);

        let ids: HashSet<&str> = list.iter().map(|a| a.id()).collect();
        assert_eq!(ids.len(), list.len(), "duplicate adapter id in registry");

        let home = PathBuf::from("/citrine-fake-home");
        let roots = Roots {
            home: home.clone(),
            config: home.join(".config"),
            data: home.join(".local").join("share"),
        };

        for a in list {
            let found = adapter_by_id(a.id()).expect("registered id resolves");
            assert_eq!(found.id(), a.id());
            assert!(
                format_by_id(a.format_id()).is_some(),
                "{} has unknown format_id {}",
                a.id(),
                a.format_id()
            );
            assert!(!a.display_name().is_empty());
            assert!(
                a.theme_dir(&roots).starts_with(&home),
                "{} theme_dir escapes the fake home",
                a.id()
            );
            assert!(
                a.config_dir(&roots).starts_with(&home),
                "{} config_dir escapes the fake home",
                a.id()
            );
        }
        assert!(adapter_by_id("nope").is_none());
    }

    #[test]
    fn scaffold_manifest_serializes_with_stable_keys() {
        let (tmp, roots) = testkit::temp_roots();
        let dir = tmp.path().join("scratch");
        let manifest = adapter_by_id("kitty")
            .unwrap()
            .scaffold(&roots, &dir, &testkit::sentinel(), "exec probe")
            .unwrap();
        let v = serde_json::to_value(&manifest).unwrap();
        assert!(v["files"].is_array());
        assert!(v["launch_env"].is_array());
        assert_eq!(v["validator"][0], "kitty");
        assert_eq!(
            v["main_config"],
            dir.join("kitty.conf").display().to_string()
        );
    }

    #[test]
    fn last_theme_value_picks_last_noncomment_nonempty() {
        assert_eq!(
            last_theme_value("theme = alpha\ntheme = beta\n"),
            Some("beta".to_string())
        );
        assert_eq!(
            last_theme_value("# theme = nope\nfont = x\ntheme = gamma\n"),
            Some("gamma".to_string())
        );
        assert_eq!(last_theme_value("theme =\n"), None);
        assert_eq!(last_theme_value("font-family = Fira\n"), None);
        assert_eq!(last_theme_value(""), None);
    }
}
