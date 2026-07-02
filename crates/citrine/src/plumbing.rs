use std::collections::HashMap;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use citrine_core::color::Color;
use citrine_core::formats::{all_formats, format_by_id};
use citrine_core::palette::Palette;
use serde::Serialize;

use crate::adapters::adapter_by_id;
use crate::config::Roots;

#[derive(clap::Args, Debug)]
pub struct ExportArgs {
    #[arg(required_unless_present = "list", help = "Format id to export")]
    format: Option<String>,
    #[arg(
        long,
        value_name = "FILE",
        help = "Palette JSON file (defaults to the built-in palette)"
    )]
    palette: Option<PathBuf>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Write to this path instead of stdout"
    )]
    out: Option<PathBuf>,
    #[arg(long, help = "List all format ids")]
    list: bool,
}

#[derive(clap::Args, Debug)]
pub struct ProbeArgs {
    #[arg(long, value_name = "FILE", help = "Expected palette JSON")]
    expect: PathBuf,
    #[arg(long, value_name = "PATH", help = "Where to write the result JSON")]
    out: PathBuf,
    #[arg(
        long,
        value_name = "CSV",
        default_value = "ansi,fg,bg,cursor",
        help = "Slot groups to check: ansi, fg, bg, cursor"
    )]
    checks: String,
    #[arg(
        long,
        value_name = "MS",
        default_value_t = 4000,
        help = "Reply timeout in milliseconds"
    )]
    timeout_ms: u64,
    #[arg(
        long,
        value_name = "N",
        default_value_t = 0,
        help = "Per-channel tolerance for color comparison"
    )]
    tolerance: u8,
}

#[derive(clap::Args, Debug)]
pub struct VerifySetupArgs {
    #[arg(help = "Terminal id to scaffold")]
    terminal: String,
    #[arg(long, value_name = "FILE", help = "Palette JSON file")]
    palette: PathBuf,
    #[arg(
        long,
        value_name = "DIR",
        help = "Scratch directory for the generated files"
    )]
    dir: PathBuf,
    #[arg(
        long,
        value_name = "CMD",
        help = "Shell command the terminal runs to probe itself"
    )]
    probe_cmd: String,
}

pub fn run_verify_setup(args: VerifySetupArgs) -> Result<(), Box<dyn Error>> {
    let Some(adapter) = adapter_by_id(&args.terminal) else {
        eprintln!("unknown terminal id: {}", args.terminal);
        std::process::exit(2);
    };
    let palette = load_palette(&args.palette)?;
    let roots = Roots::from_env();
    let manifest = adapter.scaffold(&roots, &args.dir, &palette, &args.probe_cmd)?;
    let mut text = serde_json::to_string_pretty(&manifest)?;
    text.push('\n');
    let mut stdout = std::io::stdout();
    stdout.write_all(text.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

pub fn run_export(args: ExportArgs) -> Result<(), Box<dyn Error>> {
    if args.list {
        let mut listing = String::new();
        for f in all_formats() {
            listing.push_str(f.id());
            listing.push('\n');
        }
        let mut stdout = std::io::stdout();
        stdout.write_all(listing.as_bytes())?;
        stdout.flush()?;
        return Ok(());
    }
    let id = args.format.as_deref().unwrap_or_default();
    let palette = match &args.palette {
        Some(path) => load_palette(path)?,
        None => Palette::default(),
    };
    let Some(text) = export_text(id, &palette) else {
        eprintln!("unknown format id: {id}");
        std::process::exit(2);
    };
    match &args.out {
        Some(path) => fs::write(path, text)?,
        None => {
            let mut stdout = std::io::stdout();
            stdout.write_all(text.as_bytes())?;
            stdout.flush()?;
        }
    }
    Ok(())
}

pub fn run_probe(args: ProbeArgs) -> Result<i32, Box<dyn Error>> {
    let expected = load_palette(&args.expect)?;
    let slots = parse_checks(&args.checks)?;
    let replies = collect_replies(&slots, args.timeout_ms);
    let got = replies.unwrap_or_default();
    let report = build_report(&expected, &slots, &got, args.tolerance);
    write_report(&args.out, &report)?;
    if got.is_empty() {
        return Ok(3);
    }
    if report.pass {
        Ok(0)
    } else {
        Ok(2)
    }
}

fn collect_replies(slots: &[ReplySlot], timeout_ms: u64) -> Option<HashMap<ReplySlot, Color>> {
    let tty = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .ok()?;
    let mut writer = tty.try_clone().ok()?;
    let guard = RawGuard::enable().ok()?;
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        let mut reader = tty;
        let mut buf = [0u8; 256];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let queries = build_queries(slots);
    writer.write_all(queries.as_bytes()).ok()?;
    writer.flush().ok()?;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut parser = OscParser::new();
    let mut got: HashMap<ReplySlot, Color> = HashMap::new();
    loop {
        if slots.iter().all(|s| got.contains_key(s)) {
            break;
        }
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        match rx.recv_timeout(deadline - now) {
            Ok(bytes) => {
                for (slot, color) in parser.feed_slice(&bytes) {
                    got.insert(slot, color);
                }
            }
            Err(_) => break,
        }
    }
    drop(guard);
    Some(got)
}

fn load_palette(path: &Path) -> Result<Palette, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str::<Palette>(&text)?)
}

fn export_text(id: &str, palette: &Palette) -> Option<String> {
    format_by_id(id).map(|f| f.export(palette))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ReplySlot {
    Ansi(u8),
    Foreground,
    Background,
    Cursor,
}

impl ReplySlot {
    fn name(&self) -> String {
        match self {
            ReplySlot::Ansi(i) => format!("ansi{i}"),
            ReplySlot::Foreground => "foreground".to_string(),
            ReplySlot::Background => "background".to_string(),
            ReplySlot::Cursor => "cursor".to_string(),
        }
    }

    fn expected_color(&self, p: &Palette) -> Color {
        match self {
            ReplySlot::Ansi(i) => p.ansi[*i as usize],
            ReplySlot::Foreground => p.foreground,
            ReplySlot::Background => p.background,
            ReplySlot::Cursor => p.cursor,
        }
    }
}

fn parse_checks(csv: &str) -> Result<Vec<ReplySlot>, Box<dyn Error>> {
    let mut slots = Vec::new();
    let push = |slot: ReplySlot, slots: &mut Vec<ReplySlot>| {
        if !slots.contains(&slot) {
            slots.push(slot);
        }
    };
    for token in csv.split(',') {
        match token.trim() {
            "ansi" => {
                for i in 0u8..16 {
                    push(ReplySlot::Ansi(i), &mut slots);
                }
            }
            "fg" => push(ReplySlot::Foreground, &mut slots),
            "bg" => push(ReplySlot::Background, &mut slots),
            "cursor" => push(ReplySlot::Cursor, &mut slots),
            other => return Err(format!("unknown check: {other}").into()),
        }
    }
    Ok(slots)
}

fn build_queries(slots: &[ReplySlot]) -> String {
    let mut q = String::new();
    for slot in slots {
        match slot {
            ReplySlot::Ansi(i) => {
                let _ = write!(q, "\x1b]4;{i};?\x07");
            }
            ReplySlot::Foreground => q.push_str("\x1b]10;?\x07"),
            ReplySlot::Background => q.push_str("\x1b]11;?\x07"),
            ReplySlot::Cursor => q.push_str("\x1b]12;?\x07"),
        }
    }
    q
}

#[derive(Serialize)]
struct ProbeReport {
    pass: bool,
    checked: usize,
    matched: usize,
    results: Vec<ProbeResult>,
}

#[derive(Serialize)]
struct ProbeResult {
    slot: String,
    expected: String,
    actual: Option<String>,
    status: String,
}

fn within_tolerance(a: Color, b: Color, tolerance: u8) -> bool {
    let d = |x: u8, y: u8| x.abs_diff(y) <= tolerance;
    d(a.r, b.r) && d(a.g, b.g) && d(a.b, b.b)
}

fn build_report(
    expected: &Palette,
    slots: &[ReplySlot],
    got: &HashMap<ReplySlot, Color>,
    tolerance: u8,
) -> ProbeReport {
    let mut results = Vec::with_capacity(slots.len());
    let mut matched = 0usize;
    for slot in slots {
        let want = slot.expected_color(expected);
        let (actual, status) = match got.get(slot) {
            Some(c) if within_tolerance(*c, want, tolerance) => {
                matched += 1;
                (Some(c.to_hex()), "ok")
            }
            Some(c) => (Some(c.to_hex()), "mismatch"),
            None => (None, "noreply"),
        };
        results.push(ProbeResult {
            slot: slot.name(),
            expected: want.to_hex(),
            actual,
            status: status.to_string(),
        });
    }
    ProbeReport {
        pass: matched == slots.len(),
        checked: slots.len(),
        matched,
        results,
    }
}

fn write_report(path: &Path, report: &ProbeReport) -> Result<(), Box<dyn Error>> {
    let mut text = serde_json::to_string_pretty(report)?;
    text.push('\n');
    fs::write(path, text)?;
    Ok(())
}

struct RawGuard;

impl RawGuard {
    fn enable() -> std::io::Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

enum ParseState {
    Idle,
    Esc,
    Osc,
    OscEsc,
}

struct OscParser {
    state: ParseState,
    buf: Vec<u8>,
}

impl OscParser {
    fn new() -> Self {
        Self {
            state: ParseState::Idle,
            buf: Vec::new(),
        }
    }

    fn feed(&mut self, byte: u8) -> Option<(ReplySlot, Color)> {
        match self.state {
            ParseState::Idle => {
                if byte == 0x1b {
                    self.state = ParseState::Esc;
                }
                None
            }
            ParseState::Esc => {
                match byte {
                    b']' => {
                        self.buf.clear();
                        self.state = ParseState::Osc;
                    }
                    0x1b => {}
                    _ => self.state = ParseState::Idle,
                }
                None
            }
            ParseState::Osc => match byte {
                0x07 => {
                    self.state = ParseState::Idle;
                    parse_payload(&self.buf)
                }
                0x1b => {
                    self.state = ParseState::OscEsc;
                    None
                }
                _ => {
                    self.buf.push(byte);
                    if self.buf.len() > 128 {
                        self.buf.clear();
                        self.state = ParseState::Idle;
                    }
                    None
                }
            },
            ParseState::OscEsc => match byte {
                b'\\' => {
                    self.state = ParseState::Idle;
                    parse_payload(&self.buf)
                }
                b']' => {
                    self.buf.clear();
                    self.state = ParseState::Osc;
                    None
                }
                0x1b => {
                    self.state = ParseState::Esc;
                    None
                }
                _ => {
                    self.state = ParseState::Idle;
                    None
                }
            },
        }
    }

    fn feed_slice(&mut self, bytes: &[u8]) -> Vec<(ReplySlot, Color)> {
        let mut out = Vec::new();
        for &b in bytes {
            if let Some(reply) = self.feed(b) {
                out.push(reply);
            }
        }
        out
    }
}

fn parse_payload(buf: &[u8]) -> Option<(ReplySlot, Color)> {
    let s = std::str::from_utf8(buf).ok()?;
    let mut parts = s.splitn(3, ';');
    let code = parts.next()?;
    match code {
        "4" => {
            let idx: u8 = parts.next()?.trim().parse().ok()?;
            if idx > 15 {
                return None;
            }
            let color = parse_color_spec(parts.next()?)?;
            Some((ReplySlot::Ansi(idx), color))
        }
        "10" | "11" | "12" => {
            let color = parse_color_spec(parts.next()?)?;
            let slot = match code {
                "10" => ReplySlot::Foreground,
                "11" => ReplySlot::Background,
                _ => ReplySlot::Cursor,
            };
            Some((slot, color))
        }
        _ => None,
    }
}

fn parse_color_spec(spec: &str) -> Option<Color> {
    let rest = spec.trim().strip_prefix("rgb:")?;
    let mut channels = rest.split('/');
    let r = parse_channel(channels.next()?)?;
    let g = parse_channel(channels.next()?)?;
    let b = parse_channel(channels.next()?)?;
    if channels.next().is_some() {
        return None;
    }
    Some(Color::rgb(r, g, b))
}

fn parse_channel(s: &str) -> Option<u8> {
    if s.is_empty() || s.len() > 4 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    if s.len() == 1 {
        let v = u8::from_str_radix(s, 16).ok()?;
        Some(v * 17)
    } else {
        u8::from_str_radix(&s[..2], 16).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citrine_core::palette::{Slot, Variant};
    use std::collections::HashSet;

    fn sentinel_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ci/sentinel.json")
    }

    fn sentinel() -> Palette {
        let text = fs::read_to_string(sentinel_path()).expect("read ci/sentinel.json");
        serde_json::from_str(&text).expect("sentinel deserializes to Palette")
    }

    #[test]
    fn sentinel_fixture_loads_with_22_distinct_colors() {
        let p = sentinel();
        assert_eq!(p.name, "Citrine Sentinel");
        assert_eq!(p.variant, Variant::Dark);
        assert_eq!(p.minimum_contrast, None);
        let hexes: HashSet<String> = Slot::all().map(|s| p.get(s).to_hex()).collect();
        assert_eq!(hexes.len(), 22);
        for (i, c) in p.ansi.iter().enumerate() {
            let i = i as u8;
            assert_eq!(
                *c,
                Color::rgb(0x20 + i, 0x40 + 2 * i, 0xB0 - 3 * i),
                "ansi{i} does not follow the sentinel formula"
            );
        }
    }

    #[test]
    fn export_ghostty_sentinel_hex_lines() {
        let out = export_text("ghostty", &sentinel()).expect("ghostty format exists");
        assert!(out.contains("background = #101317\n"));
        assert!(out.contains("foreground = #e3e0d7\n"));
        assert!(out.contains("cursor-color = #ff6f00\n"));
        assert!(out.contains("palette = 0=#2040b0\n"));
        assert!(out.contains("palette = 5=#254aa1\n"));
        assert!(out.contains("palette = 15=#2f5e83\n"));
        assert!(!out.contains("minimum-contrast"));
    }

    #[test]
    fn export_kitty_sentinel_hex_lines() {
        let out = export_text("kitty", &sentinel()).expect("kitty format exists");
        assert!(out.contains("background #101317\n"));
        assert!(out.contains("foreground #e3e0d7\n"));
        assert!(out.contains("cursor #ff6f00\n"));
        assert!(out.contains("selection_background #143f2b\n"));
        assert!(out.contains("color0 #2040b0\n"));
        assert!(out.contains("color15 #2f5e83\n"));
    }

    #[test]
    fn export_text_rejects_unknown_format() {
        assert!(export_text("nope", &Palette::default()).is_none());
    }

    #[test]
    fn parser_reads_four_digit_reply_with_bel() {
        let mut p = OscParser::new();
        let replies = p.feed_slice(b"\x1b]4;5;rgb:2525/4a4a/a1a1\x07");
        assert_eq!(
            replies,
            vec![(ReplySlot::Ansi(5), Color::rgb(0x25, 0x4a, 0xa1))]
        );
    }

    #[test]
    fn parser_reads_two_digit_reply_with_st() {
        let mut p = OscParser::new();
        let replies = p.feed_slice(b"\x1b]10;rgb:e3/e0/d7\x1b\\");
        assert_eq!(
            replies,
            vec![(ReplySlot::Foreground, Color::rgb(0xe3, 0xe0, 0xd7))]
        );
    }

    #[test]
    fn parser_reads_three_digit_reply() {
        let mut p = OscParser::new();
        let replies = p.feed_slice(b"\x1b]11;rgb:101/131/171\x07");
        assert_eq!(
            replies,
            vec![(ReplySlot::Background, Color::rgb(0x10, 0x13, 0x17))]
        );
    }

    #[test]
    fn parser_ignores_interleaved_noise_and_da_reply() {
        let mut p = OscParser::new();
        let mut input = Vec::new();
        input.extend_from_slice(b"qwerty\x1b[?62;4c");
        input.extend_from_slice(b"\x1b]4;0;rgb:2040/4040/b0b0\x07");
        input.extend_from_slice(b"junk\x1b]not-a-reply\x07more");
        input.extend_from_slice(b"\x1b]12;rgb:ff6f/6f6f/0000\x1b\\");
        let replies = p.feed_slice(&input);
        assert_eq!(
            replies,
            vec![
                (ReplySlot::Ansi(0), Color::rgb(0x20, 0x40, 0xb0)),
                (ReplySlot::Cursor, Color::rgb(0xff, 0x6f, 0x00)),
            ]
        );
    }

    #[test]
    fn parser_reads_several_replies_in_one_chunk() {
        let mut p = OscParser::new();
        let replies = p.feed_slice(
            b"\x1b]4;0;rgb:2040/4040/b0b0\x07\x1b]4;1;rgb:2121/4242/adad\x1b\\\x1b]10;rgb:e3e3/e0e0/d7d7\x07",
        );
        assert_eq!(
            replies,
            vec![
                (ReplySlot::Ansi(0), Color::rgb(0x20, 0x40, 0xb0)),
                (ReplySlot::Ansi(1), Color::rgb(0x21, 0x42, 0xad)),
                (ReplySlot::Foreground, Color::rgb(0xe3, 0xe0, 0xd7)),
            ]
        );
    }

    #[test]
    fn parser_survives_split_reply_across_chunks() {
        let mut p = OscParser::new();
        assert!(p.feed_slice(b"\x1b]4;15;rgb:2f").is_empty());
        let replies = p.feed_slice(b"2f/5e5e/8383\x07");
        assert_eq!(
            replies,
            vec![(ReplySlot::Ansi(15), Color::rgb(0x2f, 0x5e, 0x83))]
        );
    }

    #[test]
    fn parser_rejects_out_of_range_index_and_bad_spec() {
        let mut p = OscParser::new();
        assert!(p.feed_slice(b"\x1b]4;16;rgb:0000/0000/0000\x07").is_empty());
        assert!(p.feed_slice(b"\x1b]10;cmyk:0/0/0/0\x07").is_empty());
        assert!(p.feed_slice(b"\x1b]11;rgb:zz/00/00\x07").is_empty());
        assert!(p.feed_slice(b"\x1b]11;rgb:00/00\x07").is_empty());
    }

    #[test]
    fn channel_scaling_takes_top_byte() {
        assert_eq!(parse_channel("ab"), Some(0xab));
        assert_eq!(parse_channel("abc"), Some(0xab));
        assert_eq!(parse_channel("abcd"), Some(0xab));
        assert_eq!(parse_channel("a"), Some(0xaa));
        assert_eq!(parse_channel(""), None);
        assert_eq!(parse_channel("abcde"), None);
    }

    #[test]
    fn checks_csv_expands_to_slots() {
        let all = parse_checks("ansi,fg,bg,cursor").unwrap();
        assert_eq!(all.len(), 19);
        assert_eq!(all[0], ReplySlot::Ansi(0));
        assert_eq!(all[15], ReplySlot::Ansi(15));
        assert_eq!(all[16], ReplySlot::Foreground);
        assert_eq!(all[17], ReplySlot::Background);
        assert_eq!(all[18], ReplySlot::Cursor);
        let some = parse_checks("fg,bg").unwrap();
        assert_eq!(some, vec![ReplySlot::Foreground, ReplySlot::Background]);
        assert!(parse_checks("fg,huh").is_err());
    }

    #[test]
    fn queries_match_osc_contract() {
        let q = build_queries(&[
            ReplySlot::Ansi(7),
            ReplySlot::Foreground,
            ReplySlot::Background,
            ReplySlot::Cursor,
        ]);
        assert_eq!(q, "\x1b]4;7;?\x07\x1b]10;?\x07\x1b]11;?\x07\x1b]12;?\x07");
    }

    #[test]
    fn report_json_shape_covers_ok_mismatch_noreply() {
        let p = sentinel();
        let slots = vec![ReplySlot::Ansi(0), ReplySlot::Foreground, ReplySlot::Cursor];
        let mut got = HashMap::new();
        got.insert(ReplySlot::Ansi(0), Color::rgb(0x20, 0x40, 0xb0));
        got.insert(ReplySlot::Foreground, Color::rgb(0x00, 0x00, 0x00));
        let report = build_report(&p, &slots, &got, 0);
        assert!(!report.pass);
        assert_eq!(report.checked, 3);
        assert_eq!(report.matched, 1);
        let v = serde_json::to_value(&report).unwrap();
        assert_eq!(v["pass"], false);
        assert_eq!(v["checked"], 3);
        assert_eq!(v["matched"], 1);
        assert_eq!(v["results"][0]["slot"], "ansi0");
        assert_eq!(v["results"][0]["expected"], "#2040b0");
        assert_eq!(v["results"][0]["actual"], "#2040b0");
        assert_eq!(v["results"][0]["status"], "ok");
        assert_eq!(v["results"][1]["slot"], "foreground");
        assert_eq!(v["results"][1]["actual"], "#000000");
        assert_eq!(v["results"][1]["status"], "mismatch");
        assert_eq!(v["results"][2]["slot"], "cursor");
        assert!(v["results"][2]["actual"].is_null());
        assert_eq!(v["results"][2]["status"], "noreply");
    }

    #[test]
    fn report_passes_when_every_slot_matches() {
        let p = sentinel();
        let slots = parse_checks("ansi,fg,bg,cursor").unwrap();
        let mut got = HashMap::new();
        for slot in &slots {
            got.insert(*slot, slot.expected_color(&p));
        }
        let report = build_report(&p, &slots, &got, 0);
        assert!(report.pass);
        assert_eq!(report.checked, 19);
        assert_eq!(report.matched, 19);
    }

    #[test]
    fn tolerance_accepts_small_shifts_and_rejects_large_ones() {
        let p = sentinel();
        let slots = vec![ReplySlot::Ansi(0)];
        let mut got = HashMap::new();
        got.insert(ReplySlot::Ansi(0), Color::rgb(0x27, 0x3f, 0xaa));
        let strict = build_report(&p, &slots, &got, 0);
        assert!(!strict.pass);
        let tolerant = build_report(&p, &slots, &got, 16);
        assert!(tolerant.pass);
        let mut far = HashMap::new();
        far.insert(ReplySlot::Ansi(0), Color::rgb(0x15, 0x18, 0x1d));
        let rejected = build_report(&p, &slots, &far, 16);
        assert!(!rejected.pass);
    }
}
