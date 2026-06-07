//! LAAT engine tests (sprint 007): highlight stepping and exit-code gating
//! (L1–L6). The gating decision is a pure `LaatState` transition; the live
//! highlight/flag rendering and the `Esc Esc` abort use the Constitution III
//! manual exception (quickstart). See
//! `specs/007-laat-mode/contracts/laat-engine.md` §6.

use kapollo::input::{InputMode, LaatOutcome, LaatState};

#[test]
fn l1_enter_laat_highlights_line_zero() {
    // Entering LAAT starts the highlight on line 0 with the `1T` status label.
    let laat = LaatState::new();
    assert_eq!(laat.highlight, 0);
    assert!(laat.failed_lines.is_empty());
    assert_eq!(laat.pending, None);
    assert_eq!(InputMode::Laat.label(), "1T");
}

#[test]
fn l2_submit_line_zero_success_advances() {
    // Submit line 0; CommandEnd { Some(0) } advances to line 1, no flags.
    let mut laat = LaatState::new();
    laat.submit_line(0);
    assert_eq!(laat.apply_exit_code(Some(0)), Some(LaatOutcome::Advance));
    assert_eq!(laat.highlight, 1);
    assert!(laat.failed_lines.is_empty());
}

#[test]
fn l3_submit_failure_flags_and_holds() {
    // Submit line 1; CommandEnd { Some(7) } flags line 1, highlight stays.
    let mut laat = LaatState::new();
    laat.highlight = 1;
    laat.submit_line(1);
    assert_eq!(laat.apply_exit_code(Some(7)), Some(LaatOutcome::Flag));
    assert_eq!(laat.highlight, 1);
    assert!(laat.is_failed(1));
}

#[test]
fn l4_advance_past_failure_retains_earlier_flag() {
    // After a line-1 failure, Down to line 2 + Enter + success advances past the
    // end (highlight 3); the line-1 flag is retained (only the completed line's
    // flag is touched).
    let mut laat = LaatState::new();
    laat.highlight = 1;
    laat.submit_line(1);
    laat.apply_exit_code(Some(7));
    assert!(laat.is_failed(1));

    // Down moves the highlight to line 2 (caller tracks the caret line).
    laat.highlight = 2;
    laat.submit_line(2);
    assert_eq!(laat.apply_exit_code(Some(0)), Some(LaatOutcome::Advance));
    assert_eq!(laat.highlight, 3);
    assert!(laat.is_failed(1), "the earlier line-1 flag is retained");
}

#[test]
fn l5_rerun_clears_the_flag_on_success() {
    // Re-submitting the flagged line and succeeding clears its flag and advances.
    let mut laat = LaatState::new();
    laat.highlight = 1;
    laat.submit_line(1);
    laat.apply_exit_code(Some(7));
    assert!(laat.is_failed(1));

    // Re-run line 1 (Enter on the same highlight).
    laat.submit_line(1);
    assert_eq!(laat.apply_exit_code(Some(0)), Some(LaatOutcome::Advance));
    assert_eq!(laat.highlight, 2);
    assert!(
        !laat.is_failed(1),
        "the flag is cleared on the successful re-run"
    );
}

#[test]
fn l6_missing_exit_code_is_treated_as_success() {
    // A completion with no reported exit code advances rather than flagging
    // (no failure was observed).
    let mut laat = LaatState::new();
    laat.submit_line(0);
    assert_eq!(laat.apply_exit_code(None), Some(LaatOutcome::Advance));
    assert_eq!(laat.highlight, 1);
}
