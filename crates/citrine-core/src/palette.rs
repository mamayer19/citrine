use serde::{Deserialize, Serialize};

use crate::color::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Variant {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    Background,
    Foreground,
    Cursor,
    CursorText,
    SelectionBg,
    SelectionFg,
    Ansi(u8),
}

impl Slot {
    pub const NAMED: [Slot; 6] = [
        Slot::Background,
        Slot::Foreground,
        Slot::Cursor,
        Slot::CursorText,
        Slot::SelectionBg,
        Slot::SelectionFg,
    ];

    pub fn all() -> impl Iterator<Item = Slot> {
        Self::NAMED.into_iter().chain((0u8..16).map(Slot::Ansi))
    }

    pub fn label(&self) -> String {
        match self {
            Slot::Background => "Background".to_string(),
            Slot::Foreground => "Foreground".to_string(),
            Slot::Cursor => "Cursor".to_string(),
            Slot::CursorText => "Cursor Text".to_string(),
            Slot::SelectionBg => "Selection Background".to_string(),
            Slot::SelectionFg => "Selection Foreground".to_string(),
            Slot::Ansi(n) => format!("ANSI {n}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    pub name: String,
    pub author: Option<String>,
    pub variant: Variant,
    pub background: Color,
    pub foreground: Color,
    pub cursor: Color,
    pub cursor_text: Color,
    pub selection_background: Color,
    pub selection_foreground: Color,
    pub ansi: [Color; 16],
    pub minimum_contrast: Option<f32>,
}

impl Palette {
    pub fn get(&self, slot: Slot) -> Color {
        match slot {
            Slot::Background => self.background,
            Slot::Foreground => self.foreground,
            Slot::Cursor => self.cursor,
            Slot::CursorText => self.cursor_text,
            Slot::SelectionBg => self.selection_background,
            Slot::SelectionFg => self.selection_foreground,
            Slot::Ansi(n) => self.ansi[n as usize],
        }
    }

    pub fn set(&mut self, slot: Slot, color: Color) {
        match slot {
            Slot::Background => self.background = color,
            Slot::Foreground => self.foreground = color,
            Slot::Cursor => self.cursor = color,
            Slot::CursorText => self.cursor_text = color,
            Slot::SelectionBg => self.selection_background = color,
            Slot::SelectionFg => self.selection_foreground = color,
            Slot::Ansi(n) => self.ansi[n as usize] = color,
        }
    }

    pub fn slots(&self) -> impl Iterator<Item = Slot> {
        Slot::all()
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            name: "Citrus Field (Dawn)".to_string(),
            author: None,
            variant: Variant::Light,
            background: Color::rgb(0xf0, 0xe5, 0xac),
            foreground: Color::rgb(0x5a, 0x53, 0x68),
            cursor: Color::rgb(0xdd, 0x77, 0x14),
            cursor_text: Color::rgb(0x2b, 0x28, 0x20),
            selection_background: Color::rgb(0xe6, 0xcf, 0x88),
            selection_foreground: Color::rgb(0x4b, 0x46, 0x56),
            ansi: [
                Color::rgb(0x4b, 0x46, 0x56),
                Color::rgb(0xb4, 0x4c, 0x37),
                Color::rgb(0x30, 0x80, 0x3f),
                Color::rgb(0x8d, 0x61, 0x0c),
                Color::rgb(0x33, 0x5d, 0xa8),
                Color::rgb(0x8d, 0x47, 0xac),
                Color::rgb(0x1a, 0x84, 0x7f),
                Color::rgb(0xcd, 0xc1, 0xab),
                Color::rgb(0x6f, 0x6a, 0x80),
                Color::rgb(0xc8, 0x5a, 0x44),
                Color::rgb(0x3a, 0x8f, 0x4a),
                Color::rgb(0x9e, 0x70, 0x13),
                Color::rgb(0x3f, 0x6b, 0xb4),
                Color::rgb(0x9d, 0x54, 0xba),
                Color::rgb(0x21, 0x9a, 0x92),
                Color::rgb(0xea, 0xe0, 0xc6),
            ],
            minimum_contrast: Some(3.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_citrus_field_dawn() {
        let p = Palette::default();
        assert_eq!(p.name, "Citrus Field (Dawn)");
        assert_eq!(p.variant, Variant::Light);
        assert_eq!(p.minimum_contrast, Some(3.0));
        assert_eq!(p.background.to_hex(), "#f0e5ac");
        assert_eq!(p.ansi[0].to_hex(), "#4b4656");
        assert_eq!(p.ansi[15].to_hex(), "#eae0c6");
    }

    #[test]
    fn default_palette_json_roundtrips() {
        let p = Palette::default();
        let json = serde_json::to_string_pretty(&p).unwrap();
        let back: Palette = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn variant_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Variant::Light).unwrap(), "\"light\"");
        assert_eq!(serde_json::to_string(&Variant::Dark).unwrap(), "\"dark\"");
    }

    #[test]
    fn get_set_every_slot() {
        let mut p = Palette::default();
        let probe = Color::rgb(1, 2, 3);
        for slot in Palette::default().slots() {
            p.set(slot, probe);
            assert_eq!(p.get(slot), probe, "get/set mismatch for {}", slot.label());
        }
    }

    #[test]
    fn slots_enumerates_all_22() {
        let all: Vec<Slot> = Slot::all().collect();
        assert_eq!(all.len(), 22);
        assert_eq!(all[0], Slot::Background);
        assert_eq!(all[6], Slot::Ansi(0));
        assert_eq!(all[21], Slot::Ansi(15));
    }

    #[test]
    fn ansi_indices_addressable() {
        let mut p = Palette::default();
        for n in 0u8..16 {
            let c = Color::rgb(n, n, n);
            p.set(Slot::Ansi(n), c);
            assert_eq!(p.get(Slot::Ansi(n)), c);
            assert_eq!(p.ansi[n as usize], c);
        }
    }

    #[test]
    fn slot_labels() {
        assert_eq!(Slot::Background.label(), "Background");
        assert_eq!(Slot::Ansi(3).label(), "ANSI 3");
    }

    #[test]
    fn every_slot_label_is_correct() {
        assert_eq!(Slot::Background.label(), "Background");
        assert_eq!(Slot::Foreground.label(), "Foreground");
        assert_eq!(Slot::Cursor.label(), "Cursor");
        assert_eq!(Slot::CursorText.label(), "Cursor Text");
        assert_eq!(Slot::SelectionBg.label(), "Selection Background");
        assert_eq!(Slot::SelectionFg.label(), "Selection Foreground");
        assert_eq!(Slot::Ansi(0).label(), "ANSI 0");
        assert_eq!(Slot::Ansi(15).label(), "ANSI 15");
    }

    #[test]
    fn palette_slots_method_yields_22() {
        assert_eq!(Palette::default().slots().count(), 22);
    }

    #[test]
    fn variant_deserializes_lowercase() {
        assert_eq!(
            serde_json::from_str::<Variant>("\"light\"").unwrap(),
            Variant::Light
        );
        assert_eq!(
            serde_json::from_str::<Variant>("\"dark\"").unwrap(),
            Variant::Dark
        );
        assert!(serde_json::from_str::<Variant>("\"Light\"").is_err());
        assert!(serde_json::from_str::<Variant>("\"blue\"").is_err());
    }
}
