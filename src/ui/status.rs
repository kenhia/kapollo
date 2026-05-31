//! Status rule rendering: a single horizontal rule above the input carrying the
//! current working directory (always) and the last exit code (only when
//! non-zero) (FR-006, FR-007, FR-008).

use std::path::Path;

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;

/// Build the status rule's content line: the cwd, and the exit code only when
/// it is non-zero (FR-007, FR-008). A leading rule glyph gives the line its
/// horizontal-rule appearance; `render` fills the remaining width.
pub fn line(cwd: &Path, last_exit: Option<i32>, color: bool) -> Line<'static> {
    let mut spans = vec![
        Span::raw("── "),
        Span::raw(cwd.display().to_string()),
        Span::raw(" "),
    ];
    if let Some(code) = last_exit {
        if code != 0 {
            spans.push(Span::raw(format!("[exit {code}] ")));
        }
    }
    let mut line = Line::from(spans);
    if color {
        line = line.style(Style::default().fg(Color::DarkGray));
    }
    line
}

/// Render the status rule into `area`, filling the remaining width with the rule
/// glyph so it reads as a single horizontal line.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let color = super::color_enabled();
    let base = line(&app.cwd, app.last_exit, color);

    let used = base.width() as u16;
    let mut spans = base.spans.clone();
    if used < area.width {
        spans.push(Span::raw("─".repeat((area.width - used) as usize)));
    }
    let mut rule = Line::from(spans);
    if color {
        rule = rule.style(Style::default().fg(Color::DarkGray));
    }
    frame.render_widget(Paragraph::new(rule), area);
}
