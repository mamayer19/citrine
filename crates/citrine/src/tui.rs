use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{cursor, execute};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color as UiColor, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use citrine_core::color::Color;
use citrine_core::contrast::{contrast_ratio, passes_aa, passes_aaa};
use citrine_core::formats::format_by_id;
use citrine_core::palette::{Palette, Slot};
use citrine_core::references::references;

use crate::config::{self, Roots, SaveError, SaveOutcome};
use crate::library::{self, LibraryEntry};
use crate::settings::Settings;

const ST: &str = "\x1b\\";

pub fn osc_apply(p: &Palette) -> String {
    fn rgb(c: Color) -> String {
        format!("{:02x}/{:02x}/{:02x}", c.r, c.g, c.b)
    }

    let mut out = String::new();
    for (i, c) in p.ansi.iter().enumerate() {
        out.push_str(&format!("\x1b]4;{i};rgb:{}{ST}", rgb(*c)));
    }
    out.push_str(&format!("\x1b]10;rgb:{}{ST}", rgb(p.foreground)));
    out.push_str(&format!("\x1b]11;rgb:{}{ST}", rgb(p.background)));
    out.push_str(&format!("\x1b]12;rgb:{}{ST}", rgb(p.cursor)));
    out
}

pub fn osc_reset() -> String {
    format!("\x1b]104{ST}\x1b]110{ST}\x1b]111{ST}\x1b]112{ST}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColorModel {
    Hsl,
    Oklch,
}

impl ColorModel {
    fn channel_names(self) -> [&'static str; 3] {
        match self {
            ColorModel::Hsl => ["H", "S", "L"],
            ColorModel::Oklch => ["L", "C", "H"],
        }
    }

    fn channel_name(self, i: usize) -> &'static str {
        self.channel_names()[i.min(2)]
    }

    fn label(self) -> &'static str {
        match self {
            ColorModel::Hsl => "HSL",
            ColorModel::Oklch => "OKLCH",
        }
    }

    fn toggled(self) -> Self {
        match self {
            ColorModel::Hsl => ColorModel::Oklch,
            ColorModel::Oklch => ColorModel::Hsl,
        }
    }
}

fn step_for(model: ColorModel, channel: usize, big: bool) -> f64 {
    match model {
        ColorModel::Hsl => match channel {
            0 => iff(big, 12.0, 2.0),
            1 => iff(big, 0.10, 0.02),
            _ => iff(big, 0.10, 0.02),
        },
        ColorModel::Oklch => match channel {
            0 => iff(big, 0.08, 0.02),
            1 => iff(big, 0.04, 0.01),
            _ => iff(big, 12.0, 2.0),
        },
    }
}

fn iff(cond: bool, a: f64, b: f64) -> f64 {
    if cond {
        a
    } else {
        b
    }
}

fn adjust_channel(color: Color, model: ColorModel, channel: usize, dir: f64, big: bool) -> Color {
    let step = dir * step_for(model, channel, big);
    match model {
        ColorModel::Hsl => {
            let (h, s, l) = color.to_hsl();
            match channel {
                0 => Color::from_hsl(h + step, s, l),
                1 => Color::from_hsl(h, (s + step).clamp(0.0, 1.0), l),
                _ => Color::from_hsl(h, s, (l + step).clamp(0.0, 1.0)),
            }
        }
        ColorModel::Oklch => {
            let (l, c, h) = color.to_oklch();
            match channel {
                0 => Color::from_oklch((l + step).clamp(0.0, 1.0), c, h),
                1 => Color::from_oklch(l, (c + step).max(0.0), h),
                _ => Color::from_oklch(l, c, h + step),
            }
        }
    }
}

fn parse_hex_input(input: &str) -> Option<Color> {
    Color::from_hex(input).ok()
}

const C_BG: UiColor = UiColor::Rgb(0x16, 0x16, 0x1e);
const C_FG: UiColor = UiColor::Rgb(0xd8, 0xd8, 0xe2);
const C_MUTED: UiColor = UiColor::Rgb(0x7c, 0x7c, 0x92);
const C_ACCENT: UiColor = UiColor::Rgb(0xdd, 0x77, 0x14);
const C_BORDER: UiColor = UiColor::Rgb(0x3a, 0x3a, 0x4c);
const C_OK: UiColor = UiColor::Rgb(0x3a, 0x8f, 0x4a);
const C_WARN: UiColor = UiColor::Rgb(0xc8, 0x5a, 0x44);

const ANSI_NAMES: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "br.black",
    "br.red",
    "br.green",
    "br.yellow",
    "br.blue",
    "br.magenta",
    "br.cyan",
    "br.white",
];

fn tc(c: Color) -> UiColor {
    UiColor::Rgb(c.r, c.g, c.b)
}

fn slot_display(slot: Slot) -> String {
    match slot {
        Slot::Ansi(n) => format!("{n:>2} {}", ANSI_NAMES[n as usize]),
        other => other.label(),
    }
}

fn mark(pass: bool) -> &'static str {
    if pass {
        "✓"
    } else {
        "✗"
    }
}

fn panel(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(C_BORDER).bg(C_BG))
        .style(Style::default().bg(C_BG))
}

static RESTORE_COLORS: AtomicBool = AtomicBool::new(false);

fn restore_terminal() {
    let mut out = io::stdout();
    if RESTORE_COLORS.load(Ordering::SeqCst) {
        let _ = out.write_all(osc_reset().as_bytes());
    }
    let _ = execute!(out, LeaveAlternateScreen, cursor::Show);
    let _ = disable_raw_mode();
    let _ = out.flush();
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

enum Mode {
    Normal,
    Hex(String),
    Picker { index: usize, set_path: bool },
    ConfirmApply { terminal_id: String },
    NamePrompt(String),
    PathPrompt { terminal_id: String, buffer: String },
    Browser { index: usize },
    ConfirmDelete { index: usize },
}

const MAX_HISTORY: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Continue,
    Quit,
}

struct App {
    palette: Palette,
    starting: Palette,
    slots: Vec<Slot>,
    selected: usize,
    model: ColorModel,
    channel: usize,
    live: bool,
    undo_stack: Vec<Palette>,
    mode: Mode,
    help: bool,
    status: String,
    ref_index: usize,
    roots: Roots,
    settings: Settings,
    library_entries: Vec<LibraryEntry>,
    should_quit: bool,
    osc_sink: Box<dyn Write>,
}

impl App {
    fn new() -> Self {
        let roots = Roots::from_env();
        let settings = Settings::load(&roots);
        let (palette, source) = initial_palette(&roots, &settings);
        let status = format!(
            "Loaded '{}' ({source}). Press ? for help · s apply · w save · o library.",
            palette.name
        );
        Self::with_parts(roots, settings, palette, status, Box::new(io::stdout()))
    }

    fn with_parts(
        roots: Roots,
        settings: Settings,
        palette: Palette,
        status: String,
        osc_sink: Box<dyn Write>,
    ) -> Self {
        Self {
            starting: palette.clone(),
            palette,
            slots: Slot::all().collect(),
            selected: 0,
            model: ColorModel::Hsl,
            channel: 0,
            live: false,
            undo_stack: Vec::new(),
            mode: Mode::Normal,
            help: false,
            status,
            ref_index: 0,
            roots,
            settings,
            library_entries: Vec::new(),
            should_quit: false,
            osc_sink,
        }
    }

    fn current_slot(&self) -> Slot {
        self.slots[self.selected]
    }

    fn snapshot(&mut self) {
        self.undo_stack.push(self.palette.clone());
        if self.undo_stack.len() > MAX_HISTORY {
            self.undo_stack.remove(0);
        }
    }

    fn apply_live_if_on(&mut self) {
        if self.live {
            let bytes = osc_apply(&self.palette);
            let _ = self.osc_sink.write_all(bytes.as_bytes());
            let _ = self.osc_sink.flush();
        }
    }

    fn move_slot(&mut self, delta: isize) {
        let n = self.slots.len() as isize;
        self.selected = (((self.selected as isize + delta) % n + n) % n) as usize;
    }

    fn cycle_channel(&mut self, delta: isize) {
        self.channel = (((self.channel as isize + delta) % 3 + 3) % 3) as usize;
    }

    fn adjust_selected(&mut self, dir: f64, big: bool) {
        let slot = self.current_slot();
        let before = self.palette.get(slot);
        let after = adjust_channel(before, self.model, self.channel, dir, big);
        if after != before {
            self.snapshot();
            self.palette.set(slot, after);
            self.apply_live_if_on();
        }
        self.status = format!(
            "{} · {} = {}",
            slot_display(slot),
            self.model.channel_name(self.channel),
            after.to_hex()
        );
    }

    fn toggle_model(&mut self) {
        self.model = self.model.toggled();
        self.status = format!("Color model: {}", self.model.label());
    }

    fn begin_hex(&mut self) {
        let cur = self.palette.get(self.current_slot()).to_hex();
        self.mode = Mode::Hex(cur);
        self.status = "Hex edit: type #rrggbb · Enter to apply · Esc to cancel".to_string();
    }

    fn toggle_live(&mut self) {
        self.live = !self.live;
        if self.live {
            RESTORE_COLORS.store(true, Ordering::SeqCst);
            let bytes = osc_apply(&self.palette);
            let _ = self.osc_sink.write_all(bytes.as_bytes());
            self.status = "● LIVE apply ON: your real terminal now mirrors the palette".to_string();
        } else {
            let _ = self.osc_sink.write_all(osc_reset().as_bytes());
            self.status = "○ LIVE apply OFF: restored your configured theme".to_string();
        }
        let _ = self.osc_sink.flush();
    }

    fn import(&mut self) {
        match config::read_current_theme(&self.roots, "ghostty") {
            Ok(p) => {
                self.snapshot();
                self.status = format!("Imported current Ghostty theme: {}", p.name);
                self.palette = p;
                self.apply_live_if_on();
            }
            Err(_) => {
                self.status =
                    "No importable Ghostty theme found (set `theme = …` in ghostty config)"
                        .to_string();
            }
        }
    }

    fn cycle_reference(&mut self) {
        let refs = references();
        if refs.is_empty() {
            return;
        }
        self.ref_index = (self.ref_index + 1) % refs.len();
        self.snapshot();
        self.palette = refs[self.ref_index].clone();
        self.status = format!(
            "Reference [{}/{}]: {}",
            self.ref_index + 1,
            refs.len(),
            self.palette.name
        );
        self.apply_live_if_on();
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.palette = prev;
            self.status = "Undo".to_string();
            self.apply_live_if_on();
        } else {
            self.status = "Nothing to undo".to_string();
        }
    }

    fn open_picker(&mut self, set_path: bool) {
        let index = self
            .settings
            .last_terminal
            .as_deref()
            .and_then(|id| config::terminals().iter().position(|t| t.id == id))
            .unwrap_or(0);
        self.mode = Mode::Picker { index, set_path };
        self.status = if set_path {
            "Set target path: pick a terminal · Enter edits · Esc cancels".to_string()
        } else {
            "Apply: pick a terminal · Enter writes · p sets path · Esc cancels".to_string()
        };
    }

    fn apply(&mut self, terminal_id: &str, overwrite: bool) {
        let Some(terminal) = config::find(terminal_id) else {
            self.status = format!("unknown terminal: {terminal_id}");
            self.mode = Mode::Normal;
            return;
        };
        let format = format_by_id(terminal.format_id).expect("terminal format is registered");
        let content = format.export(&self.palette);
        let name = self.palette.name.clone();

        match config::save_theme_at(
            self.settings.apply_override(terminal_id),
            &self.roots,
            terminal_id,
            None,
            Some(&name),
            &content,
            overwrite,
        ) {
            Ok(SaveOutcome::Written { path, backup }) => {
                let bak = backup
                    .map(|b| format!(" · backup {}", b.display()))
                    .unwrap_or_default();
                self.status = format!(
                    "Applied → {}{bak}  ·  {}",
                    path.display(),
                    terminal.reload_hint
                );
                self.settings.last_terminal = Some(terminal_id.to_string());
                self.mode = Mode::Normal;
            }
            Ok(SaveOutcome::Conflict { path }) => {
                self.status = format!("{} exists, overwrite? (y/n)", path.display());
                self.mode = Mode::ConfirmApply {
                    terminal_id: terminal_id.to_string(),
                };
            }
            Err(e) => {
                self.status = format!("Apply failed: {}", save_error(&e));
                self.mode = Mode::Normal;
            }
        }
    }

    fn begin_name_prompt(&mut self) {
        self.mode = Mode::NamePrompt(self.palette.name.clone());
        self.status = "Save to library: type a name · Enter saves · Esc cancels".to_string();
    }

    fn save_to_library(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.status = "Name cannot be empty, keep typing or press Esc".to_string();
            return;
        }
        match library::save(&self.roots, name, &self.palette) {
            Ok(path) => {
                self.palette.name = name.to_string();
                self.settings.last_palette = Some(name.to_string());
                self.status = format!("Saved to library → {}", path.display());
                self.mode = Mode::Normal;
            }
            Err(e) => {
                self.status = format!("Save failed: {e}");
                self.mode = Mode::Normal;
            }
        }
    }

    fn open_browser(&mut self) {
        self.library_entries = library::list(&self.roots);
        if self.library_entries.is_empty() {
            self.status = "Library is empty, press w to save the current palette".to_string();
            return;
        }
        self.mode = Mode::Browser { index: 0 };
        self.status = "Library: Enter loads · d deletes · Esc closes".to_string();
    }

    fn load_from_library(&mut self, slug: &str) {
        match library::load(&self.roots, slug) {
            Ok(p) => {
                self.snapshot();
                let name = p.name.clone();
                self.palette = p;
                self.settings.last_palette = Some(name.clone());
                self.status = format!("Loaded '{name}' from library");
                self.mode = Mode::Normal;
                self.apply_live_if_on();
            }
            Err(e) => {
                self.status = format!("Load failed: {e}");
                self.mode = Mode::Normal;
            }
        }
    }

    fn delete_selected(&mut self, index: usize) {
        let Some(entry) = self.library_entries.get(index) else {
            self.mode = Mode::Normal;
            return;
        };
        let (slug, name) = (entry.slug.clone(), entry.name.clone());
        match library::delete(&self.roots, &slug) {
            Ok(()) => {
                self.library_entries = library::list(&self.roots);
                self.status = format!("Deleted '{name}'");
                if self.library_entries.is_empty() {
                    self.mode = Mode::Normal;
                } else {
                    let index = index.min(self.library_entries.len() - 1);
                    self.mode = Mode::Browser { index };
                }
            }
            Err(e) => {
                self.status = format!("Delete failed: {e}");
                self.mode = Mode::Browser { index };
            }
        }
    }

    fn begin_path_prompt(&mut self, terminal_id: &str) {
        let buffer = self
            .settings
            .paths
            .get(terminal_id)
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        self.status =
            format!("Target path for {terminal_id}: Enter saves · empty clears · Esc cancels");
        self.mode = Mode::PathPrompt {
            terminal_id: terminal_id.to_string(),
            buffer,
        };
    }

    fn commit_path(&mut self, terminal_id: &str, path: &str) {
        let set = self.settings.set_override(terminal_id, path);
        self.status = if set {
            format!("Target path for {terminal_id} → {}", path.trim())
        } else {
            format!("Cleared target path for {terminal_id} (using default)")
        };
        if let Err(e) = self.settings.save(&self.roots) {
            self.status = format!("Could not persist settings: {e}");
        }
        self.mode = Mode::Normal;
    }

    fn persist_settings(&self) {
        let _ = self.settings.save(&self.roots);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
        } else {
            match self.mode {
                Mode::Hex(_) => self.on_hex_key(key),
                Mode::Picker { .. } => self.on_picker_key(key),
                Mode::ConfirmApply { .. } => self.on_confirm_apply_key(key),
                Mode::NamePrompt(_) => self.on_name_key(key),
                Mode::PathPrompt { .. } => self.on_path_key(key),
                Mode::Browser { .. } => self.on_browser_key(key),
                Mode::ConfirmDelete { .. } => self.on_confirm_delete_key(key),
                Mode::Normal => self.on_normal_key(key),
            }
        }
        if self.should_quit {
            self.quit_restore();
            Outcome::Quit
        } else {
            Outcome::Continue
        }
    }

    fn quit_restore(&mut self) {
        if self.live {
            let _ = self.osc_sink.write_all(osc_reset().as_bytes());
            let _ = self.osc_sink.flush();
            RESTORE_COLORS.store(false, Ordering::SeqCst);
        }
    }

    fn on_hex_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) if c.is_ascii_hexdigit() || c == '#' => {
                if let Mode::Hex(buf) = &mut self.mode {
                    if buf.len() < 7 {
                        buf.push(c);
                    }
                }
            }
            KeyCode::Backspace => {
                if let Mode::Hex(buf) = &mut self.mode {
                    buf.pop();
                }
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "Hex edit cancelled".to_string();
            }
            KeyCode::Enter => {
                let input = match &self.mode {
                    Mode::Hex(buf) => buf.clone(),
                    _ => String::new(),
                };
                match parse_hex_input(&input) {
                    Some(c) => {
                        let slot = self.current_slot();
                        self.snapshot();
                        self.palette.set(slot, c);
                        self.status = format!("Set {} = {}", slot_display(slot), c.to_hex());
                        self.mode = Mode::Normal;
                        self.apply_live_if_on();
                    }
                    None => {
                        self.status = format!("Invalid hex '{input}', keep typing or press Esc");
                    }
                }
            }
            _ => {}
        }
    }

    fn on_picker_key(&mut self, key: KeyEvent) {
        let Mode::Picker { index, set_path } = &self.mode else {
            return;
        };
        let (mut index, set_path) = (*index, *set_path);
        let terminals = config::terminals();
        let n = terminals.len();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                index = (index + n - 1) % n;
                self.mode = Mode::Picker { index, set_path };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                index = (index + 1) % n;
                self.mode = Mode::Picker { index, set_path };
            }
            KeyCode::Enter => {
                let id = terminals[index].id;
                if set_path {
                    self.begin_path_prompt(id);
                } else {
                    self.apply(id, false);
                }
            }
            KeyCode::Char('p') => {
                let id = terminals[index].id;
                self.begin_path_prompt(id);
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = Mode::Normal;
                self.status = "Cancelled".to_string();
            }
            _ => {}
        }
    }

    fn on_confirm_apply_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let Mode::ConfirmApply { terminal_id } = &self.mode else {
                    return;
                };
                let id = terminal_id.clone();
                self.apply(&id, true);
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "Apply cancelled".to_string();
            }
            _ => {}
        }
    }

    fn on_name_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Mode::NamePrompt(buf) = &mut self.mode {
                    if buf.chars().count() < 60 {
                        buf.push(c);
                    }
                }
            }
            KeyCode::Backspace => {
                if let Mode::NamePrompt(buf) = &mut self.mode {
                    buf.pop();
                }
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "Save cancelled".to_string();
            }
            KeyCode::Enter => {
                let name = match &self.mode {
                    Mode::NamePrompt(buf) => buf.clone(),
                    _ => String::new(),
                };
                self.save_to_library(&name);
            }
            _ => {}
        }
    }

    fn on_path_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Mode::PathPrompt { buffer, .. } = &mut self.mode {
                    if buffer.chars().count() < 256 {
                        buffer.push(c);
                    }
                }
            }
            KeyCode::Backspace => {
                if let Mode::PathPrompt { buffer, .. } = &mut self.mode {
                    buffer.pop();
                }
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "Path unchanged".to_string();
            }
            KeyCode::Enter => {
                let Mode::PathPrompt {
                    terminal_id,
                    buffer,
                } = &self.mode
                else {
                    return;
                };
                let (id, path) = (terminal_id.clone(), buffer.clone());
                self.commit_path(&id, &path);
            }
            _ => {}
        }
    }

    fn on_browser_key(&mut self, key: KeyEvent) {
        let Mode::Browser { index } = &self.mode else {
            return;
        };
        let mut index = *index;
        let n = self.library_entries.len();
        if n == 0 {
            self.mode = Mode::Normal;
            return;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                index = (index + n - 1) % n;
                self.mode = Mode::Browser { index };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                index = (index + 1) % n;
                self.mode = Mode::Browser { index };
            }
            KeyCode::Enter => {
                let slug = self.library_entries[index].slug.clone();
                self.load_from_library(&slug);
            }
            KeyCode::Char('d') => {
                self.mode = Mode::ConfirmDelete { index };
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = Mode::Normal;
                self.status = "Closed library".to_string();
            }
            _ => {}
        }
    }

    fn on_confirm_delete_key(&mut self, key: KeyEvent) {
        let Mode::ConfirmDelete { index } = &self.mode else {
            return;
        };
        let index = *index;
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => self.delete_selected(index),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.mode = Mode::Browser { index };
                self.status = "Delete cancelled".to_string();
            }
            _ => {}
        }
    }

    fn on_normal_key(&mut self, key: KeyEvent) {
        if self.help {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc => self.help = false,
                KeyCode::Char('q') => self.should_quit = true,
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Up | KeyCode::Char('k') => self.move_slot(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_slot(1),
            KeyCode::Left | KeyCode::BackTab => self.cycle_channel(-1),
            KeyCode::Right | KeyCode::Tab => self.cycle_channel(1),
            KeyCode::Char('-') | KeyCode::Char('[') => self.adjust_selected(-1.0, false),
            KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::Char(']') => {
                self.adjust_selected(1.0, false)
            }
            KeyCode::Char('{') => self.adjust_selected(-1.0, true),
            KeyCode::Char('}') => self.adjust_selected(1.0, true),
            KeyCode::Char('x') => self.toggle_model(),
            KeyCode::Char('e') => self.begin_hex(),
            KeyCode::Char(' ') | KeyCode::Char('a') => self.toggle_live(),
            KeyCode::Char('i') => self.import(),
            KeyCode::Char('r') => self.cycle_reference(),
            KeyCode::Char('u') => self.undo(),
            KeyCode::Char('s') => self.open_picker(false),
            KeyCode::Char('w') => self.begin_name_prompt(),
            KeyCode::Char('o') => self.open_browser(),
            KeyCode::Char('p') => self.open_picker(true),
            KeyCode::Char('?') => self.help = true,
            _ => {}
        }
    }

    fn draw(&self, f: &mut Frame) {
        let area = f.area();
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);
        let body = root[0];
        let hint = root[1];

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(34), Constraint::Min(0)])
            .split(body);
        let left = cols[0];
        let right = cols[1];

        let rrows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(8), Constraint::Length(9)])
            .split(right);
        let preview = rrows[0];
        let detail = rrows[1];

        self.render_slots(f, left);
        self.render_preview(f, preview);
        self.render_detail(f, detail);
        self.render_hint(f, hint);

        if self.help {
            self.render_help(f, body);
        }

        match &self.mode {
            Mode::Picker { index, set_path } => self.render_picker(f, body, *index, *set_path),
            Mode::Browser { index } => self.render_browser(f, body, *index, false),
            Mode::ConfirmDelete { index } => self.render_browser(f, body, *index, true),
            Mode::NamePrompt(buf) => {
                self.render_prompt(f, body, "Save to Library", "Palette name:", buf)
            }
            Mode::PathPrompt {
                terminal_id,
                buffer,
            } => self.render_prompt(
                f,
                body,
                "Target Path Override",
                &format!("Path for {terminal_id} (dir or file):"),
                buffer,
            ),
            _ => {}
        }
    }

    fn render_picker(&self, f: &mut Frame, area: Rect, index: usize, set_path: bool) {
        let rect = centered_rect(64, 74, area);
        f.render_widget(Clear, rect);

        let mut lines: Vec<Line> = vec![Line::from("")];
        for (i, t) in config::terminals().iter().enumerate() {
            let sel = i == index;
            let present = t.present(&self.roots);
            let marker = if sel { "▸ " } else { "  " };
            let dot = if present { "●" } else { "○" };
            let name_style = if sel {
                Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(C_FG)
            };
            let over = self
                .settings
                .paths
                .get(t.id)
                .map(|p| format!("  → {}", p.display()))
                .unwrap_or_default();
            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(C_ACCENT)),
                Span::styled(
                    format!("{dot} "),
                    Style::default().fg(if present { C_OK } else { C_MUTED }),
                ),
                Span::styled(format!("{:<10}", t.display_name()), name_style),
                Span::styled(over, Style::default().fg(C_MUTED)),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ● configured   ○ not detected",
            Style::default().fg(C_MUTED),
        )));
        let footer = if set_path {
            "  ↑↓ pick · Enter edit path · Esc cancel"
        } else {
            "  ↑↓ pick · Enter apply · p set path · Esc cancel"
        };
        lines.push(Line::from(Span::styled(
            footer,
            Style::default().fg(C_MUTED),
        )));

        let title = if set_path {
            "Set Target Path: Terminal"
        } else {
            "Apply Theme: Terminal"
        };
        let para = Paragraph::new(lines)
            .block(panel(title))
            .style(Style::default().bg(C_BG))
            .wrap(Wrap { trim: false });
        f.render_widget(para, rect);
    }

    fn render_browser(&self, f: &mut Frame, area: Rect, index: usize, confirm_delete: bool) {
        let rect = centered_rect(64, 74, area);
        f.render_widget(Clear, rect);

        let mut lines: Vec<Line> = vec![Line::from("")];
        if self.library_entries.is_empty() {
            lines.push(Line::from(Span::styled(
                "  (empty)",
                Style::default().fg(C_MUTED),
            )));
        }
        for (i, e) in self.library_entries.iter().enumerate() {
            let sel = i == index;
            let marker = if sel { "▸ " } else { "  " };
            let name_style = if sel {
                Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(C_FG)
            };
            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(C_ACCENT)),
                Span::styled(e.name.clone(), name_style),
                Span::styled(format!("  ({}.json)", e.slug), Style::default().fg(C_MUTED)),
            ]));
        }
        lines.push(Line::from(""));
        if confirm_delete {
            let name = self
                .library_entries
                .get(index)
                .map(|e| e.name.as_str())
                .unwrap_or("");
            lines.push(Line::from(Span::styled(
                format!("  Delete '{name}'? (y/n)"),
                Style::default().fg(C_WARN).add_modifier(Modifier::BOLD),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "  ↑↓ pick · Enter load · d delete · Esc close",
                Style::default().fg(C_MUTED),
            )));
        }

        let para = Paragraph::new(lines)
            .block(panel(&format!("Library · {}", self.library_entries.len())))
            .style(Style::default().bg(C_BG))
            .wrap(Wrap { trim: false });
        f.render_widget(para, rect);
    }

    fn render_prompt(&self, f: &mut Frame, area: Rect, title: &str, label: &str, buffer: &str) {
        let rect = centered_rect(64, 28, area);
        f.render_widget(Clear, rect);

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  {label}"),
                Style::default().fg(C_MUTED),
            )),
            Line::from(vec![
                Span::styled(
                    "  > ",
                    Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(buffer.to_string(), Style::default().fg(C_FG)),
                Span::styled("▏", Style::default().fg(C_ACCENT)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Enter confirm · Esc cancel",
                Style::default().fg(C_MUTED),
            )),
        ];
        let para = Paragraph::new(lines)
            .block(panel(title))
            .style(Style::default().bg(C_BG))
            .wrap(Wrap { trim: false });
        f.render_widget(para, rect);
    }

    fn render_slots(&self, f: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();
        let mut selected_line = 0usize;

        let header = |text: &str| {
            Line::from(Span::styled(
                format!("── {text} "),
                Style::default().fg(C_MUTED).add_modifier(Modifier::BOLD),
            ))
        };

        for (i, slot) in self.slots.iter().enumerate() {
            match i {
                0 => lines.push(header("BASE")),
                6 => lines.push(header("NORMAL 0-7")),
                14 => lines.push(header("BRIGHT 8-15")),
                _ => {}
            }
            let c = self.palette.get(*slot);
            let sel = i == self.selected;
            if sel {
                selected_line = lines.len();
            }
            let marker = if sel { "▸ " } else { "  " };
            let name_style = if sel {
                Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(C_FG)
            };
            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(C_ACCENT)),
                Span::styled("  ", Style::default().bg(tc(c))),
                Span::styled(format!(" {:<13}", slot_display(*slot)), name_style),
                Span::styled(
                    c.to_hex(),
                    Style::default().fg(if sel { C_FG } else { C_MUTED }),
                ),
            ]));
        }

        let inner_h = area.height.saturating_sub(2);
        let sel = selected_line as u16;
        let offset = if inner_h == 0 || sel < inner_h {
            0
        } else {
            sel - inner_h + 1
        };

        let p = Paragraph::new(lines)
            .block(panel("Slots · 22"))
            .style(Style::default().bg(C_BG))
            .scroll((offset, 0));
        f.render_widget(p, area);
    }

    fn render_preview(&self, f: &mut Frame, area: Rect) {
        let p = &self.palette;
        let fg = tc(p.foreground);
        let bg = tc(p.background);
        let a = |i: usize| tc(p.ansi[i]);
        let sty = |c: UiColor| Style::default().fg(c);
        let bold = |c: UiColor| Style::default().fg(c).add_modifier(Modifier::BOLD);

        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled("marci", bold(a(2))),
                Span::styled("@", sty(fg)),
                Span::styled("citrine", bold(a(4))),
                Span::styled(":", sty(fg)),
                Span::styled("~/dev", bold(a(6))),
                Span::styled(" $ ", sty(a(3))),
                Span::styled("citrine tui", sty(fg)),
            ]),
            Line::from(vec![
                Span::styled("drwxr-xr-x ", sty(a(8))),
                Span::styled("src  ", bold(a(4))),
                Span::styled("build.sh  ", bold(a(2))),
                Span::styled("theme.toml  ", sty(a(6))),
                Span::styled("README.md", sty(fg)),
            ]),
            Line::from(""),
            Line::from(Span::styled("// paint the town", sty(a(8)))),
            Line::from(vec![
                Span::styled("fn ", sty(a(5))),
                Span::styled("main", sty(a(4))),
                Span::styled("() {", sty(fg)),
            ]),
            Line::from(vec![
                Span::styled("    let ", sty(a(5))),
                Span::styled("name", sty(fg)),
                Span::styled(" = ", sty(fg)),
                Span::styled("\"Citrine\"", sty(a(2))),
                Span::styled(";", sty(fg)),
            ]),
            Line::from(vec![
                Span::styled("    println!(", sty(fg)),
                Span::styled("\"{}\"", sty(a(2))),
                Span::styled(", name);", sty(fg)),
            ]),
            Line::from(Span::styled("}", sty(fg))),
            Line::from(""),
        ];

        let mut swatches: Vec<Span> = Vec::with_capacity(16);
        for c in p.ansi.iter() {
            swatches.push(Span::styled("  ", Style::default().bg(tc(*c))));
        }
        lines.push(Line::from(swatches));

        let block = panel(&format!("Live Preview · {}", p.name)).style(Style::default().bg(bg));
        let para = Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(bg).fg(fg));
        f.render_widget(para, area);
    }

    fn render_detail(&self, f: &mut Frame, area: Rect) {
        let slot = self.current_slot();
        let c = self.palette.get(slot);

        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(vec![
            Span::styled("  ", Style::default().bg(tc(c))),
            Span::styled(
                format!(" {}  ", slot_display(slot)),
                Style::default().fg(C_FG).add_modifier(Modifier::BOLD),
            ),
            Span::styled(c.to_hex(), Style::default().fg(C_ACCENT)),
            Span::styled(
                format!("  rgb({},{},{})", c.r, c.g, c.b),
                Style::default().fg(C_MUTED),
            ),
        ]));

        let (h, s, l) = c.to_hsl();
        lines.push(self.channel_line(
            "HSL  ",
            &[
                ("H", format!("{h:5.1}")),
                ("S", format!("{s:4.2}")),
                ("L", format!("{l:4.2}")),
            ],
            ColorModel::Hsl,
        ));
        let (ol, oc, oh) = c.to_oklch();
        lines.push(self.channel_line(
            "OKLCH",
            &[
                ("L", format!("{ol:4.2}")),
                ("C", format!("{oc:4.2}")),
                ("H", format!("{oh:5.1}")),
            ],
            ColorModel::Oklch,
        ));

        let (other, other_label) = if matches!(slot, Slot::Background) {
            (self.palette.foreground, "fg")
        } else {
            (self.palette.background, "bg")
        };
        let ratio = contrast_ratio(c, other);
        let aa = passes_aa(ratio, false);
        let aaa = passes_aaa(ratio, false);
        lines.push(Line::from(vec![
            Span::styled(
                format!("Contrast vs {other_label}: "),
                Style::default().fg(C_MUTED),
            ),
            Span::styled(format!("{ratio:.2}:1  "), Style::default().fg(C_FG)),
            Span::styled(
                format!("AA {}", mark(aa)),
                Style::default().fg(if aa { C_OK } else { C_WARN }),
            ),
            Span::styled(
                format!("  AAA {}", mark(aaa)),
                Style::default().fg(if aaa { C_OK } else { C_WARN }),
            ),
        ]));

        let live = if self.live {
            Span::styled(
                "● LIVE",
                Style::default().fg(C_OK).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled("○ live", Style::default().fg(C_MUTED))
        };
        let mut state = vec![
            live,
            Span::styled(
                format!("   model {}", self.model.label()),
                Style::default().fg(C_FG),
            ),
            Span::styled(
                format!("   chan {}", self.model.channel_name(self.channel)),
                Style::default().fg(C_FG),
            ),
        ];
        if self.palette != self.starting {
            state.push(Span::styled("   ✎ modified", Style::default().fg(C_WARN)));
        }
        lines.push(Line::from(state));

        if let Mode::Hex(buf) = &self.mode {
            lines.push(Line::from(vec![
                Span::styled(
                    "hex> ",
                    Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(buf.clone(), Style::default().fg(C_FG)),
                Span::styled("▏", Style::default().fg(C_ACCENT)),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                self.status.clone(),
                Style::default().fg(C_MUTED),
            )));
        }

        let para = Paragraph::new(lines)
            .block(panel("Inspector"))
            .style(Style::default().bg(C_BG))
            .wrap(Wrap { trim: false });
        f.render_widget(para, area);
    }

    fn channel_line(
        &self,
        prefix: &'static str,
        vals: &[(&'static str, String); 3],
        model: ColorModel,
    ) -> Line<'static> {
        let mut spans = vec![Span::styled(
            format!("{prefix} "),
            Style::default().fg(C_MUTED),
        )];
        for (i, (name, val)) in vals.iter().enumerate() {
            let active = model == self.model && i == self.channel;
            let style = if active {
                Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(C_FG)
            };
            let text = if active {
                format!("[{name} {val}] ")
            } else {
                format!(" {name} {val}  ")
            };
            spans.push(Span::styled(text, style));
        }
        Line::from(spans)
    }

    fn render_hint(&self, f: &mut Frame, area: Rect) {
        let acc = Style::default().fg(C_ACCENT);
        let mut spans: Vec<Span> = Vec::new();
        let push = |k: &str, d: &str, spans: &mut Vec<Span>| {
            spans.push(Span::styled(format!(" {k}"), acc));
            spans.push(Span::styled(format!(" {d} "), Style::default().fg(C_MUTED)));
        };
        push("↑↓/jk", "slot", &mut spans);
        push("←→/Tab", "chan", &mut spans);
        push("-/+", "adj", &mut spans);
        push("{ }", "big", &mut spans);
        push("x", "model", &mut spans);
        push("e", "hex", &mut spans);
        push("space", "LIVE", &mut spans);
        push("s", "apply", &mut spans);
        push("w", "save", &mut spans);
        push("o", "library", &mut spans);
        push("p", "path", &mut spans);
        push("r", "ref", &mut spans);
        push("i", "import", &mut spans);
        push("u", "undo", &mut spans);
        push("?", "help", &mut spans);
        push("q", "quit", &mut spans);
        let para = Paragraph::new(Line::from(spans)).style(Style::default().bg(C_BG));
        f.render_widget(para, area);
    }

    fn render_help(&self, f: &mut Frame, area: Rect) {
        let rect = centered_rect(64, 82, area);
        f.render_widget(Clear, rect);

        let key = |k: &str, d: &str| {
            Line::from(vec![
                Span::styled(
                    format!("  {k:<12}"),
                    Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(d.to_string(), Style::default().fg(C_FG)),
            ])
        };
        let lines = vec![
            Line::from(""),
            key("↑ ↓ / j k", "move slot selection"),
            key("← → / Tab", "cycle active channel"),
            key("- [  /  + = ]", "decrease / increase active channel"),
            key("{  }", "decrease / increase by a bigger step"),
            key("x", "toggle color model (HSL ↔ OKLCH)"),
            key("e", "hex edit (type #rrggbb, Enter/Esc)"),
            key("space / a", "toggle LIVE apply to the real terminal"),
            key("i", "re-import current Ghostty theme"),
            key("r", "cycle bundled reference presets"),
            key("u", "undo last edit"),
            Line::from(""),
            key("s", "APPLY: pick a terminal, write its theme config"),
            key("w", "SAVE the palette to your local library"),
            key("o", "OPEN the library (Enter loads · d deletes)"),
            key("p", "set a custom target PATH for a terminal"),
            Line::from(""),
            key("?", "toggle this help"),
            key("q / Esc / Ctrl-C", "quit (restores your terminal)"),
            Line::from(""),
            Line::from(Span::styled(
                "  Library: ~/.local/share/citrine/palettes/*.json.",
                Style::default().fg(C_MUTED),
            )),
            Line::from(Span::styled(
                "  Apply paths default per-terminal; p overrides them.",
                Style::default().fg(C_MUTED),
            )),
            Line::from(Span::styled(
                "  LIVE apply recolors via OSC; exit restores your theme.",
                Style::default().fg(C_MUTED),
            )),
        ];
        let para = Paragraph::new(lines)
            .block(panel("Keybindings"))
            .style(Style::default().bg(C_BG))
            .wrap(Wrap { trim: false });
        f.render_widget(para, rect);
    }
}

fn save_error(e: &SaveError) -> String {
    match e {
        SaveError::UnknownTerminal(id) => format!("unknown terminal: {id}"),
        SaveError::InvalidFilename(f) => format!("invalid filename: {f}"),
        SaveError::MissingName => "missing name".to_string(),
        SaveError::Io(err) => err.to_string(),
    }
}

fn initial_palette(roots: &Roots, settings: &Settings) -> (Palette, &'static str) {
    if let Some(name) = &settings.last_palette {
        if let Ok(p) = library::load(roots, name) {
            return (p, "from library");
        }
    }
    match config::read_current_theme(roots, "ghostty") {
        Ok(p) => (p, "current Ghostty theme"),
        Err(_) => (Palette::default(), "default"),
    }
}

fn centered_rect(px: u16, py: u16, r: Rect) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - py) / 2),
            Constraint::Percentage(py),
            Constraint::Percentage((100 - py) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - px) / 2),
            Constraint::Percentage(px),
            Constraint::Percentage((100 - px) / 2),
        ])
        .split(v[1])[1]
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| app.draw(f))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press && app.handle_key(key) == Outcome::Quit {
                break;
            }
        }
    }
    Ok(())
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    enable_raw_mode()?;
    let _guard = TerminalGuard;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let outcome = event_loop(&mut terminal, &mut app);
    app.persist_settings();
    outcome?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::RefCell;
    use std::rc::Rc;

    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    #[derive(Clone, Default)]
    struct SharedSink(Rc<RefCell<Vec<u8>>>);

    impl SharedSink {
        fn text(&self) -> String {
            String::from_utf8_lossy(&self.0.borrow()).into_owned()
        }
    }

    impl Write for SharedSink {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
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

    fn app_with(roots: Roots) -> (App, SharedSink) {
        let sink = SharedSink::default();
        let app = App::with_parts(
            roots,
            Settings::default(),
            Palette::default(),
            "ready".to_string(),
            Box::new(sink.clone()),
        );
        (app, sink)
    }

    fn new_app() -> (tempfile::TempDir, App, SharedSink) {
        let (tmp, roots) = temp_roots();
        let (app, sink) = app_with(roots);
        (tmp, app, sink)
    }

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn c(ch: char) -> KeyEvent {
        k(KeyCode::Char(ch))
    }

    fn press(app: &mut App, code: KeyCode) -> Outcome {
        app.handle_key(k(code))
    }

    fn typ(app: &mut App, text: &str) {
        for ch in text.chars() {
            app.handle_key(c(ch));
        }
    }

    fn render(app: &App, w: u16, h: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| app.draw(f)).unwrap();
        buffer_text(terminal.backend().buffer())
    }

    fn buffer_text(buf: &Buffer) -> String {
        let area = *buf.area();
        let mut out = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                if let Some(cell) = buf.cell((x, y)) {
                    out.push_str(cell.symbol());
                }
            }
            out.push('\n');
        }
        out
    }

    fn screen(app: &App) -> String {
        render(app, 170, 44)
    }

    #[test]
    fn osc_apply_contains_expected_sequences() {
        let s = osc_apply(&Palette::default());
        assert!(
            s.contains("\x1b]4;0;rgb:4b/46/56\x1b\\"),
            "missing ansi 0: {s:?}"
        );
        assert!(
            s.contains("\x1b]4;15;rgb:ea/e0/c6\x1b\\"),
            "missing ansi 15: {s:?}"
        );
        assert!(s.contains("\x1b]10;rgb:5a/53/68\x1b\\"), "missing fg");
        assert!(s.contains("\x1b]11;rgb:f0/e5/ac\x1b\\"), "missing bg");
        assert!(s.contains("\x1b]12;rgb:dd/77/14\x1b\\"), "missing cursor");
        assert_eq!(s.matches("\x1b]4;").count(), 16);
    }

    #[test]
    fn osc_reset_contains_reset_codes() {
        let s = osc_reset();
        assert!(s.contains("\x1b]104\x1b\\"), "missing 104: {s:?}");
        assert!(s.contains("\x1b]110\x1b\\"), "missing 110");
        assert!(s.contains("\x1b]111\x1b\\"), "missing 111");
        assert!(s.contains("\x1b]112\x1b\\"), "missing 112");
    }

    #[test]
    fn hsl_lightness_clamps_high_and_low() {
        let white = Color::rgb(255, 255, 255);
        assert_eq!(adjust_channel(white, ColorModel::Hsl, 2, 1.0, false), white);
        let black = Color::rgb(0, 0, 0);
        assert_eq!(
            adjust_channel(black, ColorModel::Hsl, 2, -1.0, false),
            black
        );
    }

    #[test]
    fn hsl_saturation_clamps_at_full() {
        let red = Color::rgb(255, 0, 0);
        assert_eq!(adjust_channel(red, ColorModel::Hsl, 1, 1.0, false), red);
    }

    #[test]
    fn hsl_lightness_decrease_darkens_known_case() {
        let red = Color::rgb(255, 0, 0);
        let out = adjust_channel(red, ColorModel::Hsl, 2, -1.0, false);
        let (_, _, l) = out.to_hsl();
        assert!(l < 0.5, "expected darker than 0.5, got {l}");
        assert!(
            out.r < 255 && out.g == 0 && out.b == 0,
            "got {}",
            out.to_hex()
        );
    }

    #[test]
    fn oklch_lightness_clamps_low() {
        let black = Color::rgb(0, 0, 0);
        assert_eq!(
            adjust_channel(black, ColorModel::Oklch, 0, -1.0, false),
            black
        );
    }

    #[test]
    fn oklch_chroma_clamps_at_zero() {
        let white = Color::rgb(255, 255, 255);
        assert_eq!(
            adjust_channel(white, ColorModel::Oklch, 1, -1.0, false),
            white
        );
    }

    #[test]
    fn big_step_moves_more_than_small_step() {
        let c = Color::rgb(120, 80, 200);
        let small = adjust_channel(c, ColorModel::Hsl, 2, 1.0, false);
        let big = adjust_channel(c, ColorModel::Hsl, 2, 1.0, true);
        let (_, _, ls) = small.to_hsl();
        let (_, _, lb) = big.to_hsl();
        assert!(lb > ls, "big step ({lb}) should exceed small step ({ls})");
    }

    #[test]
    fn hex_input_accepts_valid_rejects_invalid() {
        assert_eq!(parse_hex_input("#ff0000"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(parse_hex_input("00ff00"), Some(Color::rgb(0, 255, 0)));
        assert_eq!(parse_hex_input("#abc"), Some(Color::rgb(0xaa, 0xbb, 0xcc)));
        assert!(parse_hex_input("nothex").is_none());
        assert!(parse_hex_input("#12").is_none());
        assert!(parse_hex_input("").is_none());
    }

    #[test]
    fn model_toggle_and_channel_names() {
        assert_eq!(ColorModel::Hsl.toggled(), ColorModel::Oklch);
        assert_eq!(ColorModel::Oklch.toggled(), ColorModel::Hsl);
        assert_eq!(ColorModel::Hsl.channel_name(0), "H");
        assert_eq!(ColorModel::Oklch.channel_name(0), "L");
        assert_eq!(ColorModel::Hsl.channel_name(9), "L");
    }

    #[test]
    fn initial_render_shows_all_regions() {
        let (_tmp, app, _sink) = new_app();
        let s = screen(&app);
        for needle in [
            "Slots",
            "BASE",
            "NORMAL",
            "BRIGHT",
            "Live Preview",
            "Citrus Field (Dawn)",
            "Inspector",
            "HSL",
            "OKLCH",
            "Contrast",
            "#f0e5ac",
            "slot",
            "chan",
            "adj",
            "LIVE",
            "apply",
            "save",
            "library",
            "quit",
        ] {
            assert!(s.contains(needle), "initial render missing {needle:?}\n{s}");
        }
    }

    #[test]
    fn slot_navigation_moves_and_wraps() {
        let (_tmp, mut app, _sink) = new_app();
        assert_eq!(app.current_slot(), Slot::Background);
        press(&mut app, KeyCode::Down);
        assert_eq!(app.selected, 1);
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected, 2);
        press(&mut app, KeyCode::Up);
        assert_eq!(app.selected, 1);
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.selected, 0);
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.selected, 21);
        assert_eq!(app.current_slot(), Slot::Ansi(15));
        assert!(screen(&app).contains("▸"), "selection marker missing");
    }

    #[test]
    fn channel_cycle_wraps_and_survives_model_toggle() {
        let (_tmp, mut app, _sink) = new_app();
        assert_eq!(app.channel, 0);
        press(&mut app, KeyCode::Right);
        assert_eq!(app.channel, 1);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.channel, 2);
        press(&mut app, KeyCode::Right);
        assert_eq!(app.channel, 0);
        press(&mut app, KeyCode::Left);
        assert_eq!(app.channel, 2);
        press(&mut app, KeyCode::BackTab);
        assert_eq!(app.channel, 1);
        press(&mut app, KeyCode::Char('x'));
        assert_eq!(app.model, ColorModel::Oklch);
        assert_eq!(app.channel, 1);
        assert!(app.status.contains("OKLCH"));
        assert!(screen(&app).contains("model OKLCH"));
    }

    #[test]
    fn adjust_changes_selected_color_in_expected_direction() {
        let (_tmp, mut app, _sink) = new_app();
        app.selected = 10;
        press(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Right);
        assert_eq!(app.channel, 2);
        let slot = app.current_slot();
        let (_, _, l0) = app.palette.get(slot).to_hsl();
        press(&mut app, KeyCode::Char('+'));
        let (_, _, l1) = app.palette.get(slot).to_hsl();
        assert!(l1 > l0, "'+' should lighten: {l0} -> {l1}");
        assert_eq!(app.undo_stack.len(), 1, "a real change snapshots for undo");
        press(&mut app, KeyCode::Char('-'));
        let (_, _, l2) = app.palette.get(slot).to_hsl();
        assert!(l2 < l1, "'-' should darken: {l1} -> {l2}");
        let before = app.palette.get(slot);
        press(&mut app, KeyCode::Char('}'));
        assert_ne!(before, app.palette.get(slot), "big step changes the color");
        assert!(app.status.contains("blue"), "status: {}", app.status);
    }

    #[test]
    fn hex_edit_applies_valid_rejects_invalid_and_cancels() {
        let (_tmp, mut app, _sink) = new_app();
        app.selected = 6;
        let slot = app.current_slot();
        press(&mut app, KeyCode::Char('e'));
        assert!(matches!(app.mode, Mode::Hex(_)));
        assert!(screen(&app).contains("hex>"), "hex prompt should render");
        for _ in 0..8 {
            press(&mut app, KeyCode::Backspace);
        }
        typ(&mut app, "#ff0000");
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.mode, Mode::Normal));
        assert_eq!(app.palette.get(slot), Color::rgb(255, 0, 0));
        assert_eq!(app.undo_stack.len(), 1);
        press(&mut app, KeyCode::Char('e'));
        for _ in 0..8 {
            press(&mut app, KeyCode::Backspace);
        }
        typ(&mut app, "#12");
        press(&mut app, KeyCode::Enter);
        assert!(
            matches!(app.mode, Mode::Hex(_)),
            "invalid hex stays in edit mode"
        );
        assert!(app.status.contains("Invalid hex"), "status: {}", app.status);
        press(&mut app, KeyCode::Esc);
        assert!(matches!(app.mode, Mode::Normal));
        assert!(app.status.contains("cancelled"));
        assert_eq!(app.palette.get(slot), Color::rgb(255, 0, 0));
    }

    #[test]
    fn undo_restores_and_reports_empty() {
        let (_tmp, mut app, _sink) = new_app();
        app.selected = 6;
        let slot = app.current_slot();
        let before = app.palette.get(slot);
        press(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Char('+'));
        assert_ne!(app.palette.get(slot), before);
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(app.palette.get(slot), before, "undo restores the color");
        assert_eq!(app.status, "Undo");
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(app.status, "Nothing to undo");
    }

    #[test]
    fn live_toggle_and_edits_emit_osc_reset_on_off() {
        let (_tmp, mut app, sink) = new_app();
        press(&mut app, KeyCode::Char(' '));
        assert!(app.live);
        let on = sink.text();
        assert!(on.contains("\x1b]4;0;rgb:"), "apply OSC 4 missing:\n{on:?}");
        assert!(on.contains("\x1b]10;rgb:"), "fg OSC missing");
        assert!(on.contains("\x1b]11;rgb:"), "bg OSC missing");
        assert!(on.contains("\x1b]12;rgb:"), "cursor OSC missing");
        assert_eq!(on.matches("\x1b]4;").count(), 16, "16 ANSI slots pushed");
        app.selected = 6;
        press(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Right);
        let before = sink.text().len();
        press(&mut app, KeyCode::Char('+'));
        assert!(sink.text().len() > before, "edit re-emits while LIVE");
        press(&mut app, KeyCode::Char(' '));
        assert!(!app.live);
        assert!(sink.text().contains("\x1b]104"), "reset OSC 104 missing");
        let (_t2, mut app2, sink2) = new_app();
        press(&mut app2, KeyCode::Char('a'));
        assert!(app2.live);
        assert!(sink2.text().contains("\x1b]4;"));
    }

    #[test]
    fn quit_keys_return_quit_and_reset_when_live() {
        let (_t, mut app, sink) = new_app();
        assert_eq!(press(&mut app, KeyCode::Char('q')), Outcome::Quit);
        assert!(sink.text().is_empty(), "no OSC without LIVE");
        let (_t2, mut app2, _s2) = new_app();
        assert_eq!(press(&mut app2, KeyCode::Esc), Outcome::Quit);
        let (_t3, mut app3, _s3) = new_app();
        press(&mut app3, KeyCode::Char('e'));
        assert!(matches!(app3.mode, Mode::Hex(_)));
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app3.handle_key(ctrl_c), Outcome::Quit);
        let (_t4, mut app4, sink4) = new_app();
        press(&mut app4, KeyCode::Char(' '));
        let n = sink4.text().len();
        assert_eq!(press(&mut app4, KeyCode::Char('q')), Outcome::Quit);
        let after = sink4.text();
        assert!(after.len() > n, "reset appended on quit");
        assert!(after.contains("\x1b]104"), "palette reset on quit");
        assert!(after.contains("\x1b]112"), "cursor reset on quit");
    }

    #[test]
    fn save_to_library_flow_writes_file_and_lists_it() {
        let (_tmp, mut app, _sink) = new_app();
        let roots = app.roots.clone();
        press(&mut app, KeyCode::Char('w'));
        press(&mut app, KeyCode::Esc);
        assert!(matches!(app.mode, Mode::Normal));
        assert!(app.status.contains("Save cancelled"));
        press(&mut app, KeyCode::Char('w'));
        assert!(matches!(app.mode, Mode::NamePrompt(_)));
        assert!(screen(&app).contains("Save to Library"), "prompt title");
        for _ in 0..40 {
            press(&mut app, KeyCode::Backspace);
        }
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.mode, Mode::NamePrompt(_)));
        assert!(app.status.contains("cannot be empty"));
        typ(&mut app, "My Cool Theme");
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.mode, Mode::Normal));
        assert!(app.status.contains("Saved to library"), "{}", app.status);
        assert_eq!(app.palette.name, "My Cool Theme");
        assert_eq!(app.settings.last_palette.as_deref(), Some("My Cool Theme"));
        assert!(library::palettes_dir(&roots)
            .join("my-cool-theme.json")
            .is_file());
        let listed = library::list(&roots);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "My Cool Theme");
    }

    #[test]
    fn library_browser_opens_loads_and_deletes() {
        let (_tmp, roots) = temp_roots();
        let alt = Palette {
            background: Color::rgb(1, 2, 3),
            ..Palette::default()
        };
        library::save(&roots, "Alpha", &Palette::default()).unwrap();
        library::save(&roots, "Bravo", &alt).unwrap();
        let (mut app, _sink) = app_with(roots.clone());

        press(&mut app, KeyCode::Char('o'));
        assert!(matches!(app.mode, Mode::Browser { index: 0 }));
        let s = screen(&app);
        assert!(s.contains("Library"), "browser title\n{s}");
        assert!(s.contains("Alpha") && s.contains("Bravo"), "entries listed");
        press(&mut app, KeyCode::Char('j'));
        assert!(matches!(app.mode, Mode::Browser { index: 1 }));
        press(&mut app, KeyCode::Char('k'));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.mode, Mode::Normal));
        assert_eq!(app.palette.name, "Bravo");
        assert!(app.status.contains("Loaded 'Bravo'"));
        press(&mut app, KeyCode::Char('o'));
        press(&mut app, KeyCode::Char('d'));
        assert!(matches!(app.mode, Mode::ConfirmDelete { .. }));
        assert!(screen(&app).contains("Delete"), "confirm prompt renders");
        press(&mut app, KeyCode::Char('n'));
        assert!(matches!(app.mode, Mode::Browser { .. }));
        assert!(app.status.contains("Delete cancelled"));
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(library::list(&roots).len(), 1, "one palette deleted");
    }

    #[test]
    fn browser_on_empty_library_reports_empty() {
        let (_tmp, mut app, _sink) = new_app();
        press(&mut app, KeyCode::Char('o'));
        assert!(matches!(app.mode, Mode::Normal));
        assert!(app.status.contains("Library is empty"));
    }

    #[test]
    fn browser_renders_empty_state_when_forced() {
        let (_tmp, mut app, _sink) = new_app();
        app.library_entries = Vec::new();
        app.mode = Mode::Browser { index: 0 };
        assert!(screen(&app).contains("(empty)"), "empty browser state");
        press(&mut app, KeyCode::Char('j'));
        assert!(matches!(app.mode, Mode::Normal));
    }

    #[test]
    fn apply_picker_writes_theme_and_handles_overwrite() {
        let (_tmp, roots) = temp_roots();
        let (mut app, _sink) = app_with(roots.clone());

        press(&mut app, KeyCode::Char('s'));
        assert!(matches!(
            app.mode,
            Mode::Picker {
                set_path: false,
                ..
            }
        ));
        let s = screen(&app);
        assert!(s.contains("Apply Theme"), "picker title\n{s}");
        assert_eq!(config::terminals().len(), 8);
        for t in config::terminals() {
            assert!(s.contains(t.display_name()), "missing {}", t.display_name());
        }
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.mode, Mode::Normal));
        assert!(app.status.contains("Applied"), "{}", app.status);
        assert_eq!(app.settings.last_terminal.as_deref(), Some("ghostty"));
        let theme = config::find("ghostty")
            .unwrap()
            .theme_dir(&roots)
            .join("citrus-field-dawn");
        assert!(theme.is_file(), "ghostty theme written");

        press(&mut app, KeyCode::Char('s'));
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.mode, Mode::ConfirmApply { .. }));
        assert!(app.status.contains("overwrite?"));
        press(&mut app, KeyCode::Char('y'));
        assert!(matches!(app.mode, Mode::Normal));
        let dir = config::find("ghostty").unwrap().theme_dir(&roots);
        let has_bak = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().contains(".bak."));
        assert!(has_bak, "overwrite created a .bak backup");

        press(&mut app, KeyCode::Char('s'));
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('n'));
        assert!(matches!(app.mode, Mode::Normal));
        assert!(app.status.contains("Apply cancelled"));
    }

    #[test]
    fn apply_picker_navigation_and_cancel_and_path_shortcut() {
        let (_tmp, mut app, _sink) = new_app();
        press(&mut app, KeyCode::Char('s'));
        assert!(matches!(app.mode, Mode::Picker { index: 0, .. }));
        press(&mut app, KeyCode::Char('j'));
        assert!(matches!(app.mode, Mode::Picker { index: 1, .. }));
        press(&mut app, KeyCode::Char('k'));
        assert!(matches!(app.mode, Mode::Picker { index: 0, .. }));
        press(&mut app, KeyCode::Up);
        let last = config::terminals().len() - 1;
        assert!(matches!(app.mode, Mode::Picker { index, .. } if index == last));
        press(&mut app, KeyCode::Char('p'));
        assert!(matches!(app.mode, Mode::PathPrompt { .. }));
        press(&mut app, KeyCode::Esc);
        assert!(matches!(app.mode, Mode::Normal));
        press(&mut app, KeyCode::Char('s'));
        press(&mut app, KeyCode::Esc);
        assert!(matches!(app.mode, Mode::Normal));
        assert!(app.status.contains("Cancelled"));
    }

    #[test]
    fn path_override_flow_persists_and_clears() {
        let (_tmp, roots) = temp_roots();
        let (mut app, _sink) = app_with(roots.clone());
        press(&mut app, KeyCode::Char('p'));
        assert!(matches!(app.mode, Mode::Picker { set_path: true, .. }));
        assert!(screen(&app).contains("Set Target Path"), "set-path title");
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.mode, Mode::PathPrompt { .. }));
        let s = screen(&app);
        assert!(
            s.contains("Target Path Override") && s.contains("ghostty"),
            "path prompt\n{s}"
        );
        let target = std::env::temp_dir().join("citrine-my-themes");
        let target_str = target.to_str().unwrap().to_string();
        typ(&mut app, &target_str);
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.mode, Mode::Normal));
        assert!(app.status.contains("Target path for ghostty"));
        assert_eq!(
            app.settings.apply_override("ghostty"),
            Some(target.as_path())
        );
        assert_eq!(
            Settings::load(&roots).apply_override("ghostty"),
            Some(target.as_path())
        );
        press(&mut app, KeyCode::Char('p'));
        press(&mut app, KeyCode::Enter);
        for _ in 0..(target_str.chars().count() + 8) {
            press(&mut app, KeyCode::Backspace);
        }
        press(&mut app, KeyCode::Enter);
        assert!(app.status.contains("Cleared target path"));
        assert!(app.settings.apply_override("ghostty").is_none());
        press(&mut app, KeyCode::Char('p'));
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Esc);
        assert!(matches!(app.mode, Mode::Normal));
        assert!(app.status.contains("Path unchanged"));
    }

    #[test]
    fn reference_presets_cycle() {
        let (_tmp, mut app, _sink) = new_app();
        let refs = references();
        press(&mut app, KeyCode::Char('r'));
        assert_eq!(app.palette.name, refs[1].name);
        assert!(app.status.contains("Reference [2/"));
        press(&mut app, KeyCode::Char('r'));
        assert_eq!(app.palette.name, refs[2].name);
        assert_eq!(app.undo_stack.len(), 2);
    }

    #[test]
    fn import_ghostty_theme_or_reports_none() {
        let (_tmp, roots) = temp_roots();
        let (mut app, _sink) = app_with(roots.clone());
        let name0 = app.palette.name.clone();
        press(&mut app, KeyCode::Char('i'));
        assert!(app.status.contains("No importable Ghostty theme"));
        assert_eq!(app.palette.name, name0);
        let ghostty = roots.config.join("ghostty");
        std::fs::create_dir_all(ghostty.join("themes")).unwrap();
        std::fs::write(ghostty.join("config"), "theme = seed\n").unwrap();
        let text = format_by_id("ghostty").unwrap().export(&Palette::default());
        std::fs::write(ghostty.join("themes").join("seed"), text).unwrap();
        press(&mut app, KeyCode::Char('i'));
        assert!(
            app.status.contains("Imported current Ghostty theme"),
            "{}",
            app.status
        );
        assert_eq!(app.palette.background.to_hex(), "#f0e5ac");
    }

    #[test]
    fn help_overlay_toggles_and_swallows_edit_keys() {
        let (_tmp, mut app, _sink) = new_app();
        press(&mut app, KeyCode::Char('?'));
        assert!(app.help);
        let s = screen(&app);
        assert!(s.contains("Keybindings"), "help panel\n{s}");
        assert!(s.contains("move slot selection"));
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected, 0, "help swallows navigation");
        press(&mut app, KeyCode::Esc);
        assert!(!app.help);
        press(&mut app, KeyCode::Char('?'));
        assert_eq!(press(&mut app, KeyCode::Char('q')), Outcome::Quit);
    }

    #[test]
    fn render_covers_modified_live_bg_contrast_and_scroll() {
        let (_tmp, mut app, _sink) = new_app();
        app.selected = 6;
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Char('+'));
        let s = screen(&app);
        assert!(s.contains("Contrast vs bg"), "bg contrast label\n{s}");
        assert!(s.contains("LIVE"), "live indicator");
        assert!(s.contains("modified"), "modified marker");
        app.selected = 21;
        assert!(
            render(&app, 130, 16).contains("br.white"),
            "bottom slot visible when scrolled"
        );
    }

    #[test]
    fn rare_branches_unknown_terminal_bad_index_and_apply_error() {
        let (_tmp, mut app, _sink) = new_app();
        app.mode = Mode::ConfirmApply {
            terminal_id: "nope".to_string(),
        };
        press(&mut app, KeyCode::Char('y'));
        assert!(app.status.contains("unknown terminal"));
        assert!(matches!(app.mode, Mode::Normal));

        let (_t2, mut app2, _s2) = new_app();
        app2.mode = Mode::ConfirmDelete { index: 99 };
        press(&mut app2, KeyCode::Char('y'));
        assert!(matches!(app2.mode, Mode::Normal));

        let (_t3, roots) = temp_roots();
        let blocker = roots.home.join("blocker");
        std::fs::write(&blocker, "not a dir").unwrap();
        let (mut app3, _s3) = app_with(roots);
        app3.settings
            .set_override("ghostty", blocker.join("theme.conf").to_str().unwrap());
        press(&mut app3, KeyCode::Char('s'));
        press(&mut app3, KeyCode::Enter);
        assert!(app3.status.contains("Apply failed"), "{}", app3.status);
        assert!(matches!(app3.mode, Mode::Normal));
    }

    #[test]
    fn hue_adjust_in_both_models_and_history_cap() {
        let (_tmp, mut app, _sink) = new_app();
        app.selected = 10;
        for _ in 0..(MAX_HISTORY + 5) {
            press(&mut app, KeyCode::Char('}'));
        }
        assert_eq!(app.undo_stack.len(), MAX_HISTORY, "undo history is capped");
        press(&mut app, KeyCode::Char('x'));
        press(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Right);
        let before = app.palette.get(app.current_slot());
        press(&mut app, KeyCode::Char('+'));
        assert_ne!(
            app.palette.get(app.current_slot()),
            before,
            "OKLCH hue step changes the color"
        );
    }
}
