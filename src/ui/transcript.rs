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

/// Render the transcript into `area`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let lines = lines(
        app.transcript.blocks(),
        app.config.prompt_char,
        app.config.prompt_color,
        super::color_enabled(),
    );

    // Pin the view to the newest output by default; `scroll_offset` scrolls up.
    // The pad is borderless, so the whole area height is the viewport.
    let total_lines = lines.len();
    let viewport = area.height as usize;
    let max_top = total_lines.saturating_sub(viewport);
    // Record the metrics so keyboard scrolling can page and clamp (FR-021).
    app.transcript.record_view(viewport, max_top);
    let top = max_top.saturating_sub(app.transcript.scroll_offset()) as u16;

    let widget = Paragraph::new(lines).scroll((top, 0));
    frame.render_widget(widget, area);
}
