//! Keymap engine tests (T009/T016): the default map reproduces the legacy
//! bindings, the copy variants and the newline primary/alternate are bound,
//! overrides rebind, and per-mode maps inherit the default.
//! See `specs/006-keymap-engine/contracts/keymap-engine.md`.

use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyModifiers};
use kapollo::action::{Action, Binding, KeyChord, KeySpec, Keymap, Keymaps};

fn key(code: KeyCode, mods: KeyModifiers) -> KeySpec {
    KeySpec::Single(KeyChord::new(code, mods))
}

#[test]
fn default_map_matches_legacy_bindings() {
    let map = Keymap::default_map();
    let cases = [
        (KeyCode::Home, KeyModifiers::NONE, Action::LineMoveStart),
        (KeyCode::End, KeyModifiers::NONE, Action::LineMoveEnd),
        (KeyCode::Left, KeyModifiers::CONTROL, Action::WordMoveLeft),
        (KeyCode::Right, KeyModifiers::CONTROL, Action::WordMoveRight),
        (KeyCode::Left, KeyModifiers::SHIFT, Action::SelectCharLeft),
        (KeyCode::Right, KeyModifiers::SHIFT, Action::SelectCharRight),
        (
            KeyCode::Left,
            KeyModifiers::SHIFT | KeyModifiers::CONTROL,
            Action::SelectWordLeft,
        ),
        (
            KeyCode::Right,
            KeyModifiers::SHIFT | KeyModifiers::CONTROL,
            Action::SelectWordRight,
        ),
        (
            KeyCode::Char('u'),
            KeyModifiers::CONTROL,
            Action::KillToLineStart,
        ),
        (
            KeyCode::Char('k'),
            KeyModifiers::CONTROL,
            Action::KillToLineEnd,
        ),
        (
            KeyCode::Char('w'),
            KeyModifiers::CONTROL,
            Action::DeleteWordBefore,
        ),
        (KeyCode::PageUp, KeyModifiers::NONE, Action::ScrollPageUp),
        (
            KeyCode::PageDown,
            KeyModifiers::NONE,
            Action::ScrollPageDown,
        ),
        (KeyCode::PageUp, KeyModifiers::SHIFT, Action::ScrollLineUp),
        (
            KeyCode::PageDown,
            KeyModifiers::SHIFT,
            Action::ScrollLineDown,
        ),
        (KeyCode::Home, KeyModifiers::SHIFT, Action::ScrollToTop),
        (KeyCode::End, KeyModifiers::SHIFT, Action::ScrollToBottom),
    ];
    for (code, mods, action) in cases {
        assert_eq!(
            map.resolve(key(code, mods)),
            Some(action),
            "{action:?} should resolve from its legacy default"
        );
    }
}

#[test]
fn copy_variants_are_bound_by_default() {
    let map = Keymap::default_map();
    assert_eq!(
        map.resolve(key(KeyCode::Char('y'), KeyModifiers::CONTROL)),
        Some(Action::CopyCurrentLine)
    );
    assert_eq!(
        map.resolve(key(KeyCode::Char('y'), KeyModifiers::ALT)),
        Some(Action::CopyBlockWithoutCommand)
    );
}

#[test]
fn insert_newline_has_primary_and_alternate_by_default() {
    let map = Keymap::default_map();
    assert_eq!(
        map.resolve(key(KeyCode::Enter, KeyModifiers::SHIFT)),
        Some(Action::InsertNewline)
    );
    assert_eq!(
        map.resolve(key(KeyCode::Enter, KeyModifiers::ALT)),
        Some(Action::InsertNewline)
    );
}

#[test]
fn toggle_mult_laat_is_bound_to_ctrl_1_by_default() {
    // Sprint 007: the Ctrl+1 mode toggle is a named default binding, and the
    // 006 parser reaches the same chord from its key string (K1/K2/K3).
    // (Ctrl+Alt+1 collides with Windows Terminal's "Switch to Tab 1".)
    let map = Keymap::default_map();
    let chord = key(KeyCode::Char('1'), KeyModifiers::CONTROL);
    assert_eq!(map.resolve(chord), Some(Action::ToggleMultLaat));
    assert_eq!(
        Action::from_name("toggle_mult_laat"),
        Some(Action::ToggleMultLaat)
    );
    assert_eq!(
        map.resolve(KeySpec::parse("Ctrl+1").unwrap()),
        Some(Action::ToggleMultLaat)
    );
}

#[test]
fn push_input_is_bound_to_ctrl_alt_enter_by_default() {
    // Sprint 007 US4: Ctrl+Alt+Enter pushes the input buffer; the action is a
    // named default and the parser reaches the same chord from its key string.
    let map = Keymap::default_map();
    let chord = key(KeyCode::Enter, KeyModifiers::CONTROL | KeyModifiers::ALT);
    assert_eq!(map.resolve(chord), Some(Action::PushInput));
    assert_eq!(Action::from_name("push_input"), Some(Action::PushInput));
    assert_eq!(
        map.resolve(KeySpec::parse("Ctrl+Alt+Enter").unwrap()),
        Some(Action::PushInput)
    );
}

#[test]
fn override_rebinds_and_old_key_stops_resolving() {
    let rebind = KeySpec::parse("Ctrl+B").unwrap();
    let map = Keymap::with_overrides(
        &Keymap::default_map(),
        &[(Action::WordMoveLeft, Binding::single(rebind))],
    );
    // The new key fires.
    assert_eq!(map.resolve(rebind), Some(Action::WordMoveLeft));
    // The former default no longer maps to the action.
    assert_eq!(map.resolve(key(KeyCode::Left, KeyModifiers::CONTROL)), None);
}

#[test]
fn for_mode_inherits_default_for_unlisted_actions() {
    let mode = Keymap::with_overrides(
        &Keymap::default_map(),
        &[(
            Action::ScrollLineUp,
            Binding::single(KeySpec::parse("Ctrl+P").unwrap()),
        )],
    );
    let mut modes = BTreeMap::new();
    modes.insert("laat".to_string(), mode);
    let keymaps = Keymaps::new(Keymap::default_map(), modes);

    // The named mode applies its override...
    assert_eq!(
        keymaps
            .for_mode("laat")
            .resolve(KeySpec::parse("Ctrl+P").unwrap()),
        Some(Action::ScrollLineUp)
    );
    // ...and inherits every unlisted default.
    assert_eq!(
        keymaps
            .for_mode("laat")
            .resolve(key(KeyCode::Home, KeyModifiers::NONE)),
        Some(Action::LineMoveStart)
    );
    // An unknown mode falls back to the default map.
    assert_eq!(
        keymaps
            .for_mode("unknown")
            .resolve(key(KeyCode::Home, KeyModifiers::NONE)),
        Some(Action::LineMoveStart)
    );
}

#[test]
fn cleared_action_resolves_from_no_chord() {
    // Clearing an action removes it from the resolution table: its former
    // default no longer resolves (FR-011).
    let map = Keymap::with_overrides(
        &Keymap::default_map(),
        &[(Action::WordMoveLeft, Binding::cleared())],
    );
    assert_eq!(map.resolve(key(KeyCode::Left, KeyModifiers::CONTROL)), None);
}

#[test]
fn conflict_keeps_last_declared() {
    // Two distinct actions on the same key: the last-declared override wins.
    let shared = KeySpec::parse("Ctrl+G").unwrap();
    let map = Keymap::with_overrides(
        &Keymap::default_map(),
        &[
            (Action::WordMoveLeft, Binding::single(shared)),
            (Action::WordMoveRight, Binding::single(shared)),
        ],
    );
    assert_eq!(map.resolve(shared), Some(Action::WordMoveRight));
}

#[test]
fn self_collapsing_primary_and_alternate_is_not_a_conflict() {
    // An action whose own primary and alternate are the same chord still fires
    // (it is not treated as a conflict with itself).
    let same = KeySpec::parse("Ctrl+G").unwrap();
    let map = Keymap::with_overrides(
        &Keymap::default_map(),
        &[(Action::WordMoveLeft, Binding::pair(same, same))],
    );
    assert_eq!(map.resolve(same), Some(Action::WordMoveLeft));
}

#[test]
fn listing_reflects_effective_map_including_unbound() {
    // `/keys` reflects the effective map (FR-014): an action with an alternate
    // shows two rows, a cleared action shows "(unbound)", and the contextual
    // `Esc Esc` gesture row is always present.
    let map = Keymap::with_overrides(
        &Keymap::default_map(),
        &[(Action::WordMoveLeft, Binding::cleared())],
    );
    let listing = map.listing();

    // insert_newline has a primary and an alternate by default → two rows.
    let newline_rows = listing
        .iter()
        .filter(|(name, _)| name == "insert_newline")
        .count();
    assert_eq!(newline_rows, 2, "primary + alternate should each list");

    // The cleared action is shown unbound.
    assert!(
        listing
            .iter()
            .any(|(name, keys)| name == "word_move_left" && keys == "(unbound)"),
        "a cleared action should render as (unbound)"
    );

    // The contextual gesture row is present.
    assert!(
        listing
            .iter()
            .any(|(name, keys)| name == "clear_status_message" && keys == "Esc Esc"),
        "the Esc Esc gesture row should always be listed"
    );
}
