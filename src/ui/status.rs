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
/// glyph so it reads as a single horizontal line. A transient `notice` (e.g. a
/// copy failure) is shown after the cwd in a warning color so it is never
/// silently dropped (FR-013).
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let color = super::color_enabled();
    let base = line(&app.cwd, app.last_exit, color);

    let mut spans = base.spans.clone();
    // Reflect the most-recent block's elapsed runtime once it is sealed, beside
    // the exit indication, so the chrome shows both outcome and duration
    // (FR-023, T036). A still-running block has no duration yet.
    if let Some(dur) = app.store.last().and_then(|b| b.duration()) {
        let mut span = Span::raw(format!("({}) ", format_duration(dur)));
        if color {
            span = span.style(Style::default().fg(Color::DarkGray));
        }
        spans.push(span);
    }
    // Surface any pending notice prominently before the fill (FR-013).
    if let Some(notice) = app.notice.as_deref() {
        let mut span = Span::raw(format!("{notice} "));
        if color {
            span = span.style(Style::default().fg(Color::LightRed));
        }
        spans.push(span);
    }

    let used = Line::from(spans.clone()).width() as u16;
    if used < area.width {
        spans.push(Span::raw("─".repeat((area.width - used) as usize)));
    }
    let mut rule = Line::from(spans);
    if color {
        rule = rule.style(Style::default().fg(Color::DarkGray));
    }
    frame.render_widget(Paragraph::new(rule), area);
}

/// Format an elapsed command runtime compactly for the status rule: sub-minute
/// durations as fractional seconds (`0.42s`), longer ones as `1m03s`.
fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs_f64();
    if secs < 60.0 {
        format!("{secs:.2}s")
    } else {
        let total = d.as_secs();
        format!("{}m{:02}s", total / 60, total % 60)
    }
}
