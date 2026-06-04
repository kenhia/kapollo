//! Selection coordinate math: screen<->content mapping, auto-scroll, and range
//! normalization. All functions are pure and unit-tested (R5, FR-008/FR-009/FR-010).

/// A selection endpoint in **content** coordinates: `(absolute_row, col)` where
/// `absolute_row` indexes the full scrollback history (0 = oldest retained line).
pub type Cell = (usize, usize);

/// Map a visible screen row to its absolute content row.
/// `top_row` is the first visible content row; `screen_y` is the row within the
/// viewport. Content coordinates are invariant under scrolling (FR-008).
pub fn screen_to_content(top_row: usize, screen_y: usize) -> usize {
    top_row + screen_y
}

/// Inverse of [`screen_to_content`], clamped to the visible output region.
/// Returns `None` when `abs_row` is outside `[top_row, top_row + height)`.
pub fn content_to_screen(top_row: usize, abs_row: usize, height: usize) -> Option<usize> {
    if abs_row < top_row {
        return None;
    }
    let y = abs_row - top_row;
    if y >= height {
        None
    } else {
        Some(y)
    }
}

/// Compute the next `top_row` when dragging past a viewport edge.
/// Scroll up by one when `drag_y < 0`, down by one when `drag_y >= height`, and
/// otherwise leave `top_row` unchanged. The result is clamped to
/// `[0, history_len - height]` (FR-010, R5).
pub fn auto_scroll(top_row: usize, drag_y: isize, height: usize, history_len: usize) -> usize {
    let max_top = history_len.saturating_sub(height);
    let next = if drag_y < 0 {
        top_row.saturating_sub(1)
    } else if drag_y as usize >= height {
        top_row + 1
    } else {
        top_row
    };
    next.min(max_top)
}

/// Normalize a selection to document order so copy always reads top-left -> bottom-right
/// (FR-016). Comparison is lexicographic on `(row, col)`.
pub fn normalize(anchor: Cell, end: Cell) -> (Cell, Cell) {
    if end < anchor {
        (end, anchor)
    } else {
        (anchor, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_to_content_adds_top_row() {
        assert_eq!(screen_to_content(0, 0), 0);
        assert_eq!(screen_to_content(10, 3), 13);
    }

    #[test]
    fn content_to_screen_is_inverse_when_visible() {
        let top = 10;
        let height = 5;
        for screen_y in 0..height {
            let abs = screen_to_content(top, screen_y);
            assert_eq!(content_to_screen(top, abs, height), Some(screen_y));
        }
    }

    #[test]
    fn content_to_screen_clamps_out_of_region() {
        assert_eq!(content_to_screen(10, 9, 5), None); // above viewport
        assert_eq!(content_to_screen(10, 15, 5), None); // below viewport (10+5)
        assert_eq!(content_to_screen(10, 14, 5), Some(4)); // last visible row
    }

    #[test]
    fn auto_scroll_up_past_top() {
        assert_eq!(auto_scroll(5, -1, 10, 100), 4);
    }

    #[test]
    fn auto_scroll_down_past_bottom() {
        assert_eq!(auto_scroll(5, 10, 10, 100), 6); // drag_y >= height
    }

    #[test]
    fn auto_scroll_within_bounds_is_noop() {
        assert_eq!(auto_scroll(5, 3, 10, 100), 5);
    }

    #[test]
    fn auto_scroll_clamps_at_zero() {
        assert_eq!(auto_scroll(0, -1, 10, 100), 0);
    }

    #[test]
    fn auto_scroll_clamps_at_max_top() {
        // history_len - height = 90 is the last valid top_row
        assert_eq!(auto_scroll(90, 10, 10, 100), 90);
    }

    #[test]
    fn auto_scroll_handles_history_shorter_than_height() {
        assert_eq!(auto_scroll(0, 10, 10, 5), 0);
    }

    #[test]
    fn normalize_orders_by_row_then_col() {
        assert_eq!(normalize((5, 2), (3, 9)), ((3, 9), (5, 2)));
        assert_eq!(normalize((3, 9), (5, 2)), ((3, 9), (5, 2)));
        assert_eq!(normalize((4, 8), (4, 2)), ((4, 2), (4, 8))); // same row
    }

    #[test]
    fn normalize_identity_when_already_ordered() {
        assert_eq!(normalize((1, 1), (1, 1)), ((1, 1), (1, 1)));
    }
}
