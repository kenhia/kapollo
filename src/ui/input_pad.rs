//! Input pad rendering: the editable buffer the user is composing, with the
//! cursor shown and internal scrolling once the content exceeds the pad's
//! height cap (FR-009, FR-012). The pad is borderless; the status rule above it
//! provides the visual separation (FR-006).

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;

/// Render the input pad into `area`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let (cursor_row, cursor_col) = app.input.cursor_row_col();
    let viewport = area.height as usize;

    // Scroll internally so the cursor row stays visible (FR-012).
    let top = (cursor_row + 1).saturating_sub(viewport);

    let widget = Paragraph::new(input_lines(app)).scroll((top as u16, 0));
    frame.render_widget(widget, area);

    // Position the terminal cursor within the borderless area.
    let cx = area.x + cursor_col as u16;
    let cy = area.y + (cursor_row.saturating_sub(top)) as u16;
    frame.set_cursor_position((cx, cy));
}

/// Build the pad's text as styled lines, highlighting the active selection
/// range with a reversed style so it reads as selected without relying on color
/// (sprint 005, US1; FR-003/004).
fn input_lines(app: &App) -> Vec<Line<'static>> {
    let buffer = app.input.as_str();
    let selection = app
        .input
        .selection()
        .filter(|s| !s.is_empty())
        .map(|s| s.range());
    let highlight = Style::default().add_modifier(Modifier::REVERSED);

    let mut lines = Vec::new();
    let mut global = 0usize; // running char offset into the buffer
    for line in buffer.split('\n') {
        let chars: Vec<char> = line.chars().collect();
        let n = chars.len();
        let line_start = global;

        let rendered = match selection {
            Some((s, e)) => {
                let a = s.max(line_start);
                let b = e.min(line_start + n);
                if a < b {
                    let (la, lb) = (a - line_start, b - line_start);
                    let mut spans = Vec::new();
                    let before: String = chars[..la].iter().collect();
                    if !before.is_empty() {
                        spans.push(Span::raw(before));
                    }
                    let mid: String = chars[la..lb].iter().collect();
                    spans.push(Span::styled(mid, highlight));
                    let after: String = chars[lb..].iter().collect();
                    if !after.is_empty() {
                        spans.push(Span::raw(after));
                    }
                    Line::from(spans)
                } else {
                    Line::from(line.to_string())
                }
            }
            None => Line::from(line.to_string()),
        };
        lines.push(rendered);

        global += n + 1; // +1 accounts for the '\n' separator
    }
    lines
}
