use crate::color::Color;

pub fn relative_luminance(color: Color) -> f64 {
    fn linearize(c: u8) -> f64 {
        let c = c as f64 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * linearize(color.r) + 0.7152 * linearize(color.g) + 0.0722 * linearize(color.b)
}

pub fn contrast_ratio(a: Color, b: Color) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

pub fn passes_aa(ratio: f64, large: bool) -> bool {
    if large {
        ratio >= 3.0
    } else {
        ratio >= 4.5
    }
}

pub fn passes_aaa(ratio: f64, large: bool) -> bool {
    if large {
        ratio >= 4.5
    } else {
        ratio >= 7.0
    }
}

pub fn adjust_for_min_contrast(fg: Color, bg: Color, min_ratio: f64) -> Color {
    if contrast_ratio(fg, bg) >= min_ratio {
        return fg;
    }

    let black = Color::rgb(0, 0, 0);
    let white = Color::rgb(255, 255, 255);

    let toward_black = solve_toward(fg, bg, black, min_ratio);
    let toward_white = solve_toward(fg, bg, white, min_ratio);

    match (toward_black, toward_white) {
        (Some((cb, db)), Some((cw, dw))) => {
            if db <= dw {
                cb
            } else {
                cw
            }
        }
        (Some((cb, _)), None) => cb,
        (None, Some((cw, _))) => cw,
        (None, None) => {
            if contrast_ratio(black, bg) >= contrast_ratio(white, bg) {
                black
            } else {
                white
            }
        }
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn solve_toward(fg: Color, bg: Color, endpoint: Color, min_ratio: f64) -> Option<(Color, f64)> {
    if contrast_ratio(endpoint, bg) < min_ratio {
        return None;
    }

    let (fl, fa, fb) = fg.to_oklab();
    let (el, ea, eb) = endpoint.to_oklab();

    let at = |t: f64| Color::from_oklab(lerp(fl, el, t), lerp(fa, ea, t), lerp(fb, eb, t));

    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if contrast_ratio(at(mid), bg) >= min_ratio {
            hi = mid;
        } else {
            lo = mid;
        }
    }

    let color = at(hi);
    let dl = lerp(fl, el, hi) - fl;
    let da = lerp(fa, ea, hi) - fa;
    let db = lerp(fb, eb, hi) - fb;
    let dist = (dl * dl + da * da + db * db).sqrt();
    Some((color, dist))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_on_white_is_21() {
        let r = contrast_ratio(Color::rgb(0, 0, 0), Color::rgb(255, 255, 255));
        assert!((r - 21.0).abs() < 1e-6, "expected 21.0, got {r}");
    }

    #[test]
    fn identical_colors_are_one() {
        let c = Color::rgb(0x5a, 0x53, 0x68);
        assert!((contrast_ratio(c, c) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn ratio_is_symmetric() {
        let a = Color::rgb(0xf0, 0xe5, 0xac);
        let b = Color::rgb(0x5a, 0x53, 0x68);
        assert!((contrast_ratio(a, b) - contrast_ratio(b, a)).abs() < 1e-12);
    }

    #[test]
    fn luminance_bounds() {
        assert!((relative_luminance(Color::rgb(0, 0, 0))).abs() < 1e-12);
        assert!((relative_luminance(Color::rgb(255, 255, 255)) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn aa_aaa_thresholds() {
        assert!(passes_aa(4.5, false));
        assert!(!passes_aa(4.4, false));
        assert!(passes_aa(3.0, true));
        assert!(passes_aaa(7.0, false));
        assert!(!passes_aaa(6.9, false));
        assert!(passes_aaa(4.5, true));
    }

    #[test]
    fn adjust_is_noop_when_sufficient() {
        let bg = Color::rgb(255, 255, 255);
        let fg = Color::rgb(0, 0, 0);
        assert_eq!(adjust_for_min_contrast(fg, bg, 4.5), fg);
    }

    #[test]
    fn adjust_lifts_low_contrast_pair() {
        let bg = Color::rgb(255, 255, 255);
        let fg = Color::rgb(0xf5, 0xf5, 0xf5);
        assert!(contrast_ratio(fg, bg) < 4.5);

        let adjusted = adjust_for_min_contrast(fg, bg, 4.5);
        let ratio = contrast_ratio(adjusted, bg);
        assert!(ratio >= 4.5, "expected >= 4.5 after adjust, got {ratio}");
        assert_ne!(adjusted, fg);
    }

    #[test]
    fn adjust_lifts_low_contrast_on_dark_bg() {
        let bg = Color::rgb(0, 0, 0);
        let fg = Color::rgb(0x22, 0x22, 0x22);
        assert!(contrast_ratio(fg, bg) < 4.5);

        let adjusted = adjust_for_min_contrast(fg, bg, 4.5);
        assert!(contrast_ratio(adjusted, bg) >= 4.5);
    }

    #[test]
    fn luminance_stays_in_unit_interval() {
        for c in [
            Color::rgb(0, 0, 0),
            Color::rgb(255, 255, 255),
            Color::rgb(0xE6, 0xB4, 0x22),
            Color::rgb(0x33, 0x5d, 0xa8),
            Color::rgb(128, 128, 128),
        ] {
            let y = relative_luminance(c);
            assert!((0.0..=1.0).contains(&y), "luminance out of range: {y}");
        }
    }

    #[test]
    fn threshold_boundaries_for_large_text() {
        assert!(passes_aa(3.0, true));
        assert!(!passes_aa(2.999, true));
        assert!(passes_aaa(4.5, true));
        assert!(!passes_aaa(4.499, true));
        assert!(passes_aa(21.0, false));
        assert!(passes_aaa(21.0, false));
    }

    #[test]
    fn adjust_picks_the_nearer_extreme_when_both_reach() {
        let bg = Color::rgb(128, 128, 128);

        let dark_fg = Color::rgb(90, 90, 90);
        assert!(contrast_ratio(dark_fg, bg) < 3.0);
        let out = adjust_for_min_contrast(dark_fg, bg, 3.0);
        assert!(contrast_ratio(out, bg) >= 3.0);
        assert!(
            relative_luminance(out) < relative_luminance(dark_fg),
            "expected the darker direction to win"
        );

        let light_fg = Color::rgb(200, 200, 200);
        assert!(contrast_ratio(light_fg, bg) < 3.0);
        let out = adjust_for_min_contrast(light_fg, bg, 3.0);
        assert!(contrast_ratio(out, bg) >= 3.0);
        assert!(
            relative_luminance(out) > relative_luminance(light_fg),
            "expected the lighter direction to win"
        );
    }

    #[test]
    fn adjust_returns_higher_contrast_extreme_when_target_unreachable() {
        let black = Color::rgb(0, 0, 0);
        let white = Color::rgb(255, 255, 255);

        let mid = Color::rgb(128, 128, 128);
        assert!(contrast_ratio(black, mid) > contrast_ratio(white, mid));
        assert_eq!(adjust_for_min_contrast(mid, mid, 21.0), black);

        let dark = Color::rgb(60, 60, 60);
        assert!(contrast_ratio(white, dark) > contrast_ratio(black, dark));
        assert_eq!(adjust_for_min_contrast(dark, dark, 21.0), white);
    }
}
