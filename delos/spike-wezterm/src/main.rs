//! S3 vertical slice on `wezterm-term` (the `Terminal`/`Screen` model).
//!
//! Same "feel" target as S1/S2, third engine: a PTY-backed shell fed to a
//! `wezterm_term::Terminal` via `advance_bytes`, rendered to a ratatui `Buffer` by
//! reading `Screen` lines (grapheme-aware `visible_cells`), with content-coordinate
//! mouse selection, scrollback, alt-screen handover, and explicit copy. Throwaway spike.
//!
//! ## Coordinate model — the wezterm difference
//!
//! Unlike S1 (`vt100`) and S2 (`alacritty_terminal`), `wezterm-term` does **not** track a
//! display-scroll position itself — the embedding app owns it. But it gives us something
//! better: **`StableRowIndex`**, a *true absolute* row id that survives scrollback purges.
//! So our `top_row` (content row of the visible top line) is simply the real stable index
//! of that line — no `BASE` offset hack needed (S1/S2 faked absolute ids with
//! `BASE - scroll`). Selection anchored to stable ids cannot drift as output streams in.
//!
//! ```text
//! len          = screen.scrollback_rows()            (total lines incl. history)
//! top_phys     = len - physical_rows - scroll_offset (we manage scroll_offset)
//! top_row      = screen.phys_to_stable_row_index(top_phys)   ← absolute, drift-free
//! content_row  = screen_to_content(top_row, y) = top_row + y
//! ```

mod selection;

use std::io::{self, Write};
use std::sync::mpsc::Sender;
use std::sync::Arc;
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
use ratatui::text::{Line as TuiLine, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Terminal as RatTerminal;
use spike_support::coords::{content_to_screen, screen_to_content, Cell};
use spike_support::{copy_local, detect_mode, modes::ModeEvent, osc52_frame, PtyShell, PtySize};

use wezterm_term::color::{ColorAttribute, ColorPalette, SrgbaTuple};
use wezterm_term::{
    CellAttributes, Intensity, Terminal, TerminalConfiguration, TerminalSize, Underline,
};

use selection::{LeftPress, SelectionController, Trigger};

/// Scrollback depth (rows). We override `TerminalConfiguration::scrollback_size`.
const SCROLLBACK_LEN: usize = 5000;

/// Lines emitted by the `Ctrl-F` flood self-test (manual T022/T027 drift harness).
const FLOOD_LINES: usize = 2000;

type SpikeTerm = Terminal;

/// Minimal terminal configuration. `color_palette` is the only required method; we also
/// bump the scrollback to match S1/S2. Everything else uses the trait defaults.
#[derive(Debug)]
struct SpikeConfig {
    palette: ColorPalette,
}

impl TerminalConfiguration for SpikeConfig {
    fn scrollback_size(&self) -> usize {
        SCROLLBACK_LEN
    }
    fn color_palette(&self) -> ColorPalette {
        self.palette.clone()
    }
}

/// `wezterm_term::Terminal` writes its answerbacks (DSR replies, device attributes, …)
/// into a `Box<dyn Write>`. We funnel those bytes to the PTY via a channel, the same role
/// `EventProxy::PtyWrite` played in S2.
struct ChannelWriter {
    tx: Sender<Vec<u8>>,
}

impl Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let _ = self.tx.send(buf.to_vec());
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
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

fn make_size(rows: u16, cols: u16) -> TerminalSize {
    TerminalSize {
        rows: rows as usize,
        cols: cols as usize,
        pixel_width: 0,
        pixel_height: 0,
        dpi: 0,
    }
}

fn run(args: &Args) -> Result<()> {
    let mut terminal = RatTerminal::new(CrosstermBackend::new(io::stdout()))?;
    let size = terminal.size()?;
    let (mut rows, mut cols) = (size.height, size.width);

    let (tx, ans_rx) = std::sync::mpsc::channel::<Vec<u8>>();
    let config: Arc<dyn TerminalConfiguration + Send + Sync> = Arc::new(SpikeConfig {
        palette: ColorPalette::default(),
    });
    let mut term: SpikeTerm = Terminal::new(
        make_size(rows, cols),
        config,
        "spike-wezterm",
        "0.1.0",
        Box::new(ChannelWriter { tx }),
    );

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
    // We own the scroll position (rows scrolled up from the live bottom).
    let mut scroll_offset: usize = 0;

    loop {
        let mut got_output = false;
        while let Ok(chunk) = shell.output.try_recv() {
            routing.apply(&detect_mode(&chunk));
            term.advance_bytes(&chunk);
            got_output = true;
        }
        // Flush any emulator → PTY write-backs (device-status replies, etc.).
        while let Ok(bytes) = ans_rx.try_recv() {
            shell.write(&bytes)?;
        }
        // Corroborate routing with the emulator's authoritative state.
        if got_output {
            routing.alt_screen = term.is_alt_screen_active();
            routing.child_mouse = term.is_mouse_grabbed();
        }
        if shell.try_wait()? {
            break;
        }

        draw(&mut terminal, &term, &sel, scroll_offset, rows, cols, menu)?;

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
                handle_key(
                    key,
                    &mut sel,
                    &mut shell,
                    &term,
                    &mut scroll_offset,
                    rows,
                    cols,
                )?;
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
                    &term,
                    &mut scroll_offset,
                    rows,
                    cols,
                    &mut menu,
                    args,
                )?;
            }
            Event::Resize(w, h) => {
                cols = w;
                rows = h;
                term.resize(make_size(rows, cols));
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

/// Clamp the scroll offset to the available history above the live viewport.
fn max_scroll(term: &SpikeTerm, rows: u16) -> usize {
    let screen = term.screen();
    screen.scrollback_rows().saturating_sub(rows as usize)
}

/// Current `top_row`: the absolute `StableRowIndex` of the visible top line.
fn current_top_row(term: &SpikeTerm, scroll_offset: usize) -> usize {
    let screen = term.screen();
    let len = screen.scrollback_rows();
    let phys = screen.physical_rows;
    let off = scroll_offset.min(len.saturating_sub(phys));
    let top_phys = len.saturating_sub(phys + off);
    screen.phys_to_stable_row_index(top_phys).max(0) as usize
}

/// Build a `rows × cols` grid of `(symbol, style)` for the visible viewport, plus the
/// `top_row` (stable id of the top line). Reading via `visible_cells` keeps wezterm's
/// grapheme/wide-char fidelity intact.
fn build_viewport(
    term: &SpikeTerm,
    scroll_offset: usize,
    rows: u16,
    cols: u16,
) -> (usize, Vec<Vec<(String, Style)>>) {
    let screen = term.screen();
    let len = screen.scrollback_rows();
    let phys = screen.physical_rows;
    let width = cols as usize;
    let height = rows as usize;
    let off = scroll_offset.min(len.saturating_sub(phys));
    let top_phys = len.saturating_sub(phys + off);
    let top_row = screen.phys_to_stable_row_index(top_phys).max(0) as usize;
    let lines = screen.lines_in_phys_range(top_phys..top_phys + phys);

    let mut grid = vec![vec![(" ".to_string(), Style::default()); width]; height];
    for (y, line) in lines.iter().enumerate().take(height) {
        for cell in line.visible_cells() {
            let x = cell.cell_index();
            if x >= width {
                continue;
            }
            let s = cell.str();
            let sym = if s.is_empty() {
                " ".to_string()
            } else {
                s.to_string()
            };
            grid[y][x] = (sym, cell_style(cell.attrs()));
        }
    }
    (top_row, grid)
}

#[allow(clippy::too_many_arguments)]
fn handle_mouse(
    m: MouseEvent,
    sel: &mut SelectionController,
    shell: &mut PtyShell,
    term: &SpikeTerm,
    scroll_offset: &mut usize,
    rows: u16,
    cols: u16,
    menu: &mut Option<(u16, u16)>,
    args: &Args,
) -> Result<()> {
    let height = rows as usize;
    let shift = m.modifiers.contains(KeyModifiers::SHIFT);
    let max = max_scroll(term, rows);

    match m.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            *menu = None;
            let top = current_top_row(term, *scroll_offset);
            let cell = (screen_to_content(top, m.row as usize), m.column as usize);
            match sel.left_press(cell, shift) {
                LeftPress::ForwardToChild => forward_mouse(shell, m)?,
                LeftPress::StartedDrag | LeftPress::Cancelled => {}
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            // Auto-scroll when dragging past the top/bottom edge (FR-010).
            if m.row == 0 {
                *scroll_offset = (*scroll_offset + 1).min(max);
            } else if m.row as usize >= height.saturating_sub(1) {
                *scroll_offset = scroll_offset.saturating_sub(1);
            }
            let top = current_top_row(term, *scroll_offset);
            let y = m.row.min(rows.saturating_sub(1)) as usize;
            let cell = (screen_to_content(top, y), m.column as usize);
            sel.drag_to(cell);
        }
        MouseEventKind::Up(MouseButton::Left) => {
            sel.release();
        }
        MouseEventKind::Down(MouseButton::Right) => match sel.right_press() {
            Trigger::Copy(a, b) => copy_selection(term, *scroll_offset, rows, cols, a, b, args)?,
            Trigger::ContextMenu => *menu = Some((m.column, m.row)),
            Trigger::Sigint => {}
        },
        MouseEventKind::ScrollUp => *scroll_offset = (*scroll_offset + 3).min(max),
        MouseEventKind::ScrollDown => *scroll_offset = scroll_offset.saturating_sub(3),
        _ => {}
    }
    Ok(())
}

fn handle_key(
    key: KeyEvent,
    sel: &mut SelectionController,
    shell: &mut PtyShell,
    term: &SpikeTerm,
    scroll_offset: &mut usize,
    rows: u16,
    cols: u16,
) -> Result<()> {
    let max = max_scroll(term, rows);
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => match sel.ctrl_c() {
            Trigger::Copy(a, b) => {
                copy_selection(term, *scroll_offset, rows, cols, a, b, &osc_args())?;
            }
            Trigger::Sigint => shell.write(&[0x03])?,
            Trigger::ContextMenu => {}
        },
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let top = current_top_row(term, *scroll_offset);
            flood_selftest(sel, shell, top, rows, cols)?;
        }
        KeyCode::Esc if sel.cancel() => {}
        KeyCode::PageUp => *scroll_offset = (*scroll_offset + rows as usize).min(max),
        KeyCode::PageDown => *scroll_offset = scroll_offset.saturating_sub(rows as usize),
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

fn copy_selection(
    term: &SpikeTerm,
    scroll_offset: usize,
    rows: u16,
    cols: u16,
    a: Cell,
    b: Cell,
    args: &Args,
) -> Result<()> {
    let height = rows as usize;
    let width = cols as usize;
    let (top_row, grid) = build_viewport(term, scroll_offset, rows, cols);
    let sr = content_to_screen(top_row, a.0, height).unwrap_or(0);
    let er = content_to_screen(top_row, b.0, height).unwrap_or(height - 1);

    let mut lines: Vec<String> = Vec::new();
    for (sy, row) in grid.iter().enumerate().take(er + 1).skip(sr) {
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
        let text: String = row[c0..=c1].iter().map(|(s, _)| s.as_str()).collect();
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

#[allow(clippy::too_many_arguments)]
fn draw(
    terminal: &mut RatTerminal<CrosstermBackend<io::Stdout>>,
    term: &SpikeTerm,
    sel: &SelectionController,
    scroll_offset: usize,
    rows: u16,
    cols: u16,
    menu: Option<(u16, u16)>,
) -> Result<()> {
    let height = rows as usize;
    let (top_row, grid) = build_viewport(term, scroll_offset, rows, cols);
    terminal.draw(|frame| {
        let area = frame.area();
        let buf = frame.buffer_mut();

        let max_r = (rows.min(area.height)) as usize;
        let max_c = (cols.min(area.width)) as usize;
        for (vr, row) in grid.iter().enumerate().take(max_r) {
            for (c, (sym, style)) in row.iter().enumerate().take(max_c) {
                let x = area.x + c as u16;
                let y = area.y + vr as u16;
                if let Some(bc) = buf.cell_mut((x, y)) {
                    bc.set_symbol(sym);
                    bc.set_style(*style);
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
            let menu_widget = Paragraph::new(Text::from(vec![TuiLine::from("Hello, World.")]))
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
/// terminal (the crate-level model side is `attrs.hyperlink()`); not a kapollo feature.
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

/// Map a `wezterm-term` cell's colors/attributes onto a ratatui `Style`.
///
/// Mirrors S2's policy so the comparison is fair: the terminal **default** fg/bg are left
/// *unset* (no SGR emitted) so the host terminal's theme and background — including a
/// background image — show through, and palette indices are forwarded as ANSI indices
/// (host-themed) rather than resolved to fixed RGB. This is the fix for the
/// "painted on top" / palette-mismatch artifact in the first S3 cut: previously every cell
/// was resolved through wezterm's *own* default palette to solid RGB, which painted an
/// opaque background over the host and ignored the user's theme.
fn cell_style(attrs: &CellAttributes) -> Style {
    let mut s = Style::default();
    if let Some(fg) = conv_color(attrs.foreground()) {
        s = s.fg(fg);
    }
    if let Some(bg) = conv_color(attrs.background()) {
        s = s.bg(bg);
    }
    match attrs.intensity() {
        Intensity::Bold => s = s.add_modifier(Modifier::BOLD),
        Intensity::Half => s = s.add_modifier(Modifier::DIM),
        Intensity::Normal => {}
    }
    if attrs.italic() {
        s = s.add_modifier(Modifier::ITALIC);
    }
    if attrs.underline() != Underline::None {
        s = s.add_modifier(Modifier::UNDERLINED);
    }
    if attrs.reverse() {
        s = s.add_modifier(Modifier::REVERSED);
    }
    s
}

/// Convert a wezterm `ColorAttribute` to a ratatui color, returning `None` for the
/// terminal **default** so we emit no SGR and the host theme/background shows through.
/// Palette indices (the basic 16 + 256-color cube) map to `Color::Indexed`, which the host
/// terminal themes — matching what the user normally sees; only genuine truecolor becomes
/// a fixed `Color::Rgb`.
fn conv_color(c: ColorAttribute) -> Option<Color> {
    match c {
        ColorAttribute::Default => None,
        ColorAttribute::PaletteIndex(i) => Some(Color::Indexed(i)),
        ColorAttribute::TrueColorWithPaletteFallback(rgb, _) => Some(srgba_to_color(rgb)),
        ColorAttribute::TrueColorWithDefaultFallback(rgb) => Some(srgba_to_color(rgb)),
    }
}

fn srgba_to_color(c: SrgbaTuple) -> Color {
    let (r, g, b, _) = c.to_srgb_u8();
    Color::Rgb(r, g, b)
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
