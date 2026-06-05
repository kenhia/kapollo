//! Grid → ratatui render mapping: turns the engine's styled cells into ratatui
//! spans, mapping cell fg/bg/attributes to a ratatui `Style` and handling wide
//! (CJK/emoji) cells (FR-002/003/004, sprint 004).
//!
//! Colors follow the spike's policy: the terminal **default** fg/bg are left
//! unset so the host terminal's theme shows through, and palette indices are
//! forwarded as ANSI indices (host-themed) rather than resolved to fixed RGB.
//! Only genuine truecolor becomes a fixed `Color::Rgb` (D30).

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line as TuiLine, Span};

use wezterm_term::color::{ColorAttribute, SrgbaTuple};
use wezterm_term::{CellAttributes, Intensity, Line, Underline};

/// Map the visible viewport `lines` to ratatui `Line`s, padded/truncated to
/// `cols`. Wide cells occupy two columns; their continuation column renders as a
/// space so following columns stay aligned (FR-002).
pub fn rows_to_lines(lines: &[Line], cols: u16) -> Vec<TuiLine<'static>> {
    lines.iter().map(|line| row_to_line(line, cols)).collect()
}

/// Like [`rows_to_lines`] but overlays a selection highlight: `highlight[y]` is
/// an inclusive `(start_col, end_col)` range to render reversed on row `y`, or
/// `None` for no highlight on that row (FR-007).
pub fn rows_to_lines_selected(
    lines: &[Line],
    cols: u16,
    highlight: &[Option<(usize, usize)>],
) -> Vec<TuiLine<'static>> {
    lines
        .iter()
        .enumerate()
        .map(|(y, line)| {
            let hl = highlight.get(y).copied().flatten();
            row_to_line_hl(line, cols, hl)
        })
        .collect()
}

/// Map a single engine line to a ratatui `Line` of exactly `cols` columns.
pub fn row_to_line(line: &Line, cols: u16) -> TuiLine<'static> {
    row_to_line_hl(line, cols, None)
}

/// Map a single engine line to a ratatui `Line`, optionally reversing the cells
/// in the inclusive `highlight` column range to draw the selection (FR-007).
fn row_to_line_hl(line: &Line, cols: u16, highlight: Option<(usize, usize)>) -> TuiLine<'static> {
    let width = cols as usize;
    // Build a flat column buffer so gaps (unset cells, wide-cell continuations)
    // are filled with spaces and every column is accounted for.
    let mut cells: Vec<(String, Style)> = vec![(" ".to_string(), Style::default()); width];
    for cell in line.visible_cells() {
        let x = cell.cell_index();
        if x >= width {
            continue;
        }
        let s = cell.str();
        let sym = if s.is_empty() {
            " ".to_string()
        } else {
            s.to_string()
        };
        let style = cell_style(cell.attrs());
        cells[x] = (sym, style);
        // A wide (2-column) cell visually fills its continuation columns; emit
        // them as empty strings so following columns stay aligned (FR-002).
        for k in 1..cell.width() {
            if x + k < width {
                cells[x + k] = (String::new(), style);
            }
        }
    }

    // Overlay the selection highlight by reversing the covered columns (FR-007).
    if let Some((c0, c1)) = highlight {
        let end = c1.min(width.saturating_sub(1));
        for col in cells.iter_mut().take(end + 1).skip(c0) {
            col.1 = col.1.add_modifier(Modifier::REVERSED);
        }
    }

    // Coalesce runs of equal style into spans to keep the buffer small.
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_style = Style::default();
    for (sym, style) in cells {
        if run.is_empty() {
            run.push_str(&sym);
            run_style = style;
        } else if style == run_style {
            run.push_str(&sym);
        } else {
            spans.push(Span::styled(std::mem::take(&mut run), run_style));
            run.push_str(&sym);
            run_style = style;
        }
    }
    if !run.is_empty() {
        spans.push(Span::styled(run, run_style));
    }
    TuiLine::from(spans)
}

/// Map a `wezterm-term` cell's colors/attributes onto a ratatui `Style`.
pub fn cell_style(attrs: &CellAttributes) -> Style {
    let mut s = Style::default();
    if let Some(fg) = conv_color(attrs.foreground()) {
        s = s.fg(fg);
    }
    if let Some(bg) = conv_color(attrs.background()) {
        s = s.bg(bg);
    }
    match attrs.intensity() {
        Intensity::Bold => s = s.add_modifier(Modifier::BOLD),
        Intensity::Half => s = s.add_modifier(Modifier::DIM),
        Intensity::Normal => {}
    }
    if attrs.italic() {
        s = s.add_modifier(Modifier::ITALIC);
    }
    if attrs.underline() != Underline::None {
        s = s.add_modifier(Modifier::UNDERLINED);
    }
    if attrs.reverse() {
        s = s.add_modifier(Modifier::REVERSED);
    }
    s
}

/// Convert a wezterm `ColorAttribute` to a ratatui color, returning `None` for
/// the terminal **default** so the host theme/background shows through. Palette
/// indices map to `Color::Indexed` (host-themed); only genuine truecolor becomes
/// a fixed `Color::Rgb`.
fn conv_color(c: ColorAttribute) -> Option<Color> {
    match c {
        ColorAttribute::Default => None,
        ColorAttribute::PaletteIndex(i) => Some(Color::Indexed(i)),
        ColorAttribute::TrueColorWithPaletteFallback(rgb, _) => Some(srgba_to_color(rgb)),
        ColorAttribute::TrueColorWithDefaultFallback(rgb) => Some(srgba_to_color(rgb)),
    }
}

fn srgba_to_color(c: SrgbaTuple) -> Color {
    let (r, g, b, _) = c.to_srgb_u8();
    Color::Rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

    /// Concatenate the symbols of a rendered line back into a string.
    fn line_text(line: &TuiLine<'_>) -> String {
        line.spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn maps_plain_text_to_a_line() {
        let mut grid = Grid::new(24, 80);
        grid.advance_bytes(b"hello");
        let lines = rows_to_lines(&grid.viewport_lines(0), 80);
        assert_eq!(line_text(&lines[0]), "hello");
    }

    #[test]
    fn sgr_bold_red_maps_to_modifier_and_color() {
        let mut grid = Grid::new(24, 80);
        // Bold (1) + red foreground (31).
        grid.advance_bytes(b"\x1b[1;31mX\x1b[0m");
        let line = row_to_line(&grid.viewport_lines(0)[0], 80);
        let first = &line.spans[0];
        assert_eq!(first.content.as_ref(), "X");
        assert!(first.style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(first.style.fg, Some(Color::Indexed(1)));
    }

    #[test]
    fn wide_char_keeps_following_columns_aligned() {
        let mut grid = Grid::new(24, 80);
        // A wide CJK char then ASCII: the wide char takes 2 columns, the "B"
        // must land in column 2 so total visible text is "世B".
        grid.advance_bytes("世B".as_bytes());
        let line = row_to_line(&grid.viewport_lines(0)[0], 80);
        assert_eq!(line_text(&line), "世B");
    }

    #[test]
    fn default_colors_leave_style_unset() {
        let mut grid = Grid::new(24, 80);
        grid.advance_bytes(b"plain");
        let line = row_to_line(&grid.viewport_lines(0)[0], 80);
        assert_eq!(line.spans[0].style.fg, None);
        assert_eq!(line.spans[0].style.bg, None);
    }

    #[test]
    fn line_is_padded_to_requested_width() {
        let mut grid = Grid::new(24, 80);
        grid.advance_bytes(b"hi");
        let line = row_to_line(&grid.viewport_lines(0)[0], 10);
        let width: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(width, 10);
    }
}
