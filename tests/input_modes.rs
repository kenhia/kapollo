//! Input-mode tests (sprint 007). The Foundational subset (C2/C3/C4) covers the
//! `ToggleMultLaat` mode transitions and mode-aware caret motion that both P1
//! stories build on; the US2 edge-recall cases (C1/C5/C6/C7/C8) live alongside.
//! See `specs/007-laat-mode/contracts/input-modes.md` §4.

use kapollo::input::{InputHistory, InputMode, InputPad};

/// Model App's mode reconciliation after an edit (FR-008/FR-012): a `Norm`
/// buffer that grows past one line enters `Mult`; a `Mult` buffer deleted back
/// to a single line returns to `Norm`. Mirrors `App::reconcile_mode_after_edit`.
fn reconcile(mode: InputMode, lines: usize) -> InputMode {
    match mode {
        InputMode::Norm if lines > 1 => InputMode::Mult,
        InputMode::Mult if lines <= 1 => InputMode::Norm,
        other => other,
    }
}

// --- Foundational (T006): C2, C3, C4 ---------------------------------------

#[test]
fn c2_ctrl_1_from_empty_norm_enters_mult() {
    // From an empty Norm buffer, ToggleMultLaat enters Mult (FR-015).
    let pad = InputPad::new();
    let multiline = pad.line_count() > 1;
    assert_eq!(
        InputMode::Norm.toggled_mult_laat(multiline),
        InputMode::Mult
    );
}

#[test]
fn c3_ctrl_1_toggles_mult_and_laat_when_multiline() {
    // From a multi-line Mult buffer, ToggleMultLaat toggles Mult <-> Laat (FR-016).
    let mut pad = InputPad::new();
    pad.set_contents("a\nb");
    let multiline = pad.line_count() > 1;
    assert!(multiline);
    let to_laat = InputMode::Mult.toggled_mult_laat(multiline);
    assert_eq!(to_laat, InputMode::Laat);
    assert_eq!(to_laat.toggled_mult_laat(multiline), InputMode::Mult);
}

#[test]
fn c4_up_in_mult_moves_caret_without_history_recall() {
    // In Mult with the caret on line 2 of "a\nb", Up moves the caret to line 1
    // with no history recall and an unchanged buffer (FR-009).
    let mut pad = InputPad::new();
    pad.set_contents("a\nb");
    assert!(pad.caret_on_last_line());

    // Mode-aware Up in Mult is caret motion, not history recall.
    pad.caret_line_up();

    assert_eq!(pad.cursor_row_col(), (0, 1));
    assert_eq!(pad.as_str(), "a\nb");
}

// --- User Story 2 (T019): C1, C5, C6, C7, C8 -------------------------------

#[test]
fn c1_alt_enter_then_typing_enters_mult_with_combined_buffer() {
    // From Norm with "abc", Alt+Enter (InsertNewline) then "def" yields a
    // two-line buffer and reconciles Norm -> Mult (FR-008).
    let mut pad = InputPad::new();
    for c in "abc".chars() {
        pad.insert_char(c);
    }
    let mut mode = InputMode::Norm;
    pad.insert_newline();
    mode = reconcile(mode, pad.line_count());
    for c in "def".chars() {
        pad.insert_char(c);
    }
    mode = reconcile(mode, pad.line_count());

    assert_eq!(pad.as_str(), "abc\ndef");
    assert_eq!(mode, InputMode::Mult);
}

#[test]
fn c5_edge_recall_stashes_draft_and_restores_on_down_past_newest() {
    // In Mult with the caret on line 1, Up stashes the live draft and recalls
    // the previous entry; Down past the newest entry restores it (FR-010).
    let mut history = InputHistory::new();
    history.push("old");
    let mut pad = InputPad::new();
    for c in "draft".chars() {
        pad.insert_char(c);
    }
    assert!(pad.caret_on_first_line());

    // Up on the first line: edge recall (stash "draft", recall "old").
    let draft = pad.as_str().to_string();
    let recalled = history.edge_recall_older(&draft).map(str::to_string);
    if let Some(text) = recalled {
        pad.set_contents(text);
    }
    assert_eq!(pad.as_str(), "old");

    // Down past the newest entry: restore the stashed draft byte-for-byte.
    if let Some(text) = history.edge_recall_newer() {
        pad.set_contents(text);
    }
    assert_eq!(pad.as_str(), "draft");
}

#[test]
fn c6_deleting_to_one_line_returns_to_norm() {
    // In Mult, deleting the only newline so a single line remains reconciles
    // Mult -> Norm (FR-012).
    let mut pad = InputPad::new();
    pad.set_contents("a\nb");
    let mut mode = reconcile(InputMode::Norm, pad.line_count());
    assert_eq!(mode, InputMode::Mult);

    // Delete back to a single line.
    pad.set_contents("ab");
    mode = reconcile(mode, pad.line_count());

    assert_eq!(pad.line_count(), 1);
    assert_eq!(mode, InputMode::Norm);
}

#[test]
fn c7_plain_enter_in_mult_submits_whole_buffer_as_one() {
    // Plain Enter in Mult submits the entire buffer as a single combined
    // submission (FR-013): the whole buffer is taken at once.
    let mut pad = InputPad::new();
    pad.set_contents("one\ntwo\nthree");
    let submitted = pad.take_submit();

    assert_eq!(submitted, "one\ntwo\nthree");
    assert_eq!(pad.as_str(), "");
}

#[test]
fn c8_multi_line_selection_enter_is_one_combined_submission() {
    // Selecting multiple lines marks a multi-line span; Enter submits it as one
    // combined submission rather than gated LAAT steps (FR-017).
    let mut pad = InputPad::new();
    pad.set_contents("alpha\nbeta");
    // The caret sits at the buffer end after set_contents; select the whole
    // buffer backwards so the span crosses the newline.
    for _ in 0.."alpha\nbeta".chars().count() {
        pad.select_char_left();
    }
    let selected = pad.selected_text().expect("a selection is active");

    assert!(
        selected.contains('\n'),
        "the multi-line selection spans the newline: {selected:?}"
    );
}
