use proptest::prelude::*;

use citrine_core::color::Color;
use citrine_core::contrast::{adjust_for_min_contrast, contrast_ratio};
use citrine_core::formats::{all_formats, Alacritty, Ghostty, Json, Kitty, ThemeFormat, WezTerm};
use citrine_core::helpers::{harmonize, hue_separate, suggest_palette_from_colors};
use citrine_core::palette::{Palette, Slot, Variant};

fn arb_color() -> impl Strategy<Value = Color> {
    (any::<u8>(), any::<u8>(), any::<u8>()).prop_map(|(r, g, b)| Color::rgb(r, g, b))
}

fn arb_name() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[a-zA-Z0-9 _-]{0,16}").expect("valid name regex")
}

fn arb_variant() -> impl Strategy<Value = Variant> {
    prop_oneof![Just(Variant::Light), Just(Variant::Dark)]
}

fn arb_min_contrast() -> impl Strategy<Value = Option<f32>> {
    prop::option::of(1.0f32..21.0)
}

fn arb_palette() -> impl Strategy<Value = Palette> {
    let base = (
        arb_name(),
        arb_variant(),
        (
            arb_color(),
            arb_color(),
            arb_color(),
            arb_color(),
            arb_color(),
            arb_color(),
        ),
        prop::array::uniform16(arb_color()),
        arb_min_contrast(),
    );
    base.prop_map(|(name, variant, named, ansi, minimum_contrast)| {
        let (
            background,
            foreground,
            cursor,
            cursor_text,
            selection_background,
            selection_foreground,
        ) = named;
        Palette {
            name,
            author: None,
            variant,
            background,
            foreground,
            cursor,
            cursor_text,
            selection_background,
            selection_foreground,
            ansi,
            minimum_contrast,
        }
    })
}

fn arb_import_text() -> impl Strategy<Value = String> {
    let line = proptest::string::string_regex("[a-z_]{1,12} *[:=] *[#0-9a-fx\"\\[\\], ]{0,14}")
        .expect("valid config-line regex");
    prop_oneof![
        prop::collection::vec(any::<char>(), 0..80).prop_map(|v| v.into_iter().collect::<String>()),
        prop::collection::vec(line, 0..12).prop_map(|lines| lines.join("\n")),
    ]
}

fn lossless_formats() -> Vec<Box<dyn ThemeFormat>> {
    vec![
        Box::new(Ghostty),
        Box::new(Kitty),
        Box::new(Alacritty),
        Box::new(WezTerm),
        Box::new(Json),
    ]
}

fn channel_deltas(a: Color, b: Color) -> (i32, i32, i32) {
    (
        (a.r as i32 - b.r as i32).abs(),
        (a.g as i32 - b.g as i32).abs(),
        (a.b as i32 - b.b as i32).abs(),
    )
}

proptest! {
    #[test]
    fn hex_roundtrips_exactly(c in arb_color()) {
        let back = Color::from_hex(&c.to_hex()).expect("own hex re-parses");
        prop_assert_eq!(back, c);
    }

    #[test]
    fn hsl_roundtrips_within_one_channel(c in arb_color()) {
        let (h, s, l) = c.to_hsl();
        let back = Color::from_hsl(h, s, l);
        let (dr, dg, db) = channel_deltas(back, c);
        prop_assert!(
            dr <= 1 && dg <= 1 && db <= 1,
            "HSL round-trip drifted for {}: got {} (Δ {dr},{dg},{db})",
            c.to_hex(),
            back.to_hex(),
        );
    }

    #[test]
    fn oklch_roundtrips_within_one_channel(c in arb_color()) {
        let (l, ch, h) = c.to_oklch();
        let back = Color::from_oklch(l, ch, h);
        let (dr, dg, db) = channel_deltas(back, c);
        prop_assert!(
            dr <= 1 && dg <= 1 && db <= 1,
            "OKLCH round-trip drifted for {}: got {} (Δ {dr},{dg},{db})",
            c.to_hex(),
            back.to_hex(),
        );
    }

    #[test]
    fn from_hsl_never_panics(
        h in -1_000.0f64..1_000.0,
        s in -0.5f64..1.5,
        l in -0.5f64..1.5,
    ) {
        let c = Color::from_hsl(h, s, l);
        prop_assert_eq!(Color::from_hex(&c.to_hex()).unwrap(), c);
    }

    #[test]
    fn from_oklch_never_panics(
        l in -0.5f64..1.5,
        c in 0.0f64..0.6,
        h in -1_000.0f64..1_000.0,
    ) {
        let color = Color::from_oklch(l, c, h);
        prop_assert_eq!(Color::from_hex(&color.to_hex()).unwrap(), color);
    }
}

proptest! {
    #[test]
    fn contrast_is_symmetric_and_bounded(a in arb_color(), b in arb_color()) {
        let ab = contrast_ratio(a, b);
        let ba = contrast_ratio(b, a);
        prop_assert!((ab - ba).abs() < 1e-9, "asymmetric: {ab} vs {ba}");
        prop_assert!(
            (1.0 - 1e-9..=21.0 + 1e-9).contains(&ab),
            "ratio {ab} out of [1, 21]",
        );
    }

    #[test]
    fn adjust_meets_target_or_hits_extreme(
        fg in arb_color(),
        bg in arb_color(),
        m in 1.0f64..=21.0,
    ) {
        let out = adjust_for_min_contrast(fg, bg, m);
        let ratio = contrast_ratio(out, bg);
        let black = Color::rgb(0, 0, 0);
        let white = Color::rgb(255, 255, 255);
        prop_assert!(
            ratio + 1e-9 >= m || out == black || out == white,
            "adjust({}, {}, {m}) -> {} has ratio {ratio} < {m} and is not an extreme",
            fg.to_hex(),
            bg.to_hex(),
            out.to_hex(),
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn lossless_formats_roundtrip_all_slots(p in arb_palette()) {
        for f in lossless_formats() {
            let text = f.export(&p);
            let back = match f.import(&text) {
                Ok(b) => b,
                Err(e) => {
                    return Err(TestCaseError::fail(format!(
                        "{} failed to re-import its own export: {e}",
                        f.id()
                    )));
                }
            };
            for slot in Slot::all() {
                prop_assert_eq!(
                    back.get(slot),
                    p.get(slot),
                    "{} lost slot {} on round-trip",
                    f.id(),
                    slot.label()
                );
            }
        }
    }

    #[test]
    fn importers_never_panic_on_arbitrary_text(text in arb_import_text()) {
        for f in all_formats() {
            if let Ok(p) = f.import(&text) {
                prop_assert_eq!(
                    p.ansi.len(),
                    16,
                    "{} imported a palette with a broken ANSI ramp",
                    f.id()
                );
            }
        }
    }
}

proptest! {
    #[test]
    fn hue_separate_preserves_structure(p in arb_palette()) {
        let out = hue_separate(&p);
        prop_assert_eq!(out.ansi.len(), 16);
        prop_assert_eq!(out.background, p.background);
        prop_assert_eq!(out.foreground, p.foreground);
    }

    #[test]
    fn harmonize_preserves_structure(p in arb_palette()) {
        let out = harmonize(&p);
        prop_assert_eq!(out.ansi.len(), 16);
        prop_assert_eq!(out.background, p.background);
        prop_assert_eq!(out.foreground, p.foreground);
    }

    #[test]
    fn suggest_never_panics(colors in prop::collection::vec(arb_color(), 0..24)) {
        let p = suggest_palette_from_colors(&colors);
        prop_assert_eq!(p.ansi.len(), 16);
        prop_assert_eq!(p.name, "Extracted");
    }
}
