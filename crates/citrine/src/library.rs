use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;

use citrine_core::formats::format_by_id;
use citrine_core::palette::Palette;

use crate::config::{self, Roots};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryEntry {
    pub name: String,
    pub slug: String,
    pub path: PathBuf,
}

#[derive(Debug)]
pub enum LibraryError {
    Io(io::Error),
    NotFound(String),
    Parse(String),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibraryError::Io(e) => write!(f, "{e}"),
            LibraryError::NotFound(name) => write!(f, "no saved palette named '{name}'"),
            LibraryError::Parse(msg) => write!(f, "corrupt palette file: {msg}"),
        }
    }
}

impl std::error::Error for LibraryError {}

impl From<io::Error> for LibraryError {
    fn from(e: io::Error) -> Self {
        LibraryError::Io(e)
    }
}

pub fn palettes_dir(roots: &Roots) -> PathBuf {
    roots.data.join("citrine").join("palettes")
}

pub fn slug(name: &str) -> String {
    config::slugify(name)
}

pub fn save(roots: &Roots, name: &str, palette: &Palette) -> Result<PathBuf, LibraryError> {
    let dir = palettes_dir(roots);
    fs::create_dir_all(&dir)?;

    let mut to_store = palette.clone();
    to_store.name = name.to_string();

    let json = format_by_id("json")
        .expect("json is a registered core format")
        .export(&to_store);

    let path = dir.join(format!("{}.json", slug(name)));
    fs::write(&path, json)?;
    Ok(path)
}

pub fn load(roots: &Roots, name_or_slug: &str) -> Result<Palette, LibraryError> {
    let path = palettes_dir(roots).join(format!("{}.json", slug(name_or_slug)));
    let text =
        fs::read_to_string(&path).map_err(|_| LibraryError::NotFound(name_or_slug.to_string()))?;
    format_by_id("json")
        .expect("json is a registered core format")
        .import(&text)
        .map_err(|e| LibraryError::Parse(e.to_string()))
}

pub fn list(roots: &Roots) -> Vec<LibraryEntry> {
    let dir = palettes_dir(roots);
    let Ok(read_dir) = fs::read_dir(&dir) else {
        return Vec::new();
    };

    let json = format_by_id("json").expect("json is a registered core format");
    let mut entries = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(palette) = json.import(&text) else {
            continue;
        };
        entries.push(LibraryEntry {
            name: palette.name,
            slug: stem.to_string(),
            path,
        });
    }
    entries.sort_by_key(|e| e.name.to_lowercase());
    entries
}

pub fn delete(roots: &Roots, name_or_slug: &str) -> Result<(), LibraryError> {
    let path = palettes_dir(roots).join(format!("{}.json", slug(name_or_slug)));
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Err(LibraryError::NotFound(name_or_slug.to_string()))
        }
        Err(e) => Err(LibraryError::Io(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citrine_core::color::Color;

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
    fn palettes_dir_lives_under_the_data_root() {
        let base = std::env::temp_dir().join("citrine-lib");
        let home = base.clone().into_os_string();
        let roots = Roots::resolve(move |k| match k {
            "CITRINE_HOME" => Some(home.clone()),
            _ => None,
        });
        assert_eq!(
            palettes_dir(&roots),
            base.join(".local")
                .join("share")
                .join("citrine")
                .join("palettes")
        );
    }

    #[test]
    fn xdg_data_home_relocates_the_library() {
        let base = std::env::temp_dir().join("citrine-lib");
        let data = std::env::temp_dir().join("citrine-xdg-data");
        let home = base.into_os_string();
        let xdg = data.clone().into_os_string();
        let roots = Roots::resolve(move |k| match k {
            "CITRINE_HOME" => Some(home.clone()),
            "XDG_DATA_HOME" => Some(xdg.clone()),
            _ => None,
        });
        assert_eq!(palettes_dir(&roots), data.join("citrine").join("palettes"));
    }

    #[test]
    fn list_on_missing_dir_is_empty() {
        let (_tmp, roots) = temp_roots();
        assert!(list(&roots).is_empty());
        assert!(
            !palettes_dir(&roots).exists(),
            "list must not create the dir"
        );
    }

    #[test]
    fn save_creates_dir_and_round_trips_through_list_and_load() {
        let (_tmp, roots) = temp_roots();
        let palette = Palette::default();

        let path = save(&roots, "Citrus Field (Dawn)", &palette).unwrap();
        assert!(path.is_file());
        assert_eq!(path, palettes_dir(&roots).join("citrus-field-dawn.json"));

        let entries = list(&roots);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Citrus Field (Dawn)");
        assert_eq!(entries[0].slug, "citrus-field-dawn");
        assert_eq!(entries[0].path, path);

        let by_name = load(&roots, "Citrus Field (Dawn)").unwrap();
        let by_slug = load(&roots, "citrus-field-dawn").unwrap();
        assert_eq!(by_name, by_slug);
        assert_eq!(by_name, palette);
    }

    #[test]
    fn save_sets_persisted_name_to_the_supplied_name() {
        let (_tmp, roots) = temp_roots();
        let palette = Palette::default();
        save(&roots, "My Renamed Theme", &palette).unwrap();

        let loaded = load(&roots, "My Renamed Theme").unwrap();
        assert_eq!(loaded.name, "My Renamed Theme");
        assert_eq!(loaded.background, palette.background);
        assert_eq!(list(&roots)[0].slug, "my-renamed-theme");
    }

    #[test]
    fn save_overwrites_same_slug() {
        let (_tmp, roots) = temp_roots();
        let a = Palette {
            background: Color::rgb(1, 2, 3),
            ..Palette::default()
        };
        save(&roots, "dupe", &a).unwrap();
        let b = Palette {
            background: Color::rgb(4, 5, 6),
            ..Palette::default()
        };
        save(&roots, "dupe", &b).unwrap();

        assert_eq!(list(&roots).len(), 1, "same slug is one file");
        assert_eq!(
            load(&roots, "dupe").unwrap().background,
            Color::rgb(4, 5, 6)
        );
    }

    #[test]
    fn list_is_sorted_case_insensitively_by_name() {
        let (_tmp, roots) = temp_roots();
        save(&roots, "Zephyr", &Palette::default()).unwrap();
        save(&roots, "amber", &Palette::default()).unwrap();
        save(&roots, "Basil", &Palette::default()).unwrap();

        let names: Vec<String> = list(&roots).into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["amber", "Basil", "Zephyr"]);
    }

    #[test]
    fn delete_removes_the_file() {
        let (_tmp, roots) = temp_roots();
        save(&roots, "gone", &Palette::default()).unwrap();
        assert_eq!(list(&roots).len(), 1);

        delete(&roots, "gone").unwrap();
        assert!(list(&roots).is_empty());
        assert!(matches!(
            delete(&roots, "gone"),
            Err(LibraryError::NotFound(_))
        ));
    }

    #[test]
    fn load_missing_is_not_found() {
        let (_tmp, roots) = temp_roots();
        assert!(matches!(
            load(&roots, "nope").err().unwrap(),
            LibraryError::NotFound(_)
        ));
    }

    #[test]
    fn list_skips_corrupt_and_non_json_files() {
        let (_tmp, roots) = temp_roots();
        save(&roots, "good", &Palette::default()).unwrap();
        let dir = palettes_dir(&roots);
        fs::write(dir.join("broken.json"), "{ not valid json").unwrap();
        fs::write(dir.join("notes.txt"), "ignore me").unwrap();

        let entries = list(&roots);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "good");
    }

    #[test]
    fn load_corrupt_file_is_parse_error() {
        let (_tmp, roots) = temp_roots();
        let dir = palettes_dir(&roots);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("bad.json"), "{ not valid json").unwrap();
        assert!(matches!(
            load(&roots, "bad").err().unwrap(),
            LibraryError::Parse(_)
        ));
    }

    #[test]
    fn slug_is_idempotent_and_reuses_config_slugify() {
        assert_eq!(slug("Citrus Field (Dawn)"), "citrus-field-dawn");
        assert_eq!(slug("citrus-field-dawn"), "citrus-field-dawn");
        assert_eq!(slug("///"), "theme");
    }
}
