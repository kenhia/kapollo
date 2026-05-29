//! Multiline + history tests (T037): Shift/Alt+Enter insert a newline without
//! submitting; Enter submits the whole multiline buffer; Up/Down recall
//! kapollo's own input history (FR-010, FR-011, FR-013, SC-007).

use kapollo::input::{InputHistory, InputPad};

#[test]
fn newline_insertion_builds_a_multiline_buffer() {
    let mut pad = InputPad::new();
    for c in "echo one".chars() {
        pad.insert_char(c);
    }
    pad.insert_newline(); // Shift+Enter / Alt+Enter
    for c in "echo two".chars() {
        pad.insert_char(c);
    }
    assert_eq!(pad.line_count(), 2);
    assert_eq!(pad.as_str(), "echo one\necho two");
}

#[test]
fn enter_submits_the_whole_multiline_buffer_as_one_unit() {
    let mut pad = InputPad::new();
    pad.set_contents("line1\nline2\nline3");
    let submitted = pad.take_submit();
    assert_eq!(submitted, "line1\nline2\nline3");
    assert!(pad.is_empty(), "submitting clears the pad");
}

#[test]
fn up_and_down_recall_prior_inputs() {
    let mut history = InputHistory::new();
    history.push("git status");
    history.push("cargo test");

    // Up walks toward older entries.
    assert_eq!(history.recall_older(), Some("cargo test"));
    assert_eq!(history.recall_older(), Some("git status"));
    // Down walks back toward the newest, then to an empty draft.
    assert_eq!(history.recall_newer(), Some("cargo test"));
    assert_eq!(history.recall_newer(), Some(""));
}

#[test]
fn history_is_independent_of_buffer_editing() {
    let mut history = InputHistory::new();
    let mut pad = InputPad::new();
    history.push("previous command");

    pad.set_contents("draft in progress");
    // Recalling replaces the pad contents with the history entry.
    if let Some(text) = history.recall_older() {
        let text = text.to_string();
        pad.set_contents(text);
    }
    assert_eq!(pad.as_str(), "previous command");
}
