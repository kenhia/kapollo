//! Input-pad selection tests (sprint 005, US1/US5; FR-003/004/027/029).
//! Validates that Shift-motion creates/extends a selection, word selection
//! respects line-aware word boundaries, plain motion / cancellation collapse it,
//! and the US5 arbiter + `Esc`/`Esc Esc` gestures follow the spec precedence.

use kapollo::input::selection::{esc_action, ActiveSelection, EscAction, InputSelection};
use kapollo::input::InputPad;

fn pad(text: &str, cursor: usize) -> InputPad {
    let mut p = InputPad::new();
    p.set_contents(text);
    let len = text.chars().count();
    for _ in cursor..len {
        p.move_left();
    }
    p
}

#[test]
fn select_char_extends_one_at_a_time() {
    let mut p = pad("hello world", 11);
    p.select_char_left();
    p.select_char_left();
    assert!(p.has_selection());
    let range = p.selection().expect("selection present").range();
    assert_eq!(range, (9, 11));
}

#[test]
fn select_word_uses_line_word_boundaries() {
    let mut p = pad("hello world", 11);
    p.select_word_left();
    let range = p.selection().expect("selection present").range();
    assert_eq!(range, (6, 11));
    assert_eq!(p.selected_text().as_deref(), Some("world"));
}

#[test]
fn select_char_right_extends_forward() {
    let mut p = pad("hello world", 0);
    p.select_char_right();
    p.select_char_right();
    let range = p.selection().expect("selection present").range();
    assert_eq!(range, (0, 2));
}

#[test]
fn plain_motion_collapses_selection() {
    let mut p = pad("hello world", 11);
    p.select_char_left();
    assert!(p.has_selection());
    p.move_left();
    assert!(!p.has_selection());
}

#[test]
fn cancel_selection_clears_it() {
    let mut p = pad("hello world", 11);
    p.select_word_left();
    assert!(p.has_selection());
    p.cancel_selection();
    assert!(!p.has_selection());
}

#[test]
fn editing_collapses_selection() {
    let mut p = pad("hello", 5);
    p.select_char_left();
    p.insert_char('!');
    assert!(!p.has_selection());
}

// --- US5: single-selection arbiter + Esc / Esc Esc gestures ---------------

#[test]
fn active_selection_holds_at_most_one_pad() {
    // The sum type makes "at most one pad selected" structural (FR-027): an
    // input selection exposes no transcript range and vice versa.
    let input = ActiveSelection::Input(InputSelection::new(3));
    assert!(input.is_active());
    assert_eq!(input.input(), Some(InputSelection::new(3)));
    assert_eq!(input.transcript(), None);

    let transcript = ActiveSelection::Transcript((2, 0), (2, 5));
    assert_eq!(transcript.transcript(), Some(((2, 0), (2, 5))));
    assert_eq!(transcript.input(), None);

    assert!(ActiveSelection::None.is_none());
}

#[test]
fn esc_first_cancels_selection_then_clears_line() {
    // First Esc with a selection cancels; first Esc with none clears the line
    // (FR-029).
    assert_eq!(esc_action(false, true, false), EscAction::CancelSelection);
    assert_eq!(esc_action(false, false, false), EscAction::ClearCurrentLine);
    assert_eq!(esc_action(false, false, true), EscAction::ClearCurrentLine);
}

#[test]
fn esc_esc_clears_whole_multiline_buffer_only() {
    // Second consecutive Esc clears the whole buffer when multi-line, and does
    // nothing further on a single-line buffer (the message clears regardless).
    assert_eq!(esc_action(true, false, true), EscAction::ClearWholeBuffer);
    assert_eq!(esc_action(true, false, false), EscAction::None);
}

#[test]
fn clear_current_line_empties_a_single_line_buffer() {
    let mut p = pad("hello world", 11);
    p.clear_current_line();
    assert_eq!(p.as_str(), "");
    assert!(p.is_empty());
}

#[test]
fn clear_current_line_keeps_other_lines() {
    // Cursor on the middle line: only that line's text is removed; the line
    // structure (and the other lines) survive (FR-029, single Esc multi-line).
    let mut p = InputPad::new();
    p.set_contents("one\ntwo\nthree");
    // Move the caret onto the middle line ("two", chars 4..7); cursor starts at
    // end (13). Walk it back to just inside "two".
    for _ in 0..7 {
        p.move_left();
    }
    p.clear_current_line();
    assert_eq!(p.as_str(), "one\n\nthree");
}
