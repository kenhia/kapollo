//! UI layer: terminal lifecycle (RAII guard + panic hook) and the split-pad
//! layout. Per-widget rendering lives in the `transcript`, `input_pad`,
//! `divider`, and `status` submodules; full-screen handoff lives in
//! `passthrough` (US3).

pub mod divider;
pub mod input_pad;
pub mod passthrough;
pub mod status;
pub mod transcript;

use std::io::{self, Write};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::{Constraint, Layout, Rect};
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

/// The input pad's content height in lines, clamped to `[1, MAX_INPUT_LINES]`.
/// Shared by the layout and the grid-sizing path so the emulator's row count
/// always matches the rendered transcript pane (FR-004/006).
pub fn input_pad_height(input: &crate::input::InputPad) -> u16 {
    (input.as_str().split('\n').count() as u16).clamp(1, MAX_INPUT_LINES)
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
        execute!(
            out,
            EnterAlternateScreen,
            Hide,
            EnableMouseCapture,
            EnableBracketedPaste
        )?;
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
        execute!(
            out,
            DisableBracketedPaste,
            DisableMouseCapture,
            LeaveAlternateScreen,
            Show
        )?;
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

/// The chrome rows laid out beneath (and around) the transcript pane. The
/// `divider` and `status` rects are present only when their chrome is shown.
pub struct ChromeLayout {
    pub transcript: Rect,
    pub divider: Option<Rect>,
    pub input: Rect,
    pub status: Option<Rect>,
}

/// Compute the split-pad layout: the transcript fills the remaining space, an
/// optional dividing rule sits directly above the input pad, the input pad
/// occupies `input_height` lines, and an optional fixed status bar is pinned to
/// the very bottom (FR-005, FR-017).
pub fn chrome_layout(area: Rect, input_height: u16, divider: bool, status: bool) -> ChromeLayout {
    let mut constraints = vec![Constraint::Min(1)];
    if divider {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(input_height));
    if status {
        constraints.push(Constraint::Length(1));
    }
    let chunks = Layout::vertical(constraints).split(area);

    let mut idx = 0;
    let transcript = chunks[idx];
    idx += 1;
    let divider = divider.then(|| {
        let r = chunks[idx];
        idx += 1;
        r
    });
    let input = chunks[idx];
    idx += 1;
    let status = status.then(|| chunks[idx]);
    ChromeLayout {
        transcript,
        divider,
        input,
        status,
    }
}

/// Render the split-pad layout: borderless transcript on top, the dividing rule,
/// the input pad, then the fixed status bar pinned to the bottom (each piece
/// hidden per config / a short terminal).
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

    // While a full-screen program owns the screen (alt-screen), the emulator's
    // grid fills the whole area and kapollo's chrome is hidden so the program
    // gets the full viewport (FR-005, FR-018).
    if app.grid.is_alt_screen_active() {
        render_transcript(frame, area, app);
        return;
    }

    // The input pad is borderless now, so its height is just its line count.
    let input_height = input_pad_height(&app.input);
    let show_divider = app.config.divider.enabled;
    // The status bar is opt-out and hidden on a short terminal (FR-022, FR-026).
    let show_status = status::is_visible(app.config.status.enabled, area.height);

    let layout = chrome_layout(area, input_height, show_divider, show_status);
    render_transcript(frame, layout.transcript, app);
    if let Some(divider_area) = layout.divider {
        divider::render(frame, divider_area, color_enabled());
    }
    render_input(frame, layout.input, app);
    if let Some(status_area) = layout.status {
        render_status(frame, status_area, app);
    }
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
