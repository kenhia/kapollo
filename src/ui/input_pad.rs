//! Input pad rendering: the editable buffer the user is composing, with the
//! cursor shown and internal scrolling once the content exceeds the pad's
//! height cap (FR-009, FR-012). The pad is borderless; the status rule above it
//! provides the visual separation (FR-006).
//!
//! The pad's left prefix comes from one of two features: the prompt glyph
//! (`λ `, kwi #37) matching the transcript echo, or — when the divider-prompt
//! fold is active — the brought-down tail of the wrapped shell's own prompt
//! (kwi #47), which replaces the glyph and is dropped entirely on multiline
//! input so every line aligns flush left.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::input::InputMode;

/// The input pad's left prefix for this frame.
enum PadPrefix {
    /// No prefix; the text owns the full width.
    None,
    /// The prompt glyph + space on the first line (kwi #37); continuation
    /// lines indent to keep one left edge.
    Glyph(char, Style),
    /// The brought-down shell-prompt tail (kwi #47); single-line input only.
    Tail(String, Style),
}

impl PadPrefix {
    /// Display columns the prefix occupies (for cursor positioning).
    fn cols(&self) -> u16 {
        match self {
            PadPrefix::None => 0,
            PadPrefix::Glyph(..) => 2,
            PadPrefix::Tail(tail, _) => tail.chars().count() as u16,
        }
    }

    /// The prefix span for input-pad line `line_index`, if any.
    fn span(&self, line_index: usize) -> Option<Span<'static>> {
        match self {
            PadPrefix::None => None,
            PadPrefix::Glyph(glyph, style) => Some(if line_index == 0 {
                Span::styled(format!("{glyph} "), *style)
            } else {
                Span::raw("  ")
            }),
            PadPrefix::Tail(tail, style) => {
                (line_index == 0).then(|| Span::styled(tail.clone(), *style))
            }
        }
    }
}

/// The prefix style: `prompt_color` when idle, `running_color` while a command
/// is in flight (kwi #35), plain under `NO_COLOR` (where the status bar's
/// running marker carries the cue instead).
fn prompt_style(app: &App) -> Style {
    if super::color_enabled() {
        let color = if app.command_running() {
            app.config.running_color
        } else {
            app.config.prompt_color
        };
        Style::default().fg(color)
    } else {
        Style::default()
    }
}

/// Resolve this frame's prefix. The divider-prompt tail (kwi #47) takes
/// precedence over the glyph (kwi #37) while active — it IS the prompt — and
/// multiline input drops it so the buffer's lines align flush left.
fn pad_prefix(app: &App) -> PadPrefix {
    let style = prompt_style(app);
    if let Some((_, tail)) = app.divider_prompt() {
        if app.input.line_count() > 1 || tail.is_empty() {
            return PadPrefix::None;
        }
        return PadPrefix::Tail(tail, style);
    }
    if !app.config.input_prompt {
        return PadPrefix::None;
    }
    PadPrefix::Glyph(app.config.prompt_char, style)
}

/// Render the input pad into `area`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let (cursor_row, cursor_col) = app.input.cursor_row_col();
    let viewport = area.height as usize;

    // Scroll internally so the cursor row stays visible (FR-012).
    let top = (cursor_row + 1).saturating_sub(viewport);

    let prefix = pad_prefix(app);
    let widget = Paragraph::new(input_lines(app, &prefix)).scroll((top as u16, 0));
    frame.render_widget(widget, area);

    // Position the terminal cursor within the borderless area, after the
    // prefix when one is shown (kwi #37/#47).
    let cx = area.x + prefix.cols() + cursor_col as u16;
    let cy = area.y + (cursor_row.saturating_sub(top)) as u16;
    frame.set_cursor_position((cx, cy));
}

/// Build the pad's text as styled lines, highlighting the active selection
/// range with a reversed style so it reads as selected without relying on color
/// (sprint 005, US1; FR-003/004).
fn input_lines(app: &App, prefix: &PadPrefix) -> Vec<Line<'static>> {
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
        let line_index = lines.len();

        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.extend(prefix.span(line_index));
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
    fn glyph_prefix_reserves_two_columns_and_indents_continuations() {
        let style = Style::default().fg(Color::Red);
        let prefix = PadPrefix::Glyph('λ', style);
        assert_eq!(prefix.cols(), 2);

        let first = prefix.span(0).expect("prefix on the first line");
        assert_eq!(first.content, "λ ");
        assert_eq!(first.style, style);

        let cont = prefix.span(1).expect("continuation indent");
        assert_eq!(cont.content, "  ", "continuation lines keep the text edge");
        assert_eq!(cont.style, Style::default(), "indent carries no styling");
    }

    #[test]
    fn tail_prefix_matches_its_text_and_first_line_only() {
        // kwi #47: the brought-down shell-prompt tail.
        let style = Style::default().fg(Color::Red);
        let prefix = PadPrefix::Tail("> ".into(), style);
        assert_eq!(prefix.cols(), 2);
        assert_eq!(prefix.span(0).expect("tail on line 0").content, "> ");
        assert!(prefix.span(1).is_none(), "no indent for the tail prefix");
    }

    #[test]
    fn no_prefix_reclaims_the_full_width() {
        let prefix = PadPrefix::None;
        assert_eq!(prefix.cols(), 0);
        assert!(prefix.span(0).is_none());
        assert!(prefix.span(1).is_none());
    }
}
