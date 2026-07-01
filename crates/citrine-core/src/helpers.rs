use crate::color::Color;
use crate::contrast::{adjust_for_min_contrast, contrast_ratio, relative_luminance};
use crate::palette::{Palette, Variant};

fn ang_diff(from: f64, to: f64) -> f64 {
    let d = (to - from).rem_euclid(360.0);
    if d > 180.0 {
        d - 360.0
    } else {
        d
    }
}

fn median(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

fn mix_oklab(a: Color, b: Color, t: f64) -> Color {
    let (al, aa, ab) = a.to_oklab();
    let (bl, ba, bb) = b.to_oklab();
    Color::from_oklab(al + (bl - al) * t, aa + (ba - aa) * t, ab + (bb - ab) * t)
}

pub fn hue_separate(p: &Palette) -> Palette {
    const TARGETS: [f64; 6] = [30.0, 150.0, 90.0, 270.0, 330.0, 210.0];
    const NUDGE: f64 = 0.5;

    let mut out = p.clone();
    let mut new_hues = [0.0f64; 6];

    for (idx, target) in TARGETS.iter().enumerate() {
        let slot = idx + 1;
        let (l, c, h) = out.ansi[slot].to_oklch();
        let nh = (h + NUDGE * ang_diff(h, *target)).rem_euclid(360.0);
        new_hues[idx] = nh;
        out.ansi[slot] = Color::from_oklch(l, c, nh);
    }

    for (idx, nh) in new_hues.iter().enumerate() {
        let bright = idx + 9;
        let (l, c, _h) = out.ansi[bright].to_oklch();
        out.ansi[bright] = Color::from_oklch(l, c, *nh);
    }

    out
}

pub fn harmonize(p: &Palette) -> Palette {
    let mut out = p.clone();

    let (ml, mc) = median_lc(&out.ansi[1..=6]);
    for color in out.ansi[1..=6].iter_mut() {
        let (_l, _c, h) = color.to_oklch();
        *color = Color::from_oklch(ml, mc, h);
    }

    let (mlb, mcb) = median_lc(&out.ansi[9..=14]);
    for color in out.ansi[9..=14].iter_mut() {
        let (_l, _c, h) = color.to_oklch();
        *color = Color::from_oklch(mlb, mcb, h);
    }

    out
}

fn median_lc(colors: &[Color]) -> (f64, f64) {
    let mut ls: Vec<f64> = colors.iter().map(|c| c.to_oklch().0).collect();
    let mut cs: Vec<f64> = colors.iter().map(|c| c.to_oklch().1).collect();
    (median(&mut ls), median(&mut cs))
}

pub fn suggest_palette_from_colors(colors: &[Color]) -> Palette {
    let mut out = Palette {
        name: "Extracted".to_string(),
        author: None,
        ..Palette::default()
    };

    if colors.is_empty() {
        return out;
    }

    let mean_lum = colors.iter().map(|c| relative_luminance(*c)).sum::<f64>() / colors.len() as f64;
    let darkest = *colors
        .iter()
        .min_by(|a, b| relative_luminance(**a).total_cmp(&relative_luminance(**b)))
        .expect("non-empty");
    let lightest = *colors
        .iter()
        .max_by(|a, b| relative_luminance(**a).total_cmp(&relative_luminance(**b)))
        .expect("non-empty");

    let (bg, dark_theme) = if mean_lum >= 0.5 {
        (lightest, false)
    } else {
        (darkest, true)
    };
    out.variant = if dark_theme {
        Variant::Dark
    } else {
        Variant::Light
    };
    out.background = bg;

    let fg_candidate = *colors
        .iter()
        .max_by(|a, b| contrast_ratio(**a, bg).total_cmp(&contrast_ratio(**b, bg)))
        .expect("non-empty");
    let fg = adjust_for_min_contrast(fg_candidate, bg, 4.5);
    out.foreground = fg;

    const ROLE_HUES: [f64; 6] = [29.0, 142.0, 100.0, 264.0, 328.0, 194.0];
    const CHROMA_MIN: f64 = 0.03;

    let mut chromatic: Vec<(f64, Color)> = colors
        .iter()
        .map(|c| (c.to_oklch().1, *c))
        .filter(|(chroma, _)| *chroma >= CHROMA_MIN)
        .collect();
    chromatic.sort_by(|a, b| b.0.total_cmp(&a.0));

    let mut assigned: [Option<Color>; 6] = [None; 6];
    for (_chroma, color) in &chromatic {
        let (_l, _c, hue) = color.to_oklch();
        let mut best_role = 0usize;
        let mut best_dist = f64::MAX;
        for (role, target) in ROLE_HUES.iter().enumerate() {
            let d = ang_diff(hue, *target).abs();
            if d < best_dist {
                best_dist = d;
                best_role = role;
            }
        }
        if assigned[best_role].is_none() {
            assigned[best_role] = Some(*color);
        }
    }

    for (role, slot) in assigned.iter().enumerate() {
        if let Some(c) = slot {
            out.ansi[role + 1] = *c;
        }
        let (l, c, h) = out.ansi[role + 1].to_oklch();
        let bright = Color::from_oklch((l + 0.10).min(0.98), (c * 1.05).min(0.37), h);
        out.ansi[role + 9] = bright;
    }

    let (dark_end, light_end) = if relative_luminance(bg) <= relative_luminance(fg) {
        (bg, fg)
    } else {
        (fg, bg)
    };
    out.ansi[0] = mix_oklab(dark_end, light_end, 0.08);
    out.ansi[8] = mix_oklab(dark_end, light_end, 0.30);
    out.ansi[7] = mix_oklab(dark_end, light_end, 0.80);
    out.ansi[15] = mix_oklab(dark_end, light_end, 0.96);

    out.cursor = chromatic.first().map(|(_, c)| *c).unwrap_or(fg);
    out.cursor_text = bg;
    out.selection_background = mix_oklab(dark_end, light_end, 0.22);
    out.selection_foreground = fg;

    out
}

#[cfg(feature = "image")]
pub fn extract_palette(bytes: &[u8], k: usize) -> Result<Vec<Color>, String> {
    if k == 0 {
        return Ok(Vec::new());
    }

    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let rgb = img.to_rgb8();

    const MAX_SIDE: u32 = 128;
    let (w, h) = rgb.dimensions();
    let pixels: Vec<[u8; 3]> = if w > MAX_SIDE || h > MAX_SIDE {
        let scale = MAX_SIDE as f64 / w.max(h) as f64;
        let nw = ((w as f64 * scale).round() as u32).max(1);
        let nh = ((h as f64 * scale).round() as u32).max(1);
        let small = image::imageops::thumbnail(&rgb, nw, nh);
        small.pixels().map(|p| [p[0], p[1], p[2]]).collect()
    } else {
        rgb.pixels().map(|p| [p[0], p[1], p[2]]).collect()
    };
    if pixels.is_empty() {
        return Ok(Vec::new());
    }

    let buckets = median_cut(pixels, k);
    let mut reps: Vec<(usize, Color)> = buckets.iter().map(|b| (b.len(), average(b))).collect();
    reps.sort_by_key(|b| std::cmp::Reverse(b.0));
    Ok(reps.into_iter().map(|(_, c)| c).collect())
}

#[cfg(feature = "image")]
fn median_cut(pixels: Vec<[u8; 3]>, k: usize) -> Vec<Vec<[u8; 3]>> {
    let mut buckets: Vec<Vec<[u8; 3]>> = vec![pixels];

    while buckets.len() < k {
        let mut target = None;
        let mut widest = 0i32;
        for (i, b) in buckets.iter().enumerate() {
            if b.len() < 2 {
                continue;
            }
            let (_chan, range) = widest_channel(b);
            if range > widest {
                widest = range;
                target = Some(i);
            }
        }

        let Some(i) = target else {
            break;
        };

        let mut bucket = buckets.remove(i);
        let (chan, _) = widest_channel(&bucket);
        bucket.sort_by_key(|p| p[chan]);
        let mid = bucket.len() / 2;
        let upper = bucket.split_off(mid);
        buckets.push(bucket);
        buckets.push(upper);
    }

    buckets
}

#[cfg(feature = "image")]
fn widest_channel(pixels: &[[u8; 3]]) -> (usize, i32) {
    let mut lo = [255i32; 3];
    let mut hi = [0i32; 3];
    for p in pixels {
        for c in 0..3 {
            let v = p[c] as i32;
            lo[c] = lo[c].min(v);
            hi[c] = hi[c].max(v);
        }
    }
    let mut chan = 0usize;
    let mut range = -1i32;
    for c in 0..3 {
        let r = hi[c] - lo[c];
        if r > range {
            range = r;
            chan = c;
        }
    }
    (chan, range)
}

#[cfg(feature = "image")]
fn average(pixels: &[[u8; 3]]) -> Color {
    let n = pixels.len().max(1) as u64;
    let mut sum = [0u64; 3];
    for p in pixels {
        for c in 0..3 {
            sum[c] += p[c] as u64;
        }
    }
    Color::rgb((sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hue_dist(a: f64, b: f64) -> f64 {
        let d = (a - b).abs() % 360.0;
        d.min(360.0 - d)
    }

    fn min_pairwise_hue(p: &Palette) -> f64 {
        let hues: Vec<f64> = (1..=6).map(|i| p.ansi[i].to_oklch().2).collect();
        let mut m = f64::MAX;
        for i in 0..hues.len() {
            for j in (i + 1)..hues.len() {
                m = m.min(hue_dist(hues[i], hues[j]));
            }
        }
        m
    }

    fn variance(v: &[f64]) -> f64 {
        let n = v.len() as f64;
        let mean = v.iter().sum::<f64>() / n;
        v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n
    }

    fn clustered_palette() -> Palette {
        let mut p = Palette::default();
        for (i, hue) in [20.0, 25.0, 30.0, 35.0, 40.0, 45.0].into_iter().enumerate() {
            p.ansi[i + 1] = Color::from_oklch(0.65, 0.10, hue);
        }
        p
    }

    #[test]
    fn hue_separate_increases_min_hue_separation() {
        let clustered = clustered_palette();
        let before = min_pairwise_hue(&clustered);
        let after = min_pairwise_hue(&hue_separate(&clustered));
        assert!(
            after > before,
            "expected wider min hue separation: before={before}, after={after}"
        );
    }

    #[test]
    fn hue_separate_preserves_structure_and_neutrals() {
        let p = clustered_palette();
        let out = hue_separate(&p);
        assert_eq!(out.ansi.len(), 16);
        assert_eq!(out.background, p.background);
        assert_eq!(out.foreground, p.foreground);
        for i in [0usize, 7, 8, 15] {
            assert_eq!(out.ansi[i], p.ansi[i], "ansi {i} should be unchanged");
        }
        assert_eq!(out.cursor, p.cursor);
        assert_eq!(out.selection_background, p.selection_background);
    }

    #[test]
    fn hue_separate_holds_lightness_and_chroma() {
        let p = clustered_palette();
        let out = hue_separate(&p);
        for i in 1..=6 {
            let (l0, c0, _) = p.ansi[i].to_oklch();
            let (l1, c1, _) = out.ansi[i].to_oklch();
            assert!((l0 - l1).abs() < 0.02, "L drifted at ansi {i}");
            assert!((c0 - c1).abs() < 0.02, "C drifted at ansi {i}");
        }
    }

    #[test]
    fn harmonize_reduces_accent_variance() {
        let p = Palette::default();

        let l_before: Vec<f64> = (1..=6).map(|i| p.ansi[i].to_oklch().0).collect();
        let c_before: Vec<f64> = (1..=6).map(|i| p.ansi[i].to_oklch().1).collect();

        let out = harmonize(&p);

        let l_after: Vec<f64> = (1..=6).map(|i| out.ansi[i].to_oklch().0).collect();
        let c_after: Vec<f64> = (1..=6).map(|i| out.ansi[i].to_oklch().1).collect();

        assert!(
            variance(&l_after) < variance(&l_before),
            "L variance not reduced: {} -> {}",
            variance(&l_before),
            variance(&l_after)
        );
        assert!(
            variance(&c_after) < variance(&c_before),
            "C variance not reduced: {} -> {}",
            variance(&c_before),
            variance(&c_after)
        );

        assert_eq!(out.ansi.len(), 16);
        assert_eq!(out.background, p.background);
        assert_eq!(out.foreground, p.foreground);
    }

    #[test]
    fn harmonize_preserves_hues_roughly() {
        let p = Palette::default();
        let out = harmonize(&p);
        for i in 1..=6 {
            let h0 = p.ansi[i].to_oklch().2;
            let h1 = out.ansi[i].to_oklch().2;
            assert!(hue_dist(h0, h1) < 8.0, "hue shifted too far at ansi {i}");
        }
    }

    #[test]
    fn suggest_from_empty_is_default_extracted() {
        let p = suggest_palette_from_colors(&[]);
        assert_eq!(p.name, "Extracted");
        assert_eq!(p.ansi.len(), 16);
        assert!(contrast_ratio(p.background, p.foreground) >= 3.0);
    }

    #[test]
    fn suggest_from_single_color_is_contrasty() {
        let p = suggest_palette_from_colors(&[Color::rgb(0x33, 0x66, 0x99)]);
        assert_eq!(p.name, "Extracted");
        assert_eq!(p.ansi.len(), 16);
        assert!(
            contrast_ratio(p.background, p.foreground) >= 4.5,
            "fg/bg contrast too low: {}",
            contrast_ratio(p.background, p.foreground)
        );
    }

    #[test]
    fn suggest_from_handful_is_contrasty_and_complete() {
        let colors = [
            Color::rgb(0x1a, 0x1b, 0x26),
            Color::rgb(0xc0, 0x50, 0x40),
            Color::rgb(0x40, 0x90, 0x50),
            Color::rgb(0x40, 0x60, 0xb0),
            Color::rgb(0xe0, 0xd0, 0xa0),
        ];
        let p = suggest_palette_from_colors(&colors);
        assert_eq!(p.name, "Extracted");
        assert_eq!(p.ansi.len(), 16);
        assert!(
            contrast_ratio(p.background, p.foreground) >= 4.5,
            "fg/bg contrast too low: {}",
            contrast_ratio(p.background, p.foreground)
        );
        assert_eq!(p.background, Color::rgb(0x1a, 0x1b, 0x26));
    }

    #[test]
    fn median_handles_empty_odd_and_even() {
        assert_eq!(median(&mut []), 0.0);
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&mut [4.0, 1.0, 3.0, 2.0]), 2.5);
    }

    #[test]
    fn ang_diff_takes_the_shortest_signed_path() {
        assert!((ang_diff(350.0, 10.0) - 20.0).abs() < 1e-9);
        assert!((ang_diff(10.0, 350.0) + 20.0).abs() < 1e-9);
        assert!((ang_diff(0.0, 180.0) - 180.0).abs() < 1e-9);
        assert!((ang_diff(0.0, 181.0) + 179.0).abs() < 1e-9);
    }

    #[test]
    fn suggest_builds_light_theme_from_light_inputs() {
        let colors = [
            Color::rgb(0xff, 0xff, 0xff),
            Color::rgb(0xee, 0xdd, 0x88),
            Color::rgb(0xcc, 0xcc, 0xcc),
        ];
        let p = suggest_palette_from_colors(&colors);
        assert_eq!(p.variant, Variant::Light);
        assert_eq!(p.background, Color::rgb(0xff, 0xff, 0xff));
        assert!(
            contrast_ratio(p.background, p.foreground) >= 4.5,
            "light theme fg/bg contrast too low: {}",
            contrast_ratio(p.background, p.foreground)
        );
    }

    #[test]
    fn suggest_from_greyscale_only_has_no_chromatic_cursor() {
        let colors = [
            Color::rgb(0, 0, 0),
            Color::rgb(120, 120, 120),
            Color::rgb(255, 255, 255),
        ];
        let p = suggest_palette_from_colors(&colors);
        assert_eq!(p.cursor, p.foreground);
        assert_eq!(p.ansi.len(), 16);
        assert!(contrast_ratio(p.background, p.foreground) >= 4.5);
    }

    #[test]
    fn suggest_keeps_most_chromatic_when_two_share_a_role() {
        let vivid_red = Color::from_oklch(0.60, 0.14, 29.0);
        let muted_red = Color::from_oklch(0.60, 0.06, 29.0);
        assert_ne!(vivid_red, muted_red);
        assert!(vivid_red.to_oklch().1 > muted_red.to_oklch().1);

        let p = suggest_palette_from_colors(&[muted_red, vivid_red]);
        assert_eq!(
            p.ansi[1], vivid_red,
            "red role should take the more chromatic input"
        );
    }

    #[cfg(feature = "image")]
    #[test]
    fn extract_palette_recovers_known_colors() {
        use image::{ImageFormat, RgbImage};
        use std::io::Cursor;

        let mut img = RgbImage::new(3, 1);
        img.put_pixel(0, 0, image::Rgb([255, 0, 0]));
        img.put_pixel(1, 0, image::Rgb([0, 255, 0]));
        img.put_pixel(2, 0, image::Rgb([0, 0, 255]));

        let mut bytes: Vec<u8> = Vec::new();
        img.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();

        let colors = extract_palette(&bytes, 3).unwrap();
        assert_eq!(colors.len(), 3);

        for expected in [
            Color::rgb(255, 0, 0),
            Color::rgb(0, 255, 0),
            Color::rgb(0, 0, 255),
        ] {
            let found = colors.iter().any(|c| {
                let dr = c.r as i32 - expected.r as i32;
                let dg = c.g as i32 - expected.g as i32;
                let db = c.b as i32 - expected.b as i32;
                (dr * dr + dg * dg + db * db) < 400
            });
            assert!(found, "missing {} in {colors:?}", expected.to_hex());
        }
    }

    #[cfg(feature = "image")]
    #[test]
    fn extract_palette_rejects_garbage() {
        let err = extract_palette(b"not an image", 4);
        assert!(err.is_err());
    }

    #[cfg(feature = "image")]
    #[test]
    fn extract_palette_zero_k_is_empty() {
        use image::{ImageFormat, RgbImage};
        use std::io::Cursor;
        let img = RgbImage::new(2, 2);
        let mut bytes: Vec<u8> = Vec::new();
        img.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        assert_eq!(extract_palette(&bytes, 0).unwrap(), Vec::new());
    }
}
