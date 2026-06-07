//! Push/pop input-stack tests (sprint 007, contracts/push-pop-stack.md §5). The
//! one-item snapshot save/restore is pure and unit-testable here; the
//! `Ctrl+Alt+Enter` keymap binding and the live round-trip use the Constitution
//! III manual exception (quickstart). The local `push`/`pop` helpers mirror
//! `App::push_input`/`App::pop_input` so the snapshot semantics (including the
//! one-item no-op guard) are exercised directly.

use kapollo::input::{InputMode, InputPad, InputSnapshot, LaatState};

/// Mirror of `App::push_input`: capture and reset for an ad-hoc command, a no-op
/// when the slot is already occupied (FR-020).
fn push(
    slot: &mut Option<InputSnapshot>,
    pad: &mut InputPad,
    mode: &mut InputMode,
    stash: &mut Option<String>,
    laat: &mut Option<LaatState>,
) {
    if slot.is_some() {
        return;
    }
    *slot = Some(InputSnapshot::capture(
        pad,
        *mode,
        stash.clone(),
        laat.clone(),
    ));
    pad.clear();
    *stash = None;
    *mode = InputMode::Norm;
    *laat = None;
}

/// Mirror of `App::pop_input`: restore a pushed snapshot, clearing the slot.
fn pop(
    slot: &mut Option<InputSnapshot>,
    pad: &mut InputPad,
    mode: &mut InputMode,
    stash: &mut Option<String>,
    laat: &mut Option<LaatState>,
) {
    if let Some(snapshot) = slot.take() {
        let (m, s, l) = snapshot.restore(pad);
        *mode = m;
        *stash = s;
        *laat = l;
    }
}

#[test]
fn p1_push_saves_buffer_and_mode_and_drops_to_norm() {
    let mut pad = InputPad::new();
    pad.set_contents("a\nb");
    let mut mode = InputMode::Mult;
    let mut stash = None;
    let mut laat = None;
    let mut slot = None;

    push(&mut slot, &mut pad, &mut mode, &mut stash, &mut laat);

    assert!(pad.as_str().is_empty(), "the pad is reset for ad-hoc input");
    assert_eq!(mode, InputMode::Norm);
    let snap = slot.as_ref().expect("a snapshot is held");
    assert_eq!(snap.buffer, "a\nb");
    assert_eq!(snap.mode, InputMode::Mult);
}

#[test]
fn p2_next_submit_restores_buffer_and_mode() {
    let mut pad = InputPad::new();
    pad.set_contents("a\nb");
    let mut mode = InputMode::Mult;
    let mut stash = None;
    let mut laat = None;
    let mut slot = None;

    push(&mut slot, &mut pad, &mut mode, &mut stash, &mut laat);
    // ...the user runs an ad-hoc command in the now-empty Norm pad...
    pop(&mut slot, &mut pad, &mut mode, &mut stash, &mut laat);

    assert_eq!(pad.as_str(), "a\nb");
    assert_eq!(mode, InputMode::Mult);
    assert!(slot.is_none(), "the slot is cleared after the pop");
}

#[test]
fn p3_second_push_is_a_no_op() {
    let mut pad = InputPad::new();
    pad.set_contents("first");
    let mut mode = InputMode::Mult;
    let mut stash = None;
    let mut laat = None;
    let mut slot = None;

    push(&mut slot, &mut pad, &mut mode, &mut stash, &mut laat);
    // A second push while occupied must not overwrite the first saved state.
    pad.set_contents("second");
    mode = InputMode::Mult;
    push(&mut slot, &mut pad, &mut mode, &mut stash, &mut laat);

    assert_eq!(slot.as_ref().expect("first snapshot kept").buffer, "first");
}

#[test]
fn p4_laat_state_round_trips_with_highlight_and_flags() {
    let mut pad = InputPad::new();
    pad.set_contents("one\ntwo\nthree");
    let mut laat = Some(LaatState::new());
    if let Some(state) = laat.as_mut() {
        state.highlight = 1;
        state.submit_line(1);
        // A non-zero exit flags the line and holds the highlight.
        state.apply_exit_code(Some(1));
    }
    let mut mode = InputMode::Laat;
    let mut stash = None;
    let mut slot = None;

    push(&mut slot, &mut pad, &mut mode, &mut stash, &mut laat);
    pop(&mut slot, &mut pad, &mut mode, &mut stash, &mut laat);

    assert_eq!(mode, InputMode::Laat);
    let restored = laat.expect("LAAT state restored");
    assert_eq!(restored.highlight, 1);
    assert!(restored.is_failed(1), "the probable-failure flag survives");
}

#[test]
fn p5_stashed_draft_survives_the_round_trip() {
    let mut pad = InputPad::new();
    pad.set_contents("a\nb");
    let mut mode = InputMode::Mult;
    let mut stash = Some("draft".to_string());
    let mut laat = None;
    let mut slot = None;

    push(&mut slot, &mut pad, &mut mode, &mut stash, &mut laat);
    assert_eq!(stash, None, "the stash is cleared for the ad-hoc pad");
    pop(&mut slot, &mut pad, &mut mode, &mut stash, &mut laat);

    assert_eq!(stash.as_deref(), Some("draft"));
}
