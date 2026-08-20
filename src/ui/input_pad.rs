//! Input pad rendering: the editable buffer the user is composing, with the
//! cursor shown and internal scrolling once the content exceeds the pad's
//! height cap (FR-009, FR-012). The pad is borderless; the status rule above it
//! provides the visual separation (FR-006). When `input_prompt` is enabled the
//! first line is prefixed with the prompt glyph (`λ `), matching the transcript
//! command echo, and continuation lines are indented to align (kwi #37).

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::input::InputMode;

/// Display columns the prompt prefix occupies (`λ ` on the first line, matching
/// indentation on the rest); `0` when the input prompt is disabled.
pub fn prompt_cols(enabled: bool) -> u16 {
    if enabled {
        2
    } else {
        0
    }
}

/// The left prefix for input-pad line `line_index`: the styled prompt glyph on
/// the first line, matching indentation on continuation lines so the text
/// keeps one left edge, or nothing when the input prompt is disabled (kwi #37).
fn prefix_span(prompt: Option<(char, Style)>, line_index: usize) -> Option<Span<'static>> {
    let (glyph, style) = prompt?;
    Some(if line_index == 0 {
        Span::styled(format!("{glyph} "), style)
    } else {
        Span::raw("  ")
    })
}

/// The input-pad prompt glyph and its style, or `None` when disabled. The
/// glyph wears `prompt_color` when idle and the running color while a command
/// is in flight (kwi #35), plain under `NO_COLOR`.
fn prompt(app: &App) -> Option<(char, Style)> {
    if !app.config.input_prompt {
        return None;
    }
    let style = if super::color_enabled() {
        Style::default().fg(app.config.prompt_color)
    } else {
        Style::default()
    };
    Some((app.config.prompt_char, style))
}

/// Render the input pad into `area`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let (cursor_row, cursor_col) = app.input.cursor_row_col();
    let viewport = area.height as usize;

    // Scroll internally so the cursor row stays visible (FR-012).
    let top = (cursor_row + 1).saturating_sub(viewport);

    let widget = Paragraph::new(input_lines(app)).scroll((top as u16, 0));
    frame.render_widget(widget, area);

    // Position the terminal cursor within the borderless area, after the
    // prompt prefix when one is shown (kwi #37).
    let prefix = prompt_cols(app.config.input_prompt);
    let cx = area.x + prefix + cursor_col as u16;
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
    let prompt = prompt(app);

    let mut lines = Vec::new();
    let mut global = 0usize; // running char offset into the buffer
    for line in buffer.split('\n') {
        let chars: Vec<char> = line.chars().collect();
        let n = chars.len();
        let line_start = global;
        let line_index = lines.len();

        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.extend(prefix_span(prompt, line_index));
        match selection {
            Some((s, e)) => {
                let a = s.max(line_start);
                let b = e.min(line_start + n);
                if a < b {
                    let (la, lb) = (a - line_start, b - line_start);
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
                } else {
                    spans.push(Span::raw(line.to_string()));
                }
            }
            None => spans.push(Span::raw(line.to_string())),
        }
        let rendered = Line::from(spans);
        // In LAAT, paint the highlighted line and any probable-failure line with
        // a line-level background (sprint 007; FR-002/FR-004), honoring the color
        // gate — without color the caret still conveys the highlight position.
        let rendered = match laat_line_style(app, line_index) {
            Some(style) => rendered.style(style),
            None => rendered,
        };
        lines.push(rendered);

        global += n + 1; // +1 accounts for the '\n' separator
    }
    lines
}

/// The line-level background style for LAAT line `idx`: a highlight background
/// on the stepped line and a distinct probable-failure background on flagged
/// lines. `None` outside LAAT, when color is disabled, or for ordinary lines.
fn laat_line_style(app: &App, idx: usize) -> Option<Style> {
    if app.mode != InputMode::Laat || !super::color_enabled() {
        return None;
    }
    let laat = app.laat.as_ref()?;
    if laat.is_failed(idx) {
        Some(Style::default().bg(Color::Red).fg(Color::White))
    } else if idx == laat.highlight {
        Some(Style::default().bg(Color::Blue).fg(Color::White))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_cols_reserves_two_columns_when_enabled() {
        assert_eq!(prompt_cols(true), 2);
        assert_eq!(prompt_cols(false), 0);
    }

    #[test]
    fn first_line_wears_the_prompt_and_continuations_align() {
        let style = Style::default().fg(Color::Red);
        let first = prefix_span(Some(('λ', style)), 0).expect("prefix");
        assert_eq!(first.content, "λ ");
        assert_eq!(first.style, style);

        let cont = prefix_span(Some(('λ', style)), 1).expect("prefix");
        assert_eq!(cont.content, "  ", "continuation lines keep the text edge");
        assert_eq!(cont.style, Style::default(), "indent carries no styling");
    }

    #[test]
    fn disabled_prompt_reclaims_the_full_width() {
        assert!(prefix_span(None, 0).is_none());
        assert!(prefix_span(None, 1).is_none());
    }
}
