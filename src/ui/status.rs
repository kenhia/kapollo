//! Status line rendering: current working directory and the last exit code
//! (FR-033).

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;

/// Render the status line into `area`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "?".to_string());
    let exit = match app.last_exit {
        Some(code) => code.to_string(),
        None => "-".to_string(),
    };
    let mut widget = Paragraph::new(format!(" {cwd}    exit: {exit}"));
    // Color the status bar unless NO_COLOR is set (FR-031).
    if super::color_enabled() {
        widget = widget.style(Style::default().bg(Color::DarkGray).fg(Color::White));
    }
    frame.render_widget(widget, area);
}
