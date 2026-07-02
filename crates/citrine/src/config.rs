#![allow(dead_code)]

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use citrine_core::palette::Palette;

use crate::adapters::{adapter_by_id, adapters, TerminalAdapter};

#[derive(Clone, Debug)]
pub struct Roots {
    pub home: PathBuf,
    pub config: PathBuf,
    pub data: PathBuf,
}

impl Roots {
    pub fn from_env() -> Self {
        Self::resolve(|k| std::env::var_os(k))
    }

    pub fn resolve(get: impl Fn(&str) -> Option<OsString>) -> Self {
        let var = |key: &str| get(key).filter(|v| !v.is_empty()).map(PathBuf::from);

        let home = var("CITRINE_HOME")
            .or_else(|| var("HOME"))
            .or_else(|| home_fallback_key().and_then(&var))
            .unwrap_or_else(|| PathBuf::from("."));

        let config = var("XDG_CONFIG_HOME")
            .or_else(|| config_fallback_key().and_then(&var))
            .unwrap_or_else(|| home.join(".config"));

        let data = var("XDG_DATA_HOME")
            .or_else(|| data_fallback_key().and_then(&var))
            .unwrap_or_else(|| home.join(".local").join("share"));

        Roots { home, config, data }
    }
}

#[cfg(windows)]
fn home_fallback_key() -> Option<&'static str> {
    Some("USERPROFILE")
}

#[cfg(not(windows))]
fn home_fallback_key() -> Option<&'static str> {
    None
}

#[cfg(windows)]
fn config_fallback_key() -> Option<&'static str> {
    Some("APPDATA")
}

#[cfg(not(windows))]
fn config_fallback_key() -> Option<&'static str> {
    None
}

#[cfg(windows)]
fn data_fallback_key() -> Option<&'static str> {
    Some("LOCALAPPDATA")
}

#[cfg(not(windows))]
fn data_fallback_key() -> Option<&'static str> {
    None
}

pub fn terminals() -> &'static [&'static dyn TerminalAdapter] {
    adapters()
}

pub fn find(id: &str) -> Option<&'static dyn TerminalAdapter> {
    adapter_by_id(id)
}

pub enum SaveOutcome {
    Written {
        path: PathBuf,
        backup: Option<PathBuf>,
    },
    Conflict {
        path: PathBuf,
    },
}

pub enum SaveError {
    UnknownTerminal(String),
    InvalidFilename(String),
    MissingName,
    Io(io::Error),
}

pub fn save_theme(
    roots: &Roots,
    terminal_id: &str,
    filename: Option<&str>,
    name: Option<&str>,
    content: &str,
    overwrite: bool,
) -> Result<SaveOutcome, SaveError> {
    save_theme_at(None, roots, terminal_id, filename, name, content, overwrite)
}

pub fn save_theme_at(
    override_path: Option<&Path>,
    roots: &Roots,
    terminal_id: &str,
    filename: Option<&str>,
    name: Option<&str>,
    content: &str,
    overwrite: bool,
) -> Result<SaveOutcome, SaveError> {
    let terminal =
        find(terminal_id).ok_or_else(|| SaveError::UnknownTerminal(terminal_id.to_string()))?;

    let (dir, path) = resolve_target(override_path, roots, terminal, filename, name)?;

    fs::create_dir_all(&dir).map_err(SaveError::Io)?;

    let exists = path.exists();
    if exists && !overwrite {
        return Ok(SaveOutcome::Conflict { path });
    }

    let mut backup = None;
    if exists {
        let secs = unix_secs();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bak = path.with_file_name(format!("{name}.bak.{secs}"));
        fs::copy(&path, &bak).map_err(SaveError::Io)?;
        backup = Some(bak);
    }

    atomic_write(&dir, &path, content.as_bytes()).map_err(SaveError::Io)?;

    Ok(SaveOutcome::Written { path, backup })
}

fn resolve_target(
    override_path: Option<&Path>,
    roots: &Roots,
    terminal: &dyn TerminalAdapter,
    filename: Option<&str>,
    name: Option<&str>,
) -> Result<(PathBuf, PathBuf), SaveError> {
    if let Some(over) = override_path {
        if override_is_file(over) {
            let dir = over
                .parent()
                .filter(|d| !d.as_os_str().is_empty())
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            return Ok((dir, over.to_path_buf()));
        }
        let filename = resolve_filename(filename, name, terminal.file_extension())?;
        let path = over.join(&filename);
        if path.parent() != Some(over) {
            return Err(SaveError::InvalidFilename(filename));
        }
        return Ok((over.to_path_buf(), path));
    }

    let theme_dir = terminal.theme_dir(roots);
    let filename = resolve_filename(filename, name, terminal.file_extension())?;
    let path = theme_dir.join(&filename);
    if path.parent() != Some(theme_dir.as_path()) {
        return Err(SaveError::InvalidFilename(filename));
    }
    Ok((theme_dir, path))
}

fn override_is_file(p: &Path) -> bool {
    if p.is_dir() {
        return false;
    }
    if p.is_file() {
        return true;
    }
    p.extension().is_some()
}

fn resolve_filename(
    filename: Option<&str>,
    name: Option<&str>,
    ext: &str,
) -> Result<String, SaveError> {
    if let Some(name) = filename.map(str::trim).filter(|s| !s.is_empty()) {
        if is_unsafe_filename(name) {
            return Err(SaveError::InvalidFilename(name.to_string()));
        }
        return Ok(name.to_string());
    }

    if let Some(name) = name.map(str::trim).filter(|s| !s.is_empty()) {
        let slug = slugify(name);
        let filename = if ext.is_empty() {
            slug
        } else {
            format!("{slug}.{ext}")
        };
        return Ok(filename);
    }

    Err(SaveError::MissingName)
}

fn is_unsafe_filename(name: &str) -> bool {
    name.contains('/') || name.contains('\\') || name.contains("..")
}

pub(crate) fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_dash = false;
    for ch in s.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            out.push(c);
            pending_dash = false;
        } else {
            pending_dash = true;
        }
    }
    if out.is_empty() {
        out.push_str("theme");
    }
    out
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn atomic_write(dir: &Path, target: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = dir.join(format!(
        ".citrine-tmp-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::write(&tmp, bytes)?;
    match fs::rename(&tmp, target) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

pub enum ImportError {
    UnknownTerminal(String),
    Unsupported(String),
    NotFound(String),
    Parse(String),
}

pub fn read_current_theme(roots: &Roots, terminal_id: &str) -> Result<Palette, ImportError> {
    let terminal =
        find(terminal_id).ok_or_else(|| ImportError::UnknownTerminal(terminal_id.to_string()))?;

    if !terminal.can_import() {
        return Err(ImportError::Unsupported(format!(
            "import not supported for {terminal_id}"
        )));
    }

    terminal.current_theme(roots)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |k| {
            pairs
                .iter()
                .find(|(key, _)| *key == k)
                .map(|(_, v)| OsString::from(*v))
        }
    }

    #[test]
    fn citrine_home_overrides_base_and_derives_config() {
        let base = std::env::temp_dir().join("citrine-box");
        let roots = Roots::resolve(env(&[("CITRINE_HOME", base.to_str().unwrap())]));
        assert_eq!(roots.home, base);
        assert_eq!(roots.config, base.join(".config"));

        let ghostty = find("ghostty").unwrap();
        assert_eq!(
            ghostty.theme_dir(&roots),
            base.join(".config").join("ghostty").join("themes")
        );
    }

    #[test]
    fn citrine_home_takes_precedence_over_home() {
        let citrine_home = std::env::temp_dir().join("citrine-box");
        let real_home = std::env::temp_dir().join("citrine-real");
        let roots = Roots::resolve(env(&[
            ("CITRINE_HOME", citrine_home.to_str().unwrap()),
            ("HOME", real_home.to_str().unwrap()),
        ]));
        assert_eq!(roots.home, citrine_home);
    }

    #[test]
    fn xdg_config_home_overrides_derived_config() {
        let base = std::env::temp_dir().join("citrine-box");
        let xdg = std::env::temp_dir().join("citrine-xdg");
        let roots = Roots::resolve(env(&[
            ("CITRINE_HOME", base.to_str().unwrap()),
            ("XDG_CONFIG_HOME", xdg.to_str().unwrap()),
        ]));
        assert_eq!(roots.home, base);
        assert_eq!(roots.config, xdg);
        assert_eq!(
            find("ghostty").unwrap().theme_dir(&roots),
            xdg.join("ghostty").join("themes")
        );
    }

    #[test]
    fn theme_dirs_match_registry_spec() {
        let base = std::env::temp_dir().join("citrine-reg");
        let roots = Roots::resolve(env(&[("CITRINE_HOME", base.to_str().unwrap())]));
        let td = |id: &str| find(id).unwrap().theme_dir(&roots);
        let config = base.join(".config");
        assert_eq!(td("ghostty"), config.join("ghostty").join("themes"));
        assert_eq!(td("kitty"), config.join("kitty").join("themes"));
        assert_eq!(td("alacritty"), config.join("alacritty").join("themes"));
        assert_eq!(td("wezterm"), config.join("wezterm").join("colors"));
        assert_eq!(
            td("iterm2"),
            base.join("Library")
                .join("Application Support")
                .join("iTerm2")
                .join("DynamicProfiles")
        );
        assert_eq!(td("foot"), config.join("foot").join("themes"));
        assert_eq!(td("rio"), config.join("rio").join("themes"));
        assert_eq!(
            td("konsole"),
            base.join(".local").join("share").join("konsole")
        );
    }

    #[test]
    fn present_reflects_config_dir_existence() {
        let tmp = tempfile::tempdir().unwrap();
        let roots = Roots {
            home: tmp.path().to_path_buf(),
            config: tmp.path().join(".config"),
            data: tmp.path().join(".local").join("share"),
        };
        let ghostty = find("ghostty").unwrap();
        assert!(!ghostty.present(&roots), "absent before dir is created");

        std::fs::create_dir_all(roots.config.join("ghostty")).unwrap();
        assert!(ghostty.present(&roots), "present once dir exists");
    }

    #[test]
    fn every_terminal_id_and_format_resolve() {
        assert_eq!(terminals().len(), 8);
        for t in terminals() {
            assert!(find(t.id()).is_some());
            assert!(!t.display_name().is_empty());
        }
        assert!(find("nope").is_none());
    }

    #[test]
    fn home_used_when_no_citrine_home() {
        let real_home = std::env::temp_dir().join("citrine-real");
        let roots = Roots::resolve(env(&[("HOME", real_home.to_str().unwrap())]));
        assert_eq!(roots.home, real_home);
        assert_eq!(roots.config, real_home.join(".config"));
    }

    #[test]
    fn empty_citrine_home_falls_back_to_home() {
        let real_home = std::env::temp_dir().join("citrine-real");
        let roots = Roots::resolve(env(&[
            ("CITRINE_HOME", ""),
            ("HOME", real_home.to_str().unwrap()),
        ]));
        assert_eq!(roots.home, real_home);
    }

    #[test]
    fn all_empty_env_defaults_to_dot() {
        let roots = Roots::resolve(env(&[
            ("CITRINE_HOME", ""),
            ("HOME", ""),
            ("XDG_CONFIG_HOME", ""),
        ]));
        assert_eq!(roots.home, PathBuf::from("."));
        assert_eq!(roots.config, PathBuf::from(".").join(".config"));
    }

    #[test]
    fn missing_env_defaults_to_dot() {
        let roots = Roots::resolve(env(&[]));
        assert_eq!(roots.home, PathBuf::from("."));
        assert_eq!(roots.config, PathBuf::from(".").join(".config"));
    }

    #[test]
    fn from_env_reads_process_environment_without_panicking() {
        let roots = Roots::from_env();
        assert!(!roots.home.as_os_str().is_empty());
        assert!(!roots.config.as_os_str().is_empty());
    }

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
    fn slugify_maps_to_safe_charset() {
        assert_eq!(slugify("Citrus Field (Dawn)"), "citrus-field-dawn");
        assert_eq!(slugify("  Rosé  Pine  "), "ros-pine");
        assert_eq!(slugify("///"), "theme");
        assert_eq!(slugify("A_B.C"), "a-b-c");
    }

    #[test]
    fn unsafe_filenames_detected() {
        assert!(is_unsafe_filename("../evil"));
        assert!(is_unsafe_filename("a/b"));
        assert!(is_unsafe_filename("a\\b"));
        assert!(!is_unsafe_filename("citrus-field-dawn"));
    }

    #[test]
    fn save_creates_file_and_theme_dir() {
        let (_tmp, roots) = temp_roots();
        let theme_dir = find("ghostty").unwrap().theme_dir(&roots);
        assert!(!theme_dir.exists());

        let outcome = save_theme(
            &roots,
            "ghostty",
            Some("citrus"),
            None,
            "background = #f0e5ac\n",
            false,
        )
        .unwrap_or_else(|_| panic!("save failed"));
        match outcome {
            SaveOutcome::Written { path, backup } => {
                assert!(theme_dir.is_dir(), "theme dir created");
                assert_eq!(path, theme_dir.join("citrus"));
                assert!(backup.is_none());
                assert_eq!(fs::read_to_string(&path).unwrap(), "background = #f0e5ac\n");
            }
            SaveOutcome::Conflict { .. } => panic!("unexpected conflict"),
        }
    }

    #[test]
    fn save_without_overwrite_conflicts() {
        let (_tmp, roots) = temp_roots();
        save_theme(&roots, "ghostty", Some("citrus"), None, "A", true)
            .ok()
            .unwrap();

        let outcome = save_theme(&roots, "ghostty", Some("citrus"), None, "B", false)
            .ok()
            .unwrap();
        match outcome {
            SaveOutcome::Conflict { path } => {
                assert_eq!(
                    fs::read_to_string(&path).unwrap(),
                    "A",
                    "original preserved"
                );
            }
            SaveOutcome::Written { .. } => panic!("expected conflict"),
        }
    }

    #[test]
    fn save_with_overwrite_backs_up_old_and_writes_new() {
        let (_tmp, roots) = temp_roots();
        save_theme(&roots, "ghostty", Some("citrus"), None, "OLD", true)
            .ok()
            .unwrap();

        let outcome = save_theme(&roots, "ghostty", Some("citrus"), None, "NEW", true)
            .ok()
            .unwrap();

        match outcome {
            SaveOutcome::Written { path, backup } => {
                assert_eq!(
                    fs::read_to_string(&path).unwrap(),
                    "NEW",
                    "new content written"
                );
                let backup = backup.expect("backup created on overwrite");
                assert!(
                    backup
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("citrus.bak."),
                    "backup is timestamped: {backup:?}"
                );
                assert_eq!(
                    fs::read_to_string(&backup).unwrap(),
                    "OLD",
                    "backup holds old content"
                );
            }
            SaveOutcome::Conflict { .. } => panic!("unexpected conflict"),
        }
    }

    #[test]
    fn save_rejects_path_traversal() {
        let (_tmp, roots) = temp_roots();
        for bad in ["../evil", "a/b", "..", "sub/../../x"] {
            assert!(
                matches!(
                    save_theme(&roots, "ghostty", Some(bad), None, "x", true),
                    Err(SaveError::InvalidFilename(_))
                ),
                "expected rejection for {bad:?}"
            );
        }
    }

    #[test]
    fn save_derives_filename_from_name_with_extension() {
        let (_tmp, roots) = temp_roots();
        let outcome = save_theme(
            &roots,
            "kitty",
            None,
            Some("Citrus Field (Dawn)"),
            "background #f0e5ac\n",
            false,
        )
        .ok()
        .unwrap();
        match outcome {
            SaveOutcome::Written { path, .. } => {
                assert_eq!(path.file_name().unwrap(), "citrus-field-dawn.conf");
            }
            SaveOutcome::Conflict { .. } => panic!("unexpected conflict"),
        }
    }

    #[test]
    fn save_unknown_terminal_errors() {
        let (_tmp, roots) = temp_roots();
        assert!(matches!(
            save_theme(&roots, "nope", Some("x"), None, "y", true),
            Err(SaveError::UnknownTerminal(_))
        ));
    }

    #[test]
    fn save_missing_name_errors() {
        let (_tmp, roots) = temp_roots();
        assert!(matches!(
            save_theme(&roots, "kitty", None, None, "y", true),
            Err(SaveError::MissingName)
        ));
    }

    #[test]
    fn override_is_file_classifies_by_extension_and_kind() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(override_is_file(&tmp.path().join("themes").join("t.conf")));
        assert!(!override_is_file(&tmp.path().join("themes")));
        let real_dir = tmp.path().join("dir.conf");
        std::fs::create_dir_all(&real_dir).unwrap();
        assert!(!override_is_file(&real_dir), "existing dir is not a file");
        let real_file = tmp.path().join("noext");
        fs::write(&real_file, "x").unwrap();
        assert!(override_is_file(&real_file), "existing file is a file");
    }

    #[test]
    fn save_at_directory_override_lands_in_override_dir() {
        let (_tmp, roots) = temp_roots();
        let over = roots.home.join("custom").join("themes");
        let outcome = save_theme_at(
            Some(over.as_path()),
            &roots,
            "kitty",
            None,
            Some("Citrus Field (Dawn)"),
            "background #f0e5ac\n",
            false,
        )
        .ok()
        .unwrap();
        match outcome {
            SaveOutcome::Written { path, backup } => {
                assert_eq!(path, over.join("citrus-field-dawn.conf"));
                assert!(over.is_dir(), "override dir created");
                assert!(backup.is_none());
                assert!(!find("kitty").unwrap().theme_dir(&roots).exists());
            }
            SaveOutcome::Conflict { .. } => panic!("unexpected conflict"),
        }
    }

    #[test]
    fn save_at_file_override_writes_verbatim_and_backs_up() {
        let (_tmp, roots) = temp_roots();
        let file = roots.home.join("dots").join("my-theme.conf");

        let first = save_theme_at(
            Some(file.as_path()),
            &roots,
            "kitty",
            None,
            Some("ignored-name"),
            "OLD",
            true,
        )
        .ok()
        .unwrap();
        match first {
            SaveOutcome::Written { path, backup } => {
                assert_eq!(path, file);
                assert!(backup.is_none());
                assert_eq!(fs::read_to_string(&file).unwrap(), "OLD");
            }
            SaveOutcome::Conflict { .. } => panic!("unexpected conflict"),
        }

        let second = save_theme_at(
            Some(file.as_path()),
            &roots,
            "kitty",
            None,
            Some("ignored-name"),
            "NEW",
            true,
        )
        .ok()
        .unwrap();
        match second {
            SaveOutcome::Written { path, backup } => {
                assert_eq!(path, file);
                assert_eq!(fs::read_to_string(&file).unwrap(), "NEW");
                let backup = backup.expect("backup on overwrite");
                assert!(backup
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("my-theme.conf.bak."));
                assert_eq!(fs::read_to_string(&backup).unwrap(), "OLD");
            }
            SaveOutcome::Conflict { .. } => panic!("unexpected conflict"),
        }
    }

    #[test]
    fn save_at_directory_override_conflicts_without_overwrite() {
        let (_tmp, roots) = temp_roots();
        let over = roots.home.join("themes");
        save_theme_at(
            Some(over.as_path()),
            &roots,
            "kitty",
            None,
            Some("c"),
            "A",
            true,
        )
        .ok()
        .unwrap();
        let outcome = save_theme_at(
            Some(over.as_path()),
            &roots,
            "kitty",
            None,
            Some("c"),
            "B",
            false,
        )
        .ok()
        .unwrap();
        match outcome {
            SaveOutcome::Conflict { path } => {
                assert_eq!(
                    fs::read_to_string(&path).unwrap(),
                    "A",
                    "original preserved"
                );
            }
            SaveOutcome::Written { .. } => panic!("expected conflict"),
        }
    }

    #[test]
    fn current_unknown_terminal() {
        let (_tmp, roots) = temp_roots();
        let err = read_current_theme(&roots, "nope").err().unwrap();
        assert!(matches!(err, ImportError::UnknownTerminal(_)));
    }

    #[test]
    fn save_foot_writes_ini_into_themes_dir() {
        let (_tmp, roots) = temp_roots();
        let theme_dir = find("foot").unwrap().theme_dir(&roots);
        let outcome = save_theme(
            &roots,
            "foot",
            None,
            Some("Citrus Field (Dawn)"),
            "[colors]\nbackground=f0e5ac\n",
            false,
        )
        .ok()
        .unwrap();
        match outcome {
            SaveOutcome::Written { path, backup } => {
                assert!(theme_dir.is_dir(), "theme dir created");
                assert_eq!(path, theme_dir.join("citrus-field-dawn.ini"));
                assert!(backup.is_none());
                assert_eq!(
                    fs::read_to_string(&path).unwrap(),
                    "[colors]\nbackground=f0e5ac\n"
                );
            }
            SaveOutcome::Conflict { .. } => panic!("unexpected conflict"),
        }
    }

    #[test]
    fn save_rio_writes_toml_into_themes_dir() {
        let (_tmp, roots) = temp_roots();
        let theme_dir = find("rio").unwrap().theme_dir(&roots);
        let outcome = save_theme(
            &roots,
            "rio",
            None,
            Some("Citrus"),
            "[colors]\nbackground = \"#f0e5ac\"\n",
            false,
        )
        .ok()
        .unwrap();
        match outcome {
            SaveOutcome::Written { path, .. } => {
                assert_eq!(path, theme_dir.join("citrus.toml"));
                assert!(path.is_file());
            }
            SaveOutcome::Conflict { .. } => panic!("unexpected conflict"),
        }
    }

    #[test]
    fn save_konsole_writes_colorscheme_into_data_dir() {
        let (_tmp, roots) = temp_roots();
        let theme_dir = find("konsole").unwrap().theme_dir(&roots);
        assert_eq!(theme_dir, roots.data.join("konsole"));
        let outcome = save_theme(
            &roots,
            "konsole",
            None,
            Some("Citrus"),
            "[General]\n",
            false,
        )
        .ok()
        .unwrap();
        match outcome {
            SaveOutcome::Written { path, .. } => {
                assert!(theme_dir.is_dir(), "data theme dir created");
                assert_eq!(path, theme_dir.join("citrus.colorscheme"));
            }
            SaveOutcome::Conflict { .. } => panic!("unexpected conflict"),
        }
    }
}
