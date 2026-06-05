//! Transcript pad rendering: each block as its command line (prefixed with the
//! configurable `λ` prompt) followed by its captured output, with a blank line
//! between blocks and a truncation marker when output was dropped. The pad is
//! borderless so the renderer owns the full surface each frame (FR-002, FR-005,
//! FR-009, FR-010, FR-011). Supports independent scrolling: `scroll_offset`
//! counts lines up from the newest output (FR-021).

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::session::{Block, BlockState};

/// Display tab stop width. Tabs are expanded to spaces for rendering so ratatui
/// never emits a raw `\t` byte to the host terminal — a literal tab would jump
/// the host cursor to its own tab stop while ratatui still counts the cell as
/// one column, leaving stale cells uncleared and bleeding earlier rows into the
/// output (FR-001/002). Stored block bytes keep the original `\t` (FR-001).
const TAB_WIDTH: usize = 8;

/// Expand tab characters in `text` to spaces, advancing to the next multiple of
/// [`TAB_WIDTH`] from `start_col` (the display column where `text` begins).
fn expand_tabs(text: &str, start_col: usize) -> String {
    let mut out = String::with_capacity(text.len());
    let mut col = start_col;
    for ch in text.chars() {
        if ch == '\t' {
            let next = (col / TAB_WIDTH + 1) * TAB_WIDTH;
            for _ in col..next {
                out.push(' ');
            }
            col = next;
        } else {
            out.push(ch);
            col += 1;
        }
    }
    out
}

/// Build the transcript's display lines: a `λ`-prefixed command echo followed by
/// the block's normalized output, with a blank line separating consecutive
/// blocks. When `color` is enabled the prompt character wears `prompt_color`
/// (FR-009, FR-010, FR-011).
pub fn lines(
    blocks: &[Block],
    prompt_char: char,
    prompt_color: Color,
    color: bool,
) -> Vec<Line<'static>> {
    let prompt_style = if color {
        Style::default().fg(prompt_color)
    } else {
        Style::default()
    };

    let mut out: Vec<Line<'static>> = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        // A blank line separates each output block from the previous one.
        if index > 0 {
            out.push(Line::from(String::new()));
        }

        out.push(Line::from(vec![
            Span::styled(format!("{prompt_char} "), prompt_style),
            Span::raw(expand_tabs(&block.command, 2)),
        ]));

        if block.truncated() {
            out.push(Line::from("… output truncated …".to_string()));
        }

        let output = block.output_lossy();
        for line in output.lines() {
            out.push(Line::from(expand_tabs(line, 0)));
        }

        if block.state == BlockState::Interactive {
            out.push(Line::from("[interactive program]".to_string()));
        }
    }
    out
}

/// Render the transcript into `area` from the emulated grid: the viewport
/// window for the current scroll offset, mapped to styled ratatui lines. The
/// emulator owns the authoritative screen + scrollback, so carriage-return
/// progress lines update in place and alt-screen content never leaks into
/// scrollback (FR-001/004/005, SC-001/003).
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let max_scroll = app.grid.max_scroll();
    let offset = app.transcript.scroll_offset().min(max_scroll);
    // Record the metrics so keyboard scrolling can page and clamp (FR-021).
    app.transcript.record_view(area.height as usize, max_scroll);

    let (_, cols) = app.grid.size();
    let rows = app.grid.viewport_lines(offset);
    // Overlay the selection highlight when one is active and a full-screen
    // program is not in control of the screen (FR-007).
    let lines = match app.selection.range() {
        Some((a, b)) if !app.grid.is_alt_screen_active() => {
            let top = app.grid.top_stable_row(offset).max(0) as usize;
            let highlight = crate::selection::highlight_spans(rows.len(), cols, top, a, b);
            crate::grid::render::rows_to_lines_selected(&rows, cols, &highlight)
        }
        _ => crate::grid::render::rows_to_lines(&rows, cols),
    };

    let widget = Paragraph::new(lines);
    frame.render_widget(widget, area);

    // While a full-screen program owns the screen, show its cursor at the
    // emulator's reported position so editors place it correctly (FR-005).
    if app.grid.is_alt_screen_active() {
        let (cx, cy) = app.grid.cursor();
        let x = area.x + cx.min(area.width.saturating_sub(1));
        let y = area.y + cy.min(area.height.saturating_sub(1));
        frame.set_cursor_position(ratatui::layout::Position::new(x, y));
    }
}
