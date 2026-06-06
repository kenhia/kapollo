//! The cosmetic dividing rule between the output pad and the input pad (sprint
//! 005). Purely decorative today — it is kapollo's visual lineage back to the
//! Apollo / Domain OS display manager. A future feature (kwi #47) may fold the
//! shell prompt into this rule; for now it is a single horizontal line.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// The glyph the rule is drawn with (U+2500 BOX DRAWINGS LIGHT HORIZONTAL).
const RULE: char = '─';

/// Build the rule's text filling exactly `width` columns.
pub fn rule(width: usize) -> String {
    RULE.to_string().repeat(width)
}

/// Render the dividing rule into `area` (a single row above the input pad).
pub fn render(frame: &mut Frame, area: Rect, color: bool) {
    let mut line = Line::from(rule(area.width as usize));
    if color {
        line = line.style(Style::default().fg(Color::DarkGray));
    }
    frame.render_widget(Paragraph::new(line), area);
}
