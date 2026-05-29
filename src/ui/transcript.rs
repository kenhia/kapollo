//! Transcript pad rendering: each block as its command line followed by its
//! captured output, with a truncation marker when output was dropped (FR-004,
//! FR-016). Supports independent scrolling: `scroll_offset` counts lines up
//! from the newest output (FR-014).

use ratatui::layout::Rect;
use ratatui::widgets::{Block as BorderBlock, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::session::BlockState;

/// Render the transcript into `area`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let mut text = String::new();
    for block in app.transcript.blocks() {
        text.push_str("$ ");
        text.push_str(&block.command);
        text.push('\n');

        if block.truncated() {
            text.push_str("… output truncated …\n");
        }
        let output = block.output_lossy();
        if !output.is_empty() {
            text.push_str(&output);
            if !output.ends_with('\n') {
                text.push('\n');
            }
        }
        if block.state == BlockState::Interactive {
            text.push_str("[interactive program]\n");
        }
    }

    // Pin the view to the newest output by default; `scroll_offset` scrolls up.
    let total_lines = text.lines().count();
    let viewport = area.height.saturating_sub(2) as usize; // borders
    let max_top = total_lines.saturating_sub(viewport);
    let top = max_top.saturating_sub(app.transcript.scroll_offset()) as u16;

    let widget = Paragraph::new(text)
        .scroll((top, 0))
        .block(BorderBlock::bordered().title("transcript"));
    frame.render_widget(widget, area);
}
