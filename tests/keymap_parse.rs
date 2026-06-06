//! Key-string grammar tests (T004): case-insensitivity, modifier-order
//! tolerance, short-modifier-names-only, unknown key/empty rejection, the
//! `Esc Esc` chord, unsupported chords, and `display()` round-tripping.
//! See `specs/006-keymap-engine/contracts/key-string.md`.

use crossterm::event::{KeyCode, KeyModifiers};
use kapollo::action::{KeyChord, KeyParseReason, KeySpec};

fn single(code: KeyCode, mods: KeyModifiers) -> KeySpec {
    KeySpec::Single(KeyChord::new(code, mods))
}

#[test]
fn case_insensitive_modifiers_and_keys_resolve_equal() {
    let canonical = KeySpec::parse("Ctrl+Left").unwrap();
    assert_eq!(KeySpec::parse("ctrl+left").unwrap(), canonical);
    assert_eq!(KeySpec::parse("CTRL+LEFT").unwrap(), canonical);
    assert_eq!(canonical, single(KeyCode::Left, KeyModifiers::CONTROL));
}

#[test]
fn modifier_order_is_irrelevant() {
    assert_eq!(
        KeySpec::parse("Shift+Ctrl+Left").unwrap(),
        KeySpec::parse("Ctrl+Shift+Left").unwrap()
    );
    assert_eq!(
        KeySpec::parse("Ctrl+Shift+Left").unwrap(),
        single(KeyCode::Left, KeyModifiers::SHIFT | KeyModifiers::CONTROL)
    );
}

#[test]
fn short_modifier_names_only_long_forms_reject() {
    let err = KeySpec::parse("Control+Left").unwrap_err();
    assert_eq!(err.reason, KeyParseReason::UnknownModifier);
    assert!(KeySpec::parse("Super+Left").is_err());
    assert!(KeySpec::parse("Cmd+Left").is_err());
    assert!(KeySpec::parse("Meta+Left").is_err());
}

#[test]
fn unknown_key_name_is_rejected() {
    let err = KeySpec::parse("Ctrl+Nope").unwrap_err();
    assert_eq!(err.reason, KeyParseReason::UnknownKey);
}

#[test]
fn empty_string_is_rejected() {
    let err = KeySpec::parse("").unwrap_err();
    assert_eq!(err.reason, KeyParseReason::Empty);
    assert_eq!(
        KeySpec::parse("   ").unwrap_err().reason,
        KeyParseReason::Empty
    );
}

#[test]
fn esc_esc_parses_as_a_chord() {
    let esc = KeyChord::new(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(KeySpec::parse("Esc Esc").unwrap(), KeySpec::Chord(esc, esc));
    // Case-insensitive halves.
    assert_eq!(KeySpec::parse("esc esc").unwrap(), KeySpec::Chord(esc, esc));
}

#[test]
fn multi_key_sequences_other_than_esc_esc_reject() {
    assert_eq!(
        KeySpec::parse("Esc Esc Esc").unwrap_err().reason,
        KeyParseReason::UnsupportedChord
    );
    assert_eq!(
        KeySpec::parse("Ctrl+a Ctrl+b").unwrap_err().reason,
        KeyParseReason::UnsupportedChord
    );
    assert_eq!(
        KeySpec::parse("Esc Left").unwrap_err().reason,
        KeyParseReason::UnsupportedChord
    );
}

#[test]
fn display_round_trips_through_parse() {
    let representative = [
        "Ctrl+Left",
        "Ctrl+Right",
        "Shift+Home",
        "Alt+Enter",
        "Shift+Enter",
        "PageUp",
        "Ctrl+Y",
        "a",
        "Esc Esc",
    ];
    for s in representative {
        let spec = KeySpec::parse(s).expect("representative parses");
        let rendered = spec.display();
        let reparsed = KeySpec::parse(&rendered).expect("display() reparses");
        assert_eq!(
            spec, reparsed,
            "round-trip failed for {s:?} -> {rendered:?}"
        );
    }
}
