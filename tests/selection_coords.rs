//! Selection coordinate + FSM integration (T020, US2): a click-drag-release
//! produces an active, content-anchored range; bare/second clicks and command
//! submission clear it; the anchor never drifts as output scrolls; and
//! `extract_text` slices the viewport char-for-char with no off-by-one
//! (FR-007/008/011, SC-004).

use kapollo::selection::coords::{self, Cell};
use kapollo::selection::{extract_text, LeftPress, SelState, SelectionController, Trigger};

/// A 3x5 viewport cell grid (row-major, single-cell strings) for slice tests.
fn viewport() -> Vec<Vec<String>> {
    ["abcde", "fghij", "klmno"]
        .iter()
        .map(|row| row.chars().map(|c| c.to_string()).collect())
        .collect()
}

#[test]
fn click_drag_release_yields_active_range() {
    let mut sel = SelectionController::new();
    assert_eq!(sel.state(), SelState::Idle);

    assert_eq!(sel.left_press((10, 2), false), LeftPress::StartedDrag);
    assert_eq!(sel.state(), SelState::Dragging);

    sel.drag_to((12, 4));
    sel.release();

    assert_eq!(sel.state(), SelState::Active);
    assert!(sel.is_active());
    assert_eq!(sel.range(), Some(((10, 2), (12, 4))));
}

#[test]
fn drag_normalizes_backwards_selection() {
    let mut sel = SelectionController::new();
    sel.left_press((12, 4), false);
    sel.drag_to((10, 2));
    sel.release();

    // Normalized to document order (top-left -> bottom-right).
    assert_eq!(sel.range(), Some(((10, 2), (12, 4))));
}

#[test]
fn second_click_clears_an_active_selection() {
    let mut sel = SelectionController::new();
    sel.left_press((1, 0), false);
    sel.drag_to((2, 3));
    sel.release();
    assert!(sel.is_active());

    // A press on an active selection clears it and copies nothing.
    assert_eq!(sel.left_press((5, 5), false), LeftPress::Cancelled);
    assert_eq!(sel.state(), SelState::Idle);
    assert_eq!(sel.range(), None);
}

#[test]
fn command_submit_clears_selection() {
    let mut sel = SelectionController::new();
    sel.left_press((0, 0), false);
    sel.drag_to((1, 1));
    sel.release();
    assert!(sel.is_active());

    sel.on_command_submit();
    assert_eq!(sel.state(), SelState::Idle);
    assert_eq!(sel.range(), None);
}

#[test]
fn shift_press_forwards_to_child() {
    let mut sel = SelectionController::new();
    assert_eq!(sel.left_press((0, 0), true), LeftPress::ForwardToChild);
    assert_eq!(sel.state(), SelState::Idle);
}

#[test]
fn ctrl_c_copies_when_active_else_sigint() {
    let mut sel = SelectionController::new();
    // No selection: Ctrl-C is a SIGINT.
    assert_eq!(sel.ctrl_c(), Trigger::Sigint);

    sel.left_press((3, 1), false);
    sel.drag_to((3, 4));
    sel.release();
    // Active selection: Ctrl-C copies the normalized range and deselects.
    assert_eq!(sel.ctrl_c(), Trigger::Copy((3, 1), (3, 4)));
    assert_eq!(sel.state(), SelState::Idle);
}

#[test]
fn content_anchor_does_not_drift_when_scrolled() {
    // The user selects content row 12, col 2. Mapping that same content cell
    // back to a screen row under two different scroll offsets must place it on
    // different screen rows while the content cell itself is unchanged (R6).
    let cell: Cell = (12, 2);

    // top_row = 10 (scrolled up): content row 12 is screen row 2.
    assert_eq!(coords::content_to_screen(10, cell.0, 5), Some(2));
    // top_row = 12 (scrolled to bottom): content row 12 is screen row 0.
    assert_eq!(coords::content_to_screen(12, cell.0, 5), Some(0));
    // Out of view above: no screen row.
    assert_eq!(coords::content_to_screen(20, cell.0, 5), None);
}

#[test]
fn extract_text_single_row_is_char_for_char() {
    let rows = viewport();
    // Row 0 (top_row = 0), columns 1..=3 inclusive -> "bcd".
    let text = extract_text(&rows, 0, (0, 1), (0, 3));
    assert_eq!(text, "bcd");
}

#[test]
fn extract_text_multi_row_no_off_by_one() {
    let rows = viewport();
    // From (0,2) to (2,1): tail of row 0, all of row 1, head of row 2.
    let text = extract_text(&rows, 0, (0, 2), (2, 1));
    assert_eq!(text, "cde\nfghij\nkl");
}

#[test]
fn extract_text_right_trims_each_line() {
    let rows = vec![
        "ab   ".chars().map(|c| c.to_string()).collect::<Vec<_>>(),
        "cd   ".chars().map(|c| c.to_string()).collect::<Vec<_>>(),
    ];
    let text = extract_text(&rows, 0, (0, 0), (1, 4));
    assert_eq!(text, "ab\ncd");
}
