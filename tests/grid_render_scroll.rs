//! Scrollback-window + alt-screen contract tests (US1, T012): viewport windowing
//! with top clamp, alt-screen enter/leave restoring the prior viewport, and
//! single-row damage reporting, per
//! `specs/004-grid-rework/contracts/grid-render.md` (FR-004/005, SC-003).

use kapollo::grid::render::rows_to_lines;
use kapollo::grid::Grid;
use ratatui::text::Line as TuiLine;

fn line_text(line: &TuiLine<'_>) -> String {
    line.spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<String>()
        .trim_end()
        .to_string()
}

fn viewport_text(grid: &Grid, scroll_offset: usize) -> Vec<String> {
    rows_to_lines(&grid.viewport_lines(scroll_offset), grid.size().1)
        .iter()
        .map(line_text)
        .collect()
}

#[test]
fn scrolling_back_shows_history_and_clamps_at_the_top() {
    let mut grid = Grid::new(5, 20);
    for i in 0..20 {
        grid.advance_bytes(format!("line{i}\r\n").as_bytes());
    }
    // At the live bottom the newest lines are visible.
    let bottom = viewport_text(&grid, 0);
    assert!(bottom.iter().any(|l| l == "line19"));

    // Scrolling far past the top clamps to the oldest retained row, never panics.
    let top = viewport_text(&grid, usize::MAX);
    assert!(top.iter().any(|l| l == "line0"));
}

#[test]
fn entering_then_leaving_alt_screen_restores_prior_viewport() {
    let mut grid = Grid::new(5, 20);
    grid.advance_bytes(b"main-screen\r\n");
    let before = viewport_text(&grid, 0);

    // Enter alt screen, paint something, then leave.
    grid.advance_bytes(b"\x1b[?1049h");
    assert!(grid.is_alt_screen_active());
    grid.advance_bytes(b"ALT CONTENT");
    grid.advance_bytes(b"\x1b[?1049l");
    assert!(!grid.is_alt_screen_active());

    let after = viewport_text(&grid, 0);
    assert_eq!(
        before, after,
        "main viewport must survive an alt-screen round trip"
    );
    // Alt-screen content must not leak into the restored main scrollback.
    assert!(!after.iter().any(|l| l.contains("ALT CONTENT")));
}

#[test]
fn changed_rows_reports_a_one_row_range_after_a_single_update() {
    let mut grid = Grid::new(24, 80);
    grid.advance_bytes(b"first line of output");
    let changed = grid.changed_rows();
    assert_eq!(changed.end - changed.start, 1);
}
