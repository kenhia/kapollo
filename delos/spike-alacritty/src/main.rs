//! S2 vertical slice on `alacritty_terminal` (the `Term`/`Grid` model).
//!
//! Same "feel" target as S1, different engine: a PTY-backed shell parsed by
//! `alacritty_terminal::Term` (driven through `vte::ansi::Processor`), rendered to a
//! ratatui `Buffer` by reading the grid cells directly, with content-coordinate mouse
//! selection, scrollback, alt-screen handover, and explicit copy. Throwaway spike code.
//!
//! ## Coordinate model
//!
//! `alacritty_terminal` keeps the scroll position internally as a `display_offset`
//! (0 = live bottom, growing as you scroll up — exactly like `vt100`'s scrollback).
//! We derive the same `top_row` window over absolute history as S1:
//!
//! ```text
//! top_row = BASE - display_offset            (BASE = SCROLLBACK_LEN)
//! content_row = screen_to_content(top_row, y) = BASE - display_offset + y
//! buffer_line(visible_row) = Line(visible_row - display_offset)
//! ```
//!
//! Unlike S1, `top_row` is **not** persisted: the emulator owns the scroll position,
//! so we read `display_offset` afresh each event/frame. Net win — fewer moving parts.

mod selection;

use std::io::{self, Write};
use std::time::Duration;

use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Terminal;
use spike_support::coords::{content_to_screen, screen_to_content, Cell};
use spike_support::{copy_local, detect_mode, modes::ModeEvent, osc52_frame, PtyShell, PtySize};

use alacritty_terminal::event::{Event as AlacEvent, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line as TermLine};
use alacritty_terminal::term::cell::{Cell as GridCell, Flags};
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor, Processor};

use selection::{LeftPress, SelectionController, Trigger};

/// Scrollback depth (rows). Doubles as the `BASE` offset for the coordinate bridge.
const SCROLLBACK_LEN: usize = 5000;
const BASE: usize = SCROLLBACK_LEN;

/// Lines emitted by the `Ctrl-F` flood self-test (manual T022/T027 drift harness).
const FLOOD_LINES: usize = 2000;

type SpikeTerm = Term<EventProxy>;

/// Forwards the few emulator-originated events we care about. `alacritty_terminal`
/// occasionally needs to write back to the PTY (DSR, terminal identification, etc.);
/// we shuttle those bytes to the shell via a channel since `send_event` is `&self`.
#[derive(Clone)]
struct EventProxy {
    tx: std::sync::mpsc::Sender<Vec<u8>>,
}

impl EventListener for EventProxy {
    fn send_event(&self, event: AlacEvent) {
        if let AlacEvent::PtyWrite(text) = event {
            let _ = self.tx.send(text.into_bytes());
        }
    }
}

/// Minimal `Dimensions` descriptor for `Term::new`/`Term::resize`. History depth comes
/// from `Config::scrolling_history`, so `total_lines == screen_lines` here.
#[derive(Clone, Copy)]
struct TermDim {
    cols: usize,
    lines: usize,
}

impl Dimensions for TermDim {
    fn columns(&self) -> usize {
        self.cols
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn total_lines(&self) -> usize {
        self.lines
    }
}

enum Clipboard {
    Osc52,
    Arboard,
}

struct Args {
    shell: String,
    clipboard: Clipboard,
}

fn parse_args() -> Args {
    let mut shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let mut clipboard = Clipboard::Osc52;
    for arg in std::env::args().skip(1) {
        if arg == "--clipboard=arboard" {
            clipboard = Clipboard::Arboard;
        } else if !arg.starts_with('-') {
            shell = arg;
        }
    }
    Args { shell, clipboard }
}

fn main() -> Result<()> {
    let args = parse_args();
    install_panic_hook();
    setup_terminal()?;
    let result = run(&args);
    restore_terminal();
    result
}

fn setup_terminal() -> Result<()> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    Ok(())
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
}

fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default(info);
    }));
}

/// Live routing state, updated from the child's output stream.
#[derive(Default)]
struct Routing {
    alt_screen: bool,
    child_mouse: bool,
}

impl Routing {
    fn apply(&mut self, events: &[ModeEvent]) {
        for ev in events {
            match ev {
                ModeEvent::AltScreenEnter => self.alt_screen = true,
                ModeEvent::AltScreenExit => self.alt_screen = false,
                ModeEvent::MouseEnable(_) => self.child_mouse = true,
                ModeEvent::MouseDisable(_) => self.child_mouse = false,
            }
        }
    }

    fn override_active(&self) -> bool {
        self.alt_screen || self.child_mouse
    }
}

fn run(args: &Args) -> Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let size = terminal.size()?;
    let (mut rows, mut cols) = (size.height, size.width);

    let (tx, pty_rx) = std::sync::mpsc::channel::<Vec<u8>>();
    let config = Config {
        scrolling_history: SCROLLBACK_LEN,
        ..Config::default()
    };
    let dim = TermDim {
        cols: cols as usize,
        lines: rows as usize,
    };
    let mut term: SpikeTerm = Term::new(config, &dim, EventProxy { tx });
    let mut processor: Processor = Processor::new();

    let mut shell = PtyShell::spawn(
        &args.shell,
        PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        },
    )?;

    let mut routing = Routing::default();
    let mut sel = SelectionController::new();
    let mut menu: Option<(u16, u16)> = None;

    loop {
        let mut got_output = false;
        while let Ok(chunk) = shell.output.try_recv() {
            routing.apply(&detect_mode(&chunk));
            processor.advance(&mut term, &chunk);
            got_output = true;
        }
        // Flush any emulator → PTY write-backs (device-status replies, etc.).
        while let Ok(bytes) = pty_rx.try_recv() {
            shell.write(&bytes)?;
        }
        // Corroborate routing with the emulator's authoritative mode flags.
        if got_output {
            let mode = term.mode();
            routing.alt_screen = mode.contains(TermMode::ALT_SCREEN);
            routing.child_mouse = mode.intersects(
                TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION,
            );
        }
        if shell.try_wait()? {
            break;
        }

        let top_row = top_row(&term);
        draw(&mut terminal, &term, &sel, top_row, rows, cols, menu)?;

        if !event::poll(Duration::from_millis(16))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) => {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('q'))
                {
                    break;
                }
                menu = None;
                if routing.override_active() {
                    forward_key(&mut shell, key)?;
                    continue;
                }
                handle_key(key, &mut sel, &mut shell, &mut term, rows, cols)?;
            }
            Event::Mouse(m) => {
                if routing.override_active() {
                    forward_mouse(&mut shell, m)?;
                    continue;
                }
                handle_mouse(
                    m, &mut sel, &mut shell, &mut term, rows, cols, &mut menu, args,
                )?;
            }
            Event::Resize(w, h) => {
                cols = w;
                rows = h;
                term.resize(TermDim {
                    cols: cols as usize,
                    lines: rows as usize,
                });
                let _ = shell.resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                });
            }
            _ => {}
        }
    }
    Ok(())
}

/// Current `top_row` derived from the emulator's scroll position.
fn top_row(term: &SpikeTerm) -> usize {
    BASE - term.grid().display_offset().min(BASE)
}

#[allow(clippy::too_many_arguments)]
fn handle_mouse(
    m: MouseEvent,
    sel: &mut SelectionController,
    shell: &mut PtyShell,
    term: &mut SpikeTerm,
    rows: u16,
    cols: u16,
    menu: &mut Option<(u16, u16)>,
    args: &Args,
) -> Result<()> {
    let height = rows as usize;
    let shift = m.modifiers.contains(KeyModifiers::SHIFT);

    match m.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            *menu = None;
            let cell = (
                screen_to_content(top_row(term), m.row as usize),
                m.column as usize,
            );
            match sel.left_press(cell, shift) {
                LeftPress::ForwardToChild => forward_mouse(shell, m)?,
                LeftPress::StartedDrag | LeftPress::Cancelled => {}
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            // Auto-scroll when dragging past the top/bottom edge (FR-010).
            if m.row == 0 {
                term.scroll_display(Scroll::Delta(1));
            } else if m.row as usize >= height.saturating_sub(1) {
                term.scroll_display(Scroll::Delta(-1));
            }
            let y = m.row.min(rows.saturating_sub(1)) as usize;
            let cell = (screen_to_content(top_row(term), y), m.column as usize);
            sel.drag_to(cell);
        }
        MouseEventKind::Up(MouseButton::Left) => {
            sel.release();
        }
        MouseEventKind::Down(MouseButton::Right) => match sel.right_press() {
            Trigger::Copy(a, b) => copy_selection(term, top_row(term), rows, cols, a, b, args)?,
            Trigger::ContextMenu => *menu = Some((m.column, m.row)),
            Trigger::Sigint => {}
        },
        MouseEventKind::ScrollUp => term.scroll_display(Scroll::Delta(3)),
        MouseEventKind::ScrollDown => term.scroll_display(Scroll::Delta(-3)),
        _ => {}
    }
    Ok(())
}

fn handle_key(
    key: KeyEvent,
    sel: &mut SelectionController,
    shell: &mut PtyShell,
    term: &mut SpikeTerm,
    rows: u16,
    cols: u16,
) -> Result<()> {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => match sel.ctrl_c() {
            Trigger::Copy(a, b) => {
                copy_selection(term, top_row(term), rows, cols, a, b, &osc_args())?;
            }
            Trigger::Sigint => shell.write(&[0x03])?,
            Trigger::ContextMenu => {}
        },
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            flood_selftest(sel, shell, top_row(term), rows, cols)?;
        }
        KeyCode::Esc if sel.cancel() => {}
        KeyCode::PageUp => term.scroll_display(Scroll::PageUp),
        KeyCode::PageDown => term.scroll_display(Scroll::PageDown),
        _ => {
            if let Some(bytes) = key_to_bytes(key) {
                shell.write(&bytes)?;
            }
        }
    }
    Ok(())
}

fn osc_args() -> Args {
    Args {
        shell: String::new(),
        clipboard: Clipboard::Osc52,
    }
}

/// Repeatable drift probe (manual T022/T027 harness, **not** a product feature): drop an
/// Active selection band onto the current viewport, then flood the child with output.
/// Correct, content-anchored behavior: the highlight rides up with its text and scrolls
/// off the top. A screen-relative bug would instead glue the highlight in place over the
/// new output streaming through those rows.
fn flood_selftest(
    sel: &mut SelectionController,
    shell: &mut PtyShell,
    top_row: usize,
    rows: u16,
    cols: u16,
) -> Result<()> {
    let band_top = top_row + (rows as usize / 3);
    let band_bottom = band_top + 2;
    let last_col = cols.saturating_sub(1) as usize;
    sel.cancel();
    sel.left_press((band_top, 0), false);
    sel.drag_to((band_bottom, last_col));
    sel.release();
    shell.write(format!("seq 1 {FLOOD_LINES}\r").as_bytes())?;
    Ok(())
}

/// Build a `rows × cols` char matrix of the *visible* viewport by indexing the grid
/// directly: visible row `vr` maps to buffer `Line(vr - display_offset)`.
fn viewport_matrix(term: &SpikeTerm, rows: usize, cols: usize) -> Vec<Vec<char>> {
    let d = term.grid().display_offset() as i32;
    let grid = term.grid();
    let mut matrix = vec![vec![' '; cols]; rows];
    for (vr, row) in matrix.iter_mut().enumerate() {
        let line = TermLine(vr as i32 - d);
        for (c, slot) in row.iter_mut().enumerate() {
            let ch = grid[line][Column(c)].c;
            *slot = if ch == '\0' { ' ' } else { ch };
        }
    }
    matrix
}

#[allow(clippy::too_many_arguments)]
fn copy_selection(
    term: &SpikeTerm,
    top_row: usize,
    rows: u16,
    cols: u16,
    a: Cell,
    b: Cell,
    args: &Args,
) -> Result<()> {
    let height = rows as usize;
    let width = cols as usize;
    let sr = content_to_screen(top_row, a.0, height).unwrap_or(0);
    let er = content_to_screen(top_row, b.0, height).unwrap_or(height - 1);
    let matrix = viewport_matrix(term, height, width);

    let mut lines: Vec<String> = Vec::new();
    for (sy, row) in matrix.iter().enumerate().take(er + 1).skip(sr) {
        let c0 = if sy == sr {
            a.1.min(width.saturating_sub(1))
        } else {
            0
        };
        let c1 = if sy == er {
            b.1.min(width.saturating_sub(1))
        } else {
            width.saturating_sub(1)
        };
        let text: String = row[c0..=c1].iter().collect();
        lines.push(text.trim_end().to_string());
    }
    let text = lines.join("\n");
    if text.is_empty() {
        return Ok(());
    }
    match args.clipboard {
        Clipboard::Osc52 => {
            let mut out = io::stdout();
            out.write_all(osc52_frame(text.as_bytes()).as_bytes())?;
            out.flush()?;
        }
        Clipboard::Arboard => {
            let _ = copy_local(&text);
        }
    }
    Ok(())
}

fn draw(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    term: &SpikeTerm,
    sel: &SelectionController,
    top_row: usize,
    rows: u16,
    cols: u16,
    menu: Option<(u16, u16)>,
) -> Result<()> {
    let height = rows as usize;
    terminal.draw(|frame| {
        let area = frame.area();
        let d = term.grid().display_offset() as i32;
        let grid = term.grid();
        let buf = frame.buffer_mut();

        let max_r = (rows.min(area.height)) as usize;
        let max_c = (cols.min(area.width)) as usize;
        for vr in 0..max_r {
            let line = TermLine(vr as i32 - d);
            for c in 0..max_c {
                let cell = &grid[line][Column(c)];
                let x = area.x + c as u16;
                let y = area.y + vr as u16;
                if let Some(bc) = buf.cell_mut((x, y)) {
                    let ch = if cell.c == '\0' { ' ' } else { cell.c };
                    bc.set_symbol(&ch.to_string());
                    bc.set_style(cell_style(cell));
                }
            }
        }

        // Selection highlight overlay (content coords → visible cells).
        if let Some((sa, sb)) = sel.range() {
            for content_row in sa.0..=sb.0 {
                let Some(y) = content_to_screen(top_row, content_row, height) else {
                    continue;
                };
                let (c0, c1) = row_col_span(sa, sb, content_row, area.width);
                for col in c0..=c1 {
                    let x = area.x + col;
                    let yy = area.y + y as u16;
                    if x < area.x + area.width && yy < area.y + area.height {
                        if let Some(cell) = buf.cell_mut((x, yy)) {
                            cell.set_style(Style::new().add_modifier(Modifier::REVERSED));
                        }
                    }
                }
            }
        }

        // Trivial "Hello, World." context menu (FR-019).
        if let Some((mx, my)) = menu {
            let mw = 16u16;
            let mh = 3u16;
            let x = mx.min(area.width.saturating_sub(mw));
            let y = my.min(area.height.saturating_sub(mh));
            let rect = Rect::new(area.x + x, area.y + y, mw, mh);
            frame.render_widget(Clear, rect);
            let menu_widget = Paragraph::new(Text::from(vec![Line::from("Hello, World.")]))
                .block(Block::default().borders(Borders::ALL))
                .style(Style::new().fg(Color::Black).bg(Color::Gray));
            frame.render_widget(menu_widget, rect);
        }
    })?;
    emit_status_hyperlink(rows)?;
    Ok(())
}

/// Paint a single clickable OSC 8 hyperlink on the bottom row, re-emitted each frame so it
/// survives ratatui's redraw. Quick eyeball check that links round-trip to the host
/// terminal (the crate-level model side is `Cell::hyperlink()`); not a kapollo feature.
fn emit_status_hyperlink(rows: u16) -> Result<()> {
    let mut out = io::stdout();
    let link = spike_support::osc8(
        "https://github.com/kenhia/kapollo",
        "[OSC 8 hyperlink test — click me]",
    );
    execute!(out, MoveTo(0, rows.saturating_sub(1)))?;
    out.write_all(b"\x1b[4;36m")?; // underline + cyan, so it reads as a link
    out.write_all(link.as_bytes())?;
    out.write_all(b"\x1b[0m")?;
    out.flush()?;
    Ok(())
}

/// Map an `alacritty_terminal` cell's colors/attributes onto a ratatui `Style`.
fn cell_style(cell: &GridCell) -> Style {
    let mut s = Style::default();
    if let Some(fg) = conv_color(cell.fg) {
        s = s.fg(fg);
    }
    if let Some(bg) = conv_color(cell.bg) {
        s = s.bg(bg);
    }
    let f = cell.flags;
    if f.contains(Flags::BOLD) {
        s = s.add_modifier(Modifier::BOLD);
    }
    if f.contains(Flags::ITALIC) {
        s = s.add_modifier(Modifier::ITALIC);
    }
    if f.contains(Flags::UNDERLINE) {
        s = s.add_modifier(Modifier::UNDERLINED);
    }
    if f.contains(Flags::INVERSE) {
        s = s.add_modifier(Modifier::REVERSED);
    }
    if f.contains(Flags::DIM) {
        s = s.add_modifier(Modifier::DIM);
    }
    s
}

fn conv_color(c: AnsiColor) -> Option<Color> {
    match c {
        AnsiColor::Spec(rgb) => Some(Color::Rgb(rgb.r, rgb.g, rgb.b)),
        AnsiColor::Indexed(i) => Some(Color::Indexed(i)),
        AnsiColor::Named(n) => named_color(n),
    }
}

fn named_color(n: NamedColor) -> Option<Color> {
    use NamedColor::*;
    Some(match n {
        Black => Color::Black,
        Red => Color::Red,
        Green => Color::Green,
        Yellow => Color::Yellow,
        Blue => Color::Blue,
        Magenta => Color::Magenta,
        Cyan => Color::Cyan,
        White => Color::Gray,
        BrightBlack => Color::DarkGray,
        BrightRed => Color::LightRed,
        BrightGreen => Color::LightGreen,
        BrightYellow => Color::LightYellow,
        BrightBlue => Color::LightBlue,
        BrightMagenta => Color::LightMagenta,
        BrightCyan => Color::LightCyan,
        BrightWhite => Color::White,
        // Foreground/Background/Cursor/dim variants → fall back to terminal default.
        _ => return None,
    })
}

fn row_col_span(a: Cell, b: Cell, content_row: usize, width: u16) -> (u16, u16) {
    let last = width.saturating_sub(1);
    let start = if content_row == a.0 { a.1 as u16 } else { 0 };
    let end = if content_row == b.0 { b.1 as u16 } else { last };
    (start.min(last), end.min(last))
}

fn forward_key(shell: &mut PtyShell, key: KeyEvent) -> Result<()> {
    if let Some(bytes) = key_to_bytes(key) {
        shell.write(&bytes)?;
    }
    Ok(())
}

fn key_to_bytes(key: KeyEvent) -> Option<Vec<u8>> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char(c) => {
            if ctrl && c.is_ascii_alphabetic() {
                Some(vec![(c.to_ascii_uppercase() as u8) & 0x1f])
            } else {
                Some(c.to_string().into_bytes())
            }
        }
        KeyCode::Enter => Some(vec![b'\r']),
        KeyCode::Tab => Some(vec![b'\t']),
        KeyCode::Backspace => Some(vec![0x7f]),
        KeyCode::Esc => Some(vec![0x1b]),
        KeyCode::Left => Some(b"\x1b[D".to_vec()),
        KeyCode::Right => Some(b"\x1b[C".to_vec()),
        KeyCode::Up => Some(b"\x1b[A".to_vec()),
        KeyCode::Down => Some(b"\x1b[B".to_vec()),
        KeyCode::Home => Some(b"\x1b[H".to_vec()),
        KeyCode::End => Some(b"\x1b[F".to_vec()),
        KeyCode::PageUp => Some(b"\x1b[5~".to_vec()),
        KeyCode::PageDown => Some(b"\x1b[6~".to_vec()),
        KeyCode::Delete => Some(b"\x1b[3~".to_vec()),
        _ => None,
    }
}

fn forward_mouse(shell: &mut PtyShell, m: MouseEvent) -> Result<()> {
    let col = m.column + 1;
    let row = m.row + 1;
    let seq = match m.kind {
        MouseEventKind::Down(b) => Some((mouse_btn(b), 'M')),
        MouseEventKind::Up(b) => Some((mouse_btn(b), 'm')),
        MouseEventKind::Drag(b) => Some((mouse_btn(b) + 32, 'M')),
        MouseEventKind::ScrollUp => Some((64, 'M')),
        MouseEventKind::ScrollDown => Some((65, 'M')),
        _ => None,
    };
    if let Some((btn, kind)) = seq {
        let bytes = format!("\x1b[<{btn};{col};{row}{kind}").into_bytes();
        shell.write(&bytes)?;
    }
    Ok(())
}

fn mouse_btn(b: MouseButton) -> u16 {
    match b {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    }
}
