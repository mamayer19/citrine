#![allow(dead_code)]

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use citrine_core::formats::format_by_id;
use citrine_core::palette::Palette;

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

pub struct Terminal {
    pub id: &'static str,
    pub format_id: &'static str,
    pub file_extension: &'static str,
    pub reload_hint: &'static str,
    pub can_import: bool,
    theme_dir_fn: fn(&Roots) -> PathBuf,
    config_dir_fn: fn(&Roots) -> PathBuf,
}

impl Terminal {
    pub fn theme_dir(&self, roots: &Roots) -> PathBuf {
        (self.theme_dir_fn)(roots)
    }

    pub fn config_dir(&self, roots: &Roots) -> PathBuf {
        (self.config_dir_fn)(roots)
    }

    pub fn present(&self, roots: &Roots) -> bool {
        self.config_dir(roots).is_dir()
    }

    pub fn display_name(&self) -> &'static str {
        format_by_id(self.format_id)
            .expect("terminal format_id is a registered core format")
            .display_name()
    }
}

pub fn terminals() -> &'static [Terminal] {
    TERMINALS
}

pub fn find(id: &str) -> Option<&'static Terminal> {
    TERMINALS.iter().find(|t| t.id == id)
}

static TERMINALS: &[Terminal] = &[
    Terminal {
        id: "ghostty",
        format_id: "ghostty",
        file_extension: "",
        reload_hint: "Reload Ghostty: Cmd+Shift+, (or restart).",
        can_import: true,
        theme_dir_fn: |r| r.config.join("ghostty").join("themes"),
        config_dir_fn: |r| r.config.join("ghostty"),
    },
    Terminal {
        id: "kitty",
        format_id: "kitty",
        file_extension: "conf",
        reload_hint: "Add `include themes/<file>` to kitty.conf, then restart or `kitty @ set-colors -a themes/<file>`.",
        can_import: true,
        theme_dir_fn: |r| r.config.join("kitty").join("themes"),
        config_dir_fn: |r| r.config.join("kitty"),
    },
    Terminal {
        id: "alacritty",
        format_id: "alacritty",
        file_extension: "toml",
        reload_hint: "Add the file to `[general] import` in alacritty.toml (live-reloads).",
        can_import: true,
        theme_dir_fn: |r| r.config.join("alacritty").join("themes"),
        config_dir_fn: |r| r.config.join("alacritty"),
    },
    Terminal {
        id: "wezterm",
        format_id: "wezterm",
        file_extension: "toml",
        reload_hint: "Set `color_scheme` / load the file in wezterm.lua (live-reloads).",
        can_import: false,
        theme_dir_fn: |r| r.config.join("wezterm").join("colors"),
        config_dir_fn: |r| r.config.join("wezterm"),
    },
    Terminal {
        id: "iterm2",
        format_id: "iterm2",
        file_extension: "json",
        reload_hint: "iTerm2 auto-loads the Dynamic Profile; select it in Settings > Profiles.",
        can_import: false,
        theme_dir_fn: |r| {
            r.home
                .join("Library")
                .join("Application Support")
                .join("iTerm2")
                .join("DynamicProfiles")
        },
        config_dir_fn: |r| {
            r.home
                .join("Library")
                .join("Application Support")
                .join("iTerm2")
        },
    },
    Terminal {
        id: "foot",
        format_id: "foot",
        file_extension: "ini",
        reload_hint: "Add `include=themes/<file>` to foot.ini.",
        can_import: true,
        theme_dir_fn: |r| r.config.join("foot").join("themes"),
        config_dir_fn: |r| r.config.join("foot"),
    },
    Terminal {
        id: "rio",
        format_id: "rio",
        file_extension: "toml",
        reload_hint: "Set `theme = \"<name>\"` in rio config.",
        can_import: true,
        theme_dir_fn: |r| r.config.join("rio").join("themes"),
        config_dir_fn: |r| r.config.join("rio"),
    },
    Terminal {
        id: "konsole",
        format_id: "konsole",
        file_extension: "colorscheme",
        reload_hint: "Pick the scheme in Konsole profile > Appearance.",
        can_import: true,
        theme_dir_fn: |r| r.data.join("konsole"),
        config_dir_fn: |r| r.config.join("konsole"),
    },
];

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
    terminal: &Terminal,
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
        let filename = resolve_filename(filename, name, terminal.file_extension)?;
        let path = over.join(&filename);
        if path.parent() != Some(over) {
            return Err(SaveError::InvalidFilename(filename));
        }
        return Ok((over.to_path_buf(), path));
    }

    let theme_dir = terminal.theme_dir(roots);
    let filename = resolve_filename(filename, name, terminal.file_extension)?;
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

    if !terminal.can_import {
        return Err(ImportError::Unsupported(format!(
            "import not supported for {terminal_id}"
        )));
    }

    match terminal_id {
        "ghostty" => ghostty_current(roots),
        "kitty" => parse_active(
            roots.config.join("kitty").join("kitty.conf"),
            "kitty",
            terminal_id,
        ),
        "alacritty" => parse_active(
            roots.config.join("alacritty").join("alacritty.toml"),
            "alacritty",
            terminal_id,
        ),
        "foot" => parse_active(
            roots.config.join("foot").join("foot.ini"),
            "foot",
            terminal_id,
        ),
        "rio" => rio_current(roots),
        "konsole" => konsole_current(roots),
        other => Err(ImportError::Unsupported(format!(
            "import not supported for {other}"
        ))),
    }
}

fn ghostty_current(roots: &Roots) -> Result<Palette, ImportError> {
    let ghostty = roots.config.join("ghostty");
    let config = ghostty.join("config");

    let text = fs::read_to_string(&config)
        .map_err(|_| ImportError::NotFound("no ghostty config found".to_string()))?;
    let theme = last_theme_value(&text).ok_or_else(|| {
        ImportError::NotFound("no active theme set in ghostty config".to_string())
    })?;

    let user_theme = ghostty.join("themes").join(&theme);
    let app_theme =
        PathBuf::from("/Applications/Ghostty.app/Contents/Resources/ghostty/themes").join(&theme);

    let theme_text = fs::read_to_string(&user_theme)
        .or_else(|_| fs::read_to_string(&app_theme))
        .map_err(|_| ImportError::NotFound(format!("theme file not found: {theme}")))?;

    parse_text(&theme_text, "ghostty")
}

fn rio_current(roots: &Roots) -> Result<Palette, ImportError> {
    let rio = roots.config.join("rio");
    let config = rio.join("config.toml");

    let text = fs::read_to_string(&config)
        .map_err(|_| ImportError::NotFound("no rio config found".to_string()))?;

    if let Some(theme) = rio_theme_value(&text) {
        let theme_file = rio.join("themes").join(format!("{theme}.toml"));
        let theme_text = fs::read_to_string(&theme_file)
            .map_err(|_| ImportError::NotFound(format!("theme file not found: {theme}")))?;
        return parse_text(&theme_text, "rio");
    }

    parse_text(&text, "rio")
}

fn rio_theme_value(text: &str) -> Option<String> {
    let raw = last_theme_value(text)?;
    let trimmed = raw.trim_matches(|c| c == '"' || c == '\'');
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn konsole_current(roots: &Roots) -> Result<Palette, ImportError> {
    let data = roots.data.join("konsole");
    let konsolerc = roots.config.join("konsole").join("konsolerc");

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
            assert!(find(t.id).is_some());
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
    fn current_import_unsupported_terminal() {
        let (_tmp, roots) = temp_roots();
        let err = read_current_theme(&roots, "wezterm").err().unwrap();
        assert!(matches!(err, ImportError::Unsupported(_)));
    }

    #[test]
    fn current_unknown_terminal() {
        let (_tmp, roots) = temp_roots();
        let err = read_current_theme(&roots, "nope").err().unwrap();
        assert!(matches!(err, ImportError::UnknownTerminal(_)));
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
        assert_eq!(find("iterm2").unwrap().file_extension, "json");
        assert!(!find("iterm2").unwrap().can_import);
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

    #[test]
    fn current_rio_follows_theme_reference() {
        let (_tmp, roots) = temp_roots();
        let rio = roots.config.join("rio");
        fs::create_dir_all(rio.join("themes")).unwrap();
        fs::write(rio.join("config.toml"), "theme = \"citrus\"\n").unwrap();
        let text = format_by_id("rio").unwrap().export(&Palette::default());
        fs::write(rio.join("themes").join("citrus.toml"), &text).unwrap();

        let palette = read_current_theme(&roots, "rio").ok().unwrap();
        assert_eq!(palette.background.to_hex(), "#f0e5ac");
    }

    #[test]
    fn current_rio_parses_inline_colors_without_theme_ref() {
        let (_tmp, roots) = temp_roots();
        let rio = roots.config.join("rio");
        fs::create_dir_all(&rio).unwrap();
        let text = format_by_id("rio").unwrap().export(&Palette::default());
        fs::write(rio.join("config.toml"), &text).unwrap();

        let palette = read_current_theme(&roots, "rio").ok().unwrap();
        assert_eq!(palette.background.to_hex(), "#f0e5ac");
    }

    #[test]
    fn current_rio_missing_config_is_not_found() {
        let (_tmp, roots) = temp_roots();
        assert!(matches!(
            read_current_theme(&roots, "rio").err().unwrap(),
            ImportError::NotFound(_)
        ));
    }

    #[test]
    fn current_rio_missing_referenced_theme_is_not_found() {
        let (_tmp, roots) = temp_roots();
        let rio = roots.config.join("rio");
        fs::create_dir_all(&rio).unwrap();
        fs::write(rio.join("config.toml"), "theme = \"ghost\"\n").unwrap();
        match read_current_theme(&roots, "rio").err().unwrap() {
            ImportError::NotFound(msg) => assert!(msg.contains("ghost")),
            _ => panic!("expected NotFound"),
        }
    }

    #[test]
    fn current_foot_missing_config_is_not_found() {
        let (_tmp, roots) = temp_roots();
        assert!(matches!(
            read_current_theme(&roots, "foot").err().unwrap(),
            ImportError::NotFound(_)
        ));
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
    fn current_iterm2_import_unsupported() {
        let (_tmp, roots) = temp_roots();
        assert!(matches!(
            read_current_theme(&roots, "iterm2").err().unwrap(),
            ImportError::Unsupported(_)
        ));
    }

    #[test]
    fn rio_theme_value_strips_quotes() {
        assert_eq!(
            rio_theme_value("theme = \"citrus\"\n"),
            Some("citrus".to_string())
        );
        assert_eq!(rio_theme_value("theme = 'x'\n"), Some("x".to_string()));
        assert_eq!(rio_theme_value("theme = \"\"\n"), None);
        assert_eq!(rio_theme_value("font = x\n"), None);
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
