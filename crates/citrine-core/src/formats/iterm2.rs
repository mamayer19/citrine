use serde_json::{json, Value};

use crate::color::Color;
use crate::formats::ThemeFormat;
use crate::palette::Palette;

pub struct Iterm2;

fn color_value(c: Color) -> Value {
    json!({
        "Color Space": "sRGB",
        "Red Component": c.r() as f64 / 255.0,
        "Green Component": c.g() as f64 / 255.0,
        "Blue Component": c.b() as f64 / 255.0,
    })
}

fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut pending_sep = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_sep && !out.is_empty() {
                out.push('-');
            }
            pending_sep = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            pending_sep = true;
        }
    }
    out
}

fn guid_for(name: &str) -> String {
    format!("citrine-{}", slug(name))
}

impl ThemeFormat for Iterm2 {
    fn id(&self) -> &'static str {
        "iterm2"
    }

    fn display_name(&self) -> &'static str {
        "iTerm2 (Dynamic Profile)"
    }

    fn file_extension(&self) -> &'static str {
        "json"
    }

    fn export(&self, p: &Palette) -> String {
        let profile = json!({
            "Name": p.name,
            "Guid": guid_for(&p.name),
            "Dynamic Profile Parent Name": "Default",
            "Use Separate Colors for Light and Dark Mode": false,
            "Ansi 0 Color": color_value(p.ansi[0]),
            "Ansi 1 Color": color_value(p.ansi[1]),
            "Ansi 2 Color": color_value(p.ansi[2]),
            "Ansi 3 Color": color_value(p.ansi[3]),
            "Ansi 4 Color": color_value(p.ansi[4]),
            "Ansi 5 Color": color_value(p.ansi[5]),
            "Ansi 6 Color": color_value(p.ansi[6]),
            "Ansi 7 Color": color_value(p.ansi[7]),
            "Ansi 8 Color": color_value(p.ansi[8]),
            "Ansi 9 Color": color_value(p.ansi[9]),
            "Ansi 10 Color": color_value(p.ansi[10]),
            "Ansi 11 Color": color_value(p.ansi[11]),
            "Ansi 12 Color": color_value(p.ansi[12]),
            "Ansi 13 Color": color_value(p.ansi[13]),
            "Ansi 14 Color": color_value(p.ansi[14]),
            "Ansi 15 Color": color_value(p.ansi[15]),
            "Background Color": color_value(p.background),
            "Foreground Color": color_value(p.foreground),
            "Cursor Color": color_value(p.cursor),
            "Cursor Text Color": color_value(p.cursor_text),
            "Selection Color": color_value(p.selection_background),
            "Selected Text Color": color_value(p.selection_foreground),
        });

        let doc = json!({ "Profiles": [profile] });
        let mut s = serde_json::to_string_pretty(&doc).expect("iTerm2 profile serializes to JSON");
        s.push('\n');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_channel(component: f64) -> u8 {
        (component * 255.0).round() as u8
    }

    #[test]
    fn slug_normalizes_names() {
        assert_eq!(slug("Citrus Field (Dawn)"), "citrus-field-dawn");
        assert_eq!(slug("  Nord   Aurora  "), "nord-aurora");
        assert_eq!(slug("Solarized_Dark v2"), "solarized-dark-v2");
        assert_eq!(slug("ALLCAPS"), "allcaps");
    }

    #[test]
    fn metadata_is_correct() {
        assert_eq!(Iterm2.id(), "iterm2");
        assert_eq!(Iterm2.display_name(), "iTerm2 (Dynamic Profile)");
        assert_eq!(Iterm2.file_extension(), "json");
    }

    #[test]
    fn export_is_valid_json_and_nonempty() {
        let out = Iterm2.export(&Palette::default());
        assert!(!out.is_empty());
        assert!(out.ends_with("}\n"));
        let v: Value = serde_json::from_str(&out).expect("export is valid JSON");
        let profiles = v["Profiles"].as_array().expect("Profiles is an array");
        assert_eq!(profiles.len(), 1);
    }

    #[test]
    fn profile_name_and_stable_slugged_guid() {
        let p = Palette::default();
        let v: Value = serde_json::from_str(&Iterm2.export(&p)).unwrap();
        let profile = &v["Profiles"][0];

        assert_eq!(profile["Name"], "Citrus Field (Dawn)");
        assert_eq!(profile["Guid"], "citrine-citrus-field-dawn");
        assert_eq!(profile["Dynamic Profile Parent Name"], "Default");
    }

    #[test]
    fn guid_is_stable_per_name_and_distinct_across_names() {
        let p = Palette::default();
        let a: Value = serde_json::from_str(&Iterm2.export(&p)).unwrap();
        let b: Value = serde_json::from_str(&Iterm2.export(&p)).unwrap();
        assert_eq!(a["Profiles"][0]["Guid"], b["Profiles"][0]["Guid"]);

        let renamed = Palette {
            name: "Midnight Grove".to_string(),
            ..Palette::default()
        };
        let c: Value = serde_json::from_str(&Iterm2.export(&renamed)).unwrap();
        assert_eq!(c["Profiles"][0]["Guid"], "citrine-midnight-grove");
        assert_ne!(a["Profiles"][0]["Guid"], c["Profiles"][0]["Guid"]);
    }

    #[test]
    fn color_objects_carry_srgb_space_and_roundtripping_components() {
        let p = Palette::default();
        let v: Value = serde_json::from_str(&Iterm2.export(&p)).unwrap();
        let profile = &v["Profiles"][0];

        for key in [
            "Ansi 0 Color",
            "Background Color",
            "Foreground Color",
            "Cursor Color",
            "Cursor Text Color",
            "Selection Color",
            "Selected Text Color",
        ] {
            assert_eq!(profile[key]["Color Space"], "sRGB", "space for {key}");
        }

        let bg = &profile["Background Color"];
        assert_eq!(to_channel(bg["Red Component"].as_f64().unwrap()), 0xf0);
        assert_eq!(to_channel(bg["Green Component"].as_f64().unwrap()), 0xe5);
        assert_eq!(to_channel(bg["Blue Component"].as_f64().unwrap()), 0xac);

        let ansi0 = &profile["Ansi 0 Color"];
        assert_eq!(to_channel(ansi0["Red Component"].as_f64().unwrap()), 0x4b);
        assert_eq!(to_channel(ansi0["Green Component"].as_f64().unwrap()), 0x46);
        assert_eq!(to_channel(ansi0["Blue Component"].as_f64().unwrap()), 0x56);

        let ansi15 = &profile["Ansi 15 Color"];
        assert_eq!(to_channel(ansi15["Red Component"].as_f64().unwrap()), 0xea);
        assert_eq!(
            to_channel(ansi15["Green Component"].as_f64().unwrap()),
            0xe0
        );
        assert_eq!(to_channel(ansi15["Blue Component"].as_f64().unwrap()), 0xc6);
    }

    #[test]
    fn all_sixteen_ansi_slots_and_named_colors_present() {
        let p = Palette::default();
        let v: Value = serde_json::from_str(&Iterm2.export(&p)).unwrap();
        let profile = &v["Profiles"][0];

        for n in 0u8..16 {
            let key = format!("Ansi {n} Color");
            let obj = &profile[&key];
            assert!(obj.is_object(), "missing {key}");
            let src = p.ansi[n as usize];
            assert_eq!(to_channel(obj["Red Component"].as_f64().unwrap()), src.r());
            assert_eq!(
                to_channel(obj["Green Component"].as_f64().unwrap()),
                src.g()
            );
            assert_eq!(to_channel(obj["Blue Component"].as_f64().unwrap()), src.b());
        }

        let sel_bg = &profile["Selection Color"];
        assert_eq!(
            to_channel(sel_bg["Red Component"].as_f64().unwrap()),
            p.selection_background.r()
        );
        let sel_fg = &profile["Selected Text Color"];
        assert_eq!(
            to_channel(sel_fg["Red Component"].as_f64().unwrap()),
            p.selection_foreground.r()
        );
    }
}
