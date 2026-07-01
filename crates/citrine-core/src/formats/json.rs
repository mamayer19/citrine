use crate::formats::{FormatError, ThemeFormat};
use crate::palette::Palette;

pub struct Json;

impl ThemeFormat for Json {
    fn id(&self) -> &'static str {
        "json"
    }

    fn display_name(&self) -> &'static str {
        "JSON"
    }

    fn file_extension(&self) -> &'static str {
        "json"
    }

    fn export(&self, p: &Palette) -> String {
        let mut s = serde_json::to_string_pretty(p).expect("Palette serializes to JSON");
        s.push('\n');
        s
    }

    fn import(&self, text: &str) -> Result<Palette, FormatError> {
        serde_json::from_str::<Palette>(text).map_err(|e| FormatError::parse(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOLDEN: &str = concat!(
        "{\n",
        "  \"name\": \"Citrus Field (Dawn)\",\n",
        "  \"author\": null,\n",
        "  \"variant\": \"light\",\n",
        "  \"background\": \"#f0e5ac\",\n",
        "  \"foreground\": \"#5a5368\",\n",
        "  \"cursor\": \"#dd7714\",\n",
        "  \"cursor_text\": \"#2b2820\",\n",
        "  \"selection_background\": \"#e6cf88\",\n",
        "  \"selection_foreground\": \"#4b4656\",\n",
        "  \"ansi\": [\n",
        "    \"#4b4656\",\n",
        "    \"#b44c37\",\n",
        "    \"#30803f\",\n",
        "    \"#8d610c\",\n",
        "    \"#335da8\",\n",
        "    \"#8d47ac\",\n",
        "    \"#1a847f\",\n",
        "    \"#cdc1ab\",\n",
        "    \"#6f6a80\",\n",
        "    \"#c85a44\",\n",
        "    \"#3a8f4a\",\n",
        "    \"#9e7013\",\n",
        "    \"#3f6bb4\",\n",
        "    \"#9d54ba\",\n",
        "    \"#219a92\",\n",
        "    \"#eae0c6\"\n",
        "  ],\n",
        "  \"minimum_contrast\": 3.0\n",
        "}\n",
    );

    #[test]
    fn golden_default_export() {
        assert_eq!(Json.export(&Palette::default()), GOLDEN);
    }

    #[test]
    fn export_ends_with_trailing_newline() {
        assert!(Json.export(&Palette::default()).ends_with("}\n"));
    }

    #[test]
    fn round_trip_is_identity() {
        let p = Palette::default();
        let back = Json.import(&Json.export(&p)).expect("re-import own export");
        assert_eq!(back, p);
    }

    #[test]
    fn import_accepts_the_golden_text() {
        let p = Json.import(GOLDEN).expect("import golden JSON");
        assert_eq!(p, Palette::default());
    }

    #[test]
    fn import_rejects_malformed_json() {
        let err = Json.import("{ not valid json").unwrap_err();
        assert!(matches!(err, FormatError::Parse(_)));
    }

    #[test]
    fn import_rejects_bad_color_value() {
        let text = r##"{
  "name": "x",
  "author": null,
  "variant": "light",
  "background": "not-a-color",
  "foreground": "#5a5368",
  "cursor": "#dd7714",
  "cursor_text": "#2b2820",
  "selection_background": "#e6cf88",
  "selection_foreground": "#4b4656",
  "ansi": ["#000000","#000000","#000000","#000000","#000000","#000000","#000000","#000000","#000000","#000000","#000000","#000000","#000000","#000000","#000000","#000000"],
  "minimum_contrast": null
}"##;
        let err = Json.import(text).unwrap_err();
        assert!(matches!(err, FormatError::Parse(_)));
    }

    #[test]
    fn import_rejects_empty_input() {
        assert!(matches!(
            Json.import("").unwrap_err(),
            FormatError::Parse(_)
        ));
    }

    #[test]
    fn import_rejects_truncated_json() {
        let full = Json.export(&Palette::default());
        let truncated = &full[..full.len() / 2];
        assert!(matches!(
            Json.import(truncated).unwrap_err(),
            FormatError::Parse(_)
        ));
    }

    #[test]
    fn import_rejects_valid_json_of_wrong_shape() {
        for text in ["[]", "42", "null", "\"a string\""] {
            assert!(
                matches!(Json.import(text).unwrap_err(), FormatError::Parse(_)),
                "expected parse error for {text:?}"
            );
        }
    }

    #[test]
    fn import_rejects_object_missing_required_field() {
        let text = concat!(
            "{\n",
            "  \"name\": \"x\",\n",
            "  \"author\": null,\n",
            "  \"variant\": \"dark\",\n",
            "  \"background\": \"#000000\",\n",
            "  \"foreground\": \"#ffffff\",\n",
            "  \"cursor\": \"#111111\",\n",
            "  \"cursor_text\": \"#222222\",\n",
            "  \"selection_background\": \"#333333\",\n",
            "  \"selection_foreground\": \"#444444\",\n",
            "  \"minimum_contrast\": null\n",
            "}\n",
        );
        assert!(matches!(
            Json.import(text).unwrap_err(),
            FormatError::Parse(_)
        ));
    }

    #[test]
    fn metadata_is_correct() {
        assert_eq!(Json.id(), "json");
        assert_eq!(Json.display_name(), "JSON");
        assert_eq!(Json.file_extension(), "json");
    }
}
