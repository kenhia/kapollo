//! Input pad rendering: the editable buffer the user is composing, with the
//! cursor shown and internal scrolling once the content exceeds the pad's
//! height cap (FR-009, FR-012).

use ratatui::layout::Rect;
use ratatui::widgets::{Block as BorderBlock, Paragraph};
use ratatui::Frame;

use crate::app::App;

/// Render the input pad into `area`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let (cursor_row, cursor_col) = app.input.cursor_row_col();
    let viewport = area.height.saturating_sub(2) as usize; // borders

    // Scroll internally so the cursor row stays visible (FR-012).
    let top = (cursor_row + 1).saturating_sub(viewport);

    let widget = Paragraph::new(app.input.as_str())
        .scroll((top as u16, 0))
        .block(BorderBlock::bordered().title("input"));
    frame.render_widget(widget, area);

    // Position the terminal cursor inside the bordered area.
    let cx = area.x + 1 + cursor_col as u16;
    let cy = area.y + 1 + (cursor_row.saturating_sub(top)) as u16;
    frame.set_cursor_position((cx, cy));
}
