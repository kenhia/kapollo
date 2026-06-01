//! S1 vertical slice on `vt100` (+ `tui-term` for render).
//!
//! Proves the grid-model "feel": PTY-backed shell rendered as a styled grid, with
//! content-coordinate mouse selection, scrollback, alt-screen handover, and explicit
//! (never implicit) copy. Throwaway spike code — see `specs/003-grid-spike/`.
//!
//! ## Coordinate model (the interesting part)
//!
//! `vt100` exposes scrollback as an **offset from the bottom** (`set_scrollback(n)`),
//! whereas `spike_support::coords` models a viewport as a `top_row` window over an
//! absolute history. We bridge the two with a fixed `BASE`:
//!
//! ```text
//! top_row = BASE - vt100_scrollback           (BASE = SCROLLBACK_LEN)
//! content_row = screen_to_content(top_row, y) = BASE - scrollback + y   (stable)
//! ```
//!
//! That lets us reuse the unit-tested helpers (`screen_to_content`,
//! `content_to_screen`, `auto_scroll`, `normalize`) unchanged. The friction — vt100
//! has no absolute row index, so content rows drift when new output arrives while
//! scrolled — is a genuine spike finding recorded in `delos/docs/s1-vt100.md`.

mod selection;

use std::io::{self, Write};
use std::time::Duration;

use anyhow::Result;
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
use spike_support::coords::{auto_scroll, content_to_screen, screen_to_content, Cell};
use spike_support::{copy_local, detect_mode, modes::ModeEvent, osc52_frame, PtyShell, PtySize};
use tui_term::widget::PseudoTerminal;
use vt100::Parser;

use selection::{LeftPress, SelectionController, Trigger};

/// Scrollback depth (rows). Doubles as the `BASE` offset for the coordinate bridge.
const SCROLLBACK_LEN: usize = 5000;
const BASE: usize = SCROLLBACK_LEN;

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

    /// While the child owns the alt screen or mouse, the spike suspends its own
    /// selection/scroll and forwards everything (FR-013/FR-014).
    fn override_active(&self) -> bool {
        self.alt_screen || self.child_mouse
    }
}

fn run(args: &Args) -> Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let size = terminal.size()?;
    let (mut rows, mut cols) = (size.height, size.width);

    let mut parser = Parser::new(rows, cols, SCROLLBACK_LEN);
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
    // top_row = BASE when fully scrolled to the live bottom (vt100 scrollback 0).
    let mut top_row: usize = BASE;
    let mut menu: Option<(u16, u16)> = None;

    loop {
        // Drain child output: update routing + feed the emulator.
        let mut got_output = false;
        while let Ok(chunk) = shell.output.try_recv() {
            routing.apply(&detect_mode(&chunk));
            parser.process(&chunk);
            got_output = true;
        }
        // Corroborate with vt100's own post-parse flags (authoritative).
        if got_output {
            let scr = parser.screen();
            routing.alt_screen = scr.alternate_screen();
            routing.child_mouse = scr.mouse_protocol_mode() != vt100::MouseProtocolMode::None;
        }
        if shell.try_wait()? {
            break; // child exited (EOF) → clean quit
        }

        draw(&mut terminal, &parser, &sel, top_row, rows, menu)?;

        if !event::poll(Duration::from_millis(16))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) => {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                // Ctrl-Q always quits (escape hatch, even under override).
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
                handle_key(key, &mut sel, &mut shell, &mut parser, &mut top_row, rows)?;
            }
            Event::Mouse(m) => {
                if routing.override_active() {
                    forward_mouse(&mut shell, m)?;
                    continue;
                }
                handle_mouse(
                    m,
                    &mut sel,
                    &mut shell,
                    &mut parser,
                    &mut top_row,
                    rows,
                    cols,
                    &mut menu,
                    args,
                )?;
            }
            Event::Resize(w, h) => {
                cols = w;
                rows = h;
                parser.screen_mut().set_size(rows, cols);
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

#[allow(clippy::too_many_arguments)]
fn handle_mouse(
    m: MouseEvent,
    sel: &mut SelectionController,
    shell: &mut PtyShell,
    parser: &mut Parser,
    top_row: &mut usize,
    rows: u16,
    cols: u16,
    menu: &mut Option<(u16, u16)>,
    args: &Args,
) -> Result<()> {
    let height = rows as usize;
    let shift = m.modifiers.contains(KeyModifiers::SHIFT);
    let cell_at =
        |top: usize, y: u16| -> Cell { (screen_to_content(top, y as usize), m.column as usize) };

    match m.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            *menu = None;
            let cell = cell_at(*top_row, m.row);
            match sel.left_press(cell, shift) {
                LeftPress::ForwardToChild => forward_mouse(shell, m)?,
                LeftPress::StartedDrag | LeftPress::Cancelled => {}
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            // Auto-scroll when dragging past the top/bottom edge (FR-010).
            let drag_y: isize = if m.row == 0 {
                -1
            } else if m.row as usize >= height.saturating_sub(1) {
                height as isize
            } else {
                m.row as isize
            };
            *top_row = auto_scroll(*top_row, drag_y, height, BASE + height);
            apply_scrollback(parser, *top_row);
            let cell = cell_at(*top_row, m.row.min(rows.saturating_sub(1)));
            sel.drag_to(cell);
        }
        MouseEventKind::Up(MouseButton::Left) => {
            sel.release();
        }
        MouseEventKind::Down(MouseButton::Right) => match sel.right_press() {
            Trigger::Copy(a, b) => copy_selection(parser, *top_row, rows, cols, a, b, args)?,
            Trigger::ContextMenu => *menu = Some((m.column, m.row)),
            Trigger::Sigint => {}
        },
        MouseEventKind::ScrollUp => {
            *top_row = top_row.saturating_sub(3);
            apply_scrollback(parser, *top_row);
        }
        MouseEventKind::ScrollDown => {
            *top_row = (*top_row + 3).min(BASE);
            apply_scrollback(parser, *top_row);
        }
        _ => {}
    }
    Ok(())
}

fn handle_key(
    key: KeyEvent,
    sel: &mut SelectionController,
    shell: &mut PtyShell,
    parser: &mut Parser,
    top_row: &mut usize,
    rows: u16,
) -> Result<()> {
    let height = rows as usize;
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => match sel.ctrl_c() {
            Trigger::Copy(a, b) => {
                let cols = parser.screen().size().1;
                copy_selection(parser, *top_row, rows, cols, a, b, &osc_args())?;
            }
            Trigger::Sigint => shell.write(&[0x03])?,
            Trigger::ContextMenu => {}
        },
        KeyCode::Esc if sel.cancel() => {}
        KeyCode::PageUp => {
            *top_row = top_row.saturating_sub(height);
            apply_scrollback(parser, *top_row);
        }
        KeyCode::PageDown => {
            *top_row = (*top_row + height).min(BASE);
            apply_scrollback(parser, *top_row);
        }
        _ => {
            if let Some(bytes) = key_to_bytes(key) {
                shell.write(&bytes)?;
            }
        }
    }
    Ok(())
}

/// Default OSC-52 args for the keyboard copy path (keyboard has no per-press flag).
fn osc_args() -> Args {
    Args {
        shell: String::new(),
        clipboard: Clipboard::Osc52,
    }
}

fn apply_scrollback(parser: &mut Parser, top_row: usize) {
    // vt100 scrollback = BASE - top_row (clamped by vt100 to real history depth).
    parser.screen_mut().set_scrollback(BASE - top_row.min(BASE));
}

#[allow(clippy::too_many_arguments)]
fn copy_selection(
    parser: &Parser,
    top_row: usize,
    rows: u16,
    cols: u16,
    a: Cell,
    b: Cell,
    args: &Args,
) -> Result<()> {
    let height = rows as usize;
    // Map content rows back to the visible viewport, clamping off-screen ends.
    let sr = content_to_screen(top_row, a.0, height).unwrap_or(0) as u16;
    let er = content_to_screen(top_row, b.0, height).unwrap_or(height - 1) as u16;
    let sc = (a.1 as u16).min(cols.saturating_sub(1));
    let ec = (b.1 as u16).min(cols.saturating_sub(1));
    let text = parser.screen().contents_between(sr, sc, er, ec);
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
    parser: &Parser,
    sel: &SelectionController,
    top_row: usize,
    rows: u16,
    menu: Option<(u16, u16)>,
) -> Result<()> {
    let height = rows as usize;
    terminal.draw(|frame| {
        let area = frame.area();
        let term = PseudoTerminal::new(parser.screen());
        frame.render_widget(term, area);

        // Selection highlight overlay (content coords → visible cells).
        if let Some((a, b)) = sel.range() {
            let width = area.width;
            let buf = frame.buffer_mut();
            for content_row in a.0..=b.0 {
                let Some(y) = content_to_screen(top_row, content_row, height) else {
                    continue;
                };
                let (c0, c1) = row_col_span(a, b, content_row, width);
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

        // Trivial "Hello, World." context menu (FR-019) — proves render + route only.
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
    Ok(())
}

/// Column span to highlight on `content_row` for a normalized selection `a..=b`.
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

/// Encode a key event into the bytes a terminal would send to the child PTY.
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

/// Encode a mouse event as an SGR (1006) sequence for forwarding to the child.
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
