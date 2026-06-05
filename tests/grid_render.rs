//! Grid-render contract tests (US1, T011): in-place CR, SGR→ratatui style, and
//! wide-cell handling, per `specs/004-grid-rework/contracts/grid-render.md`
//! (FR-002/003, SC-002).

use kapollo::grid::render::{row_to_line, rows_to_lines};
use kapollo::grid::Grid;
use ratatui::style::{Color, Modifier};
use ratatui::text::Line as TuiLine;

fn line_text(line: &TuiLine<'_>) -> String {
    line.spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<String>()
        .trim_end()
        .to_string()
}

#[test]
fn carriage_return_yields_one_row_in_place() {
    // "a\rb" must leave a single row reading "b…", not two rows (FR-003, SC-001).
    let mut grid = Grid::new(24, 80);
    grid.advance_bytes(b"a\rb");
    let lines = rows_to_lines(&grid.viewport_lines(0), 80);
    assert_eq!(line_text(&lines[0]), "b");
    // The second row stays blank — the CR did not advance to a new line.
    assert_eq!(line_text(&lines[1]), "");
}

#[test]
fn sgr_sequence_maps_to_ratatui_modifier_and_color() {
    let mut grid = Grid::new(24, 80);
    // Underline (4) + blue foreground (34).
    grid.advance_bytes(b"\x1b[4;34mZ\x1b[0m");
    let line = row_to_line(&grid.viewport_lines(0)[0], 80);
    let span = &line.spans[0];
    assert_eq!(span.content.as_ref(), "Z");
    assert!(span.style.add_modifier.contains(Modifier::UNDERLINED));
    assert_eq!(span.style.fg, Some(Color::Indexed(4)));
}

#[test]
fn wide_char_advances_two_columns_with_empty_continuation() {
    let mut grid = Grid::new(24, 80);
    grid.advance_bytes("世x".as_bytes());
    let line = row_to_line(&grid.viewport_lines(0)[0], 80);
    // "世" (2 cols) + empty continuation + "x" at column 2 → visible "世x".
    assert_eq!(line_text(&line), "世x");
}
