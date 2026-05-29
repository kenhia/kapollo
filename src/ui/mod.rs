//! UI layer: terminal lifecycle (RAII guard + panic hook) and the split-pad
//! layout. Per-widget rendering lives in the `transcript`, `input_pad`, and
//! `status` submodules; full-screen handoff lives in `passthrough` (US3).

pub mod input_pad;
pub mod passthrough;
pub mod status;
pub mod transcript;

use std::io::{self, Write};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

use crate::app::App;

/// Maximum input-pad content height in lines before it scrolls internally.
const MAX_INPUT_LINES: u16 = 10;

/// Minimum terminal dimensions below which the split-pad layout cannot render
/// legibly; a single-line hint is shown instead (spec edge cases, T047).
const MIN_COLS: u16 = 20;
const MIN_ROWS: u16 = 6;

/// Whether kapollo may style its own chrome with color. Honors the `NO_COLOR`
/// convention: any non-empty value disables color (FR-031, T046).
pub fn color_enabled() -> bool {
    match std::env::var_os("NO_COLOR") {
        Some(value) => value.is_empty(),
        None => true,
    }
}

/// RAII guard that puts the terminal into raw mode + the alternate screen on
/// creation and unconditionally restores it on drop (FR-025).
#[derive(Debug)]
pub struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    /// Enter raw mode and the alternate screen, hiding the cursor. Best-effort
    /// enables the Kitty keyboard protocol so Shift+Enter is distinguishable
    /// from Enter where the terminal supports it (FR-010, research R5).
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut out = io::stdout();
        execute!(out, EnterAlternateScreen, Hide)?;
        // Ignore errors: terminals without the protocol simply fall back to
        // Alt+Enter for newline insertion.
        let _ = execute!(
            out,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        );
        Ok(Self { active: true })
    }

    /// Restore the terminal to a clean state: pop keyboard enhancements, leave
    /// the alternate screen, show the cursor, and disable raw mode.
    pub fn restore() -> io::Result<()> {
        let mut out = io::stdout();
        let _ = execute!(out, PopKeyboardEnhancementFlags);
        execute!(out, LeaveAlternateScreen, Show)?;
        disable_raw_mode()?;
        out.flush()?;
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = TerminalGuard::restore();
        }
    }
}

/// Install a panic hook that restores the terminal before the default hook
/// runs, so a panic never leaves the terminal corrupted (FR-026).
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = TerminalGuard::restore();
        tracing::error!("panic: {info}");
        default_hook(info);
    }));
}

/// Render the split-pad layout: transcript on top, input pad below, status line
/// at the bottom.
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Degrade gracefully on a tiny terminal rather than rendering a corrupt or
    // panicking layout (spec edge cases, T047).
    if area.width < MIN_COLS || area.height < MIN_ROWS {
        let hint = ratatui::widgets::Paragraph::new("kapollo: terminal too small")
            .wrap(ratatui::widgets::Wrap { trim: true });
        frame.render_widget(hint, area);
        return;
    }

    let input_lines = (app.input.as_str().split('\n').count() as u16).clamp(1, MAX_INPUT_LINES);
    let input_height = input_lines + 2; // account for the surrounding border

    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(input_height),
        Constraint::Length(1),
    ])
    .split(area);

    render_transcript(frame, chunks[0], app);
    render_input(frame, chunks[1], app);
    render_status(frame, chunks[2], app);
}

fn render_transcript(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    transcript::render(frame, area, app);
}

fn render_input(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    input_pad::render(frame, area, app);
}

fn render_status(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    status::render(frame, area, app);
}
