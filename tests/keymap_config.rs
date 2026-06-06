//! `[keymap]` config-surface tests (T010/T017): default identity, string and
//! array bindings, primary/alternate, and unknown-action tolerance.
//! See `specs/006-keymap-engine/contracts/keymap-config.md`.

use std::path::Path;

use crossterm::event::{KeyCode, KeyModifiers};
use kapollo::action::{Action, KeyChord, KeySpec};
use kapollo::config::Config;

fn parse(toml: &str) -> Config {
    Config::from_toml(toml, Path::new("test.toml")).expect("config parses")
}

fn key(code: KeyCode, mods: KeyModifiers) -> KeySpec {
    KeySpec::Single(KeyChord::new(code, mods))
}

#[test]
fn no_keymap_table_yields_default_map() {
    let cfg = parse("leader_char = \"/\"\n");
    assert_eq!(cfg.keymaps, Config::default().keymaps);
}

#[test]
fn string_binding_sets_primary_only() {
    let cfg = parse("[keymap]\nword_move_left = \"Ctrl+B\"\n");
    let map = cfg.keymaps.default();
    // The configured key fires...
    assert_eq!(
        map.resolve(KeySpec::parse("Ctrl+B").unwrap()),
        Some(Action::WordMoveLeft)
    );
    // ...and the former default no longer maps to it.
    assert_eq!(map.resolve(key(KeyCode::Left, KeyModifiers::CONTROL)), None);
}

#[test]
fn array_binding_sets_primary_and_alternate() {
    let cfg = parse("[keymap]\nword_move_left = [\"Ctrl+B\", \"Alt+B\"]\n");
    let map = cfg.keymaps.default();
    assert_eq!(
        map.resolve(KeySpec::parse("Ctrl+B").unwrap()),
        Some(Action::WordMoveLeft)
    );
    assert_eq!(
        map.resolve(KeySpec::parse("Alt+B").unwrap()),
        Some(Action::WordMoveLeft)
    );
}

#[test]
fn one_element_array_behaves_as_primary_only() {
    let cfg = parse("[keymap]\nword_move_left = [\"Ctrl+B\"]\n");
    let map = cfg.keymaps.default();
    assert_eq!(
        map.resolve(KeySpec::parse("Ctrl+B").unwrap()),
        Some(Action::WordMoveLeft)
    );
    // No alternate was set, and the former default is gone.
    assert_eq!(map.resolve(key(KeyCode::Left, KeyModifiers::CONTROL)), None);
}

#[test]
fn unknown_action_name_is_warned_and_ignored() {
    let cfg = parse("[keymap]\nnot_an_action = \"Ctrl+B\"\nword_move_right = \"Ctrl+J\"\n");
    let map = cfg.keymaps.default();
    // The valid binding still applies.
    assert_eq!(
        map.resolve(KeySpec::parse("Ctrl+J").unwrap()),
        Some(Action::WordMoveRight)
    );
    // The unknown action does not bind anything.
    assert_eq!(map.resolve(KeySpec::parse("Ctrl+B").unwrap()), None);
    // Unrelated defaults are untouched.
    assert_eq!(
        map.resolve(key(KeyCode::Left, KeyModifiers::CONTROL)),
        Some(Action::WordMoveLeft)
    );
}

#[test]
fn empty_value_clears_and_disables_action() {
    // An empty string clears the action; its former default no longer resolves.
    let cfg = parse("[keymap]\nword_move_left = \"\"\n");
    let map = cfg.keymaps.default();
    assert_eq!(map.resolve(key(KeyCode::Left, KeyModifiers::CONTROL)), None);
    assert!(
        map.listing()
            .iter()
            .any(|(name, keys)| name == "word_move_left" && keys == "(unbound)"),
        "a cleared action should be shown unbound in /keys"
    );
    // An empty array clears too.
    let cfg = parse("[keymap]\nword_move_right = []\n");
    assert_eq!(
        cfg.keymaps
            .default()
            .resolve(key(KeyCode::Right, KeyModifiers::CONTROL)),
        None
    );
}

#[test]
fn unparseable_key_is_skipped_and_others_apply() {
    // A bad key string is skipped (the action keeps its default); a valid
    // binding in the same table still applies, and kapollo still parses.
    let cfg = parse("[keymap]\nword_move_left = \"Ctrl+Nope\"\nword_move_right = \"Ctrl+J\"\n");
    let map = cfg.keymaps.default();
    // The unparseable binding was skipped, so the default survives.
    assert_eq!(
        map.resolve(key(KeyCode::Left, KeyModifiers::CONTROL)),
        Some(Action::WordMoveLeft)
    );
    // The valid sibling binding applies.
    assert_eq!(
        map.resolve(KeySpec::parse("Ctrl+J").unwrap()),
        Some(Action::WordMoveRight)
    );
}

#[test]
fn conflicting_bindings_last_declared_wins() {
    // Two distinct actions bound to the same key: the last-declared (in config
    // document order) wins (FR-010).
    let cfg = parse("[keymap]\nword_move_left = \"Ctrl+G\"\nword_move_right = \"Ctrl+G\"\n");
    assert_eq!(
        cfg.keymaps
            .default()
            .resolve(KeySpec::parse("Ctrl+G").unwrap()),
        Some(Action::WordMoveRight)
    );
    // Reversing the declaration order flips the winner.
    let cfg = parse("[keymap]\nword_move_right = \"Ctrl+G\"\nword_move_left = \"Ctrl+G\"\n");
    assert_eq!(
        cfg.keymaps
            .default()
            .resolve(KeySpec::parse("Ctrl+G").unwrap()),
        Some(Action::WordMoveLeft)
    );
}

#[test]
fn per_mode_section_overrides_only_listed_actions_and_inherits_rest() {
    // A `[keymap.norm]` subtable targets the default mode this sprint: the listed
    // action is rebound and every other action is inherited (FR-012).
    let cfg = parse("[keymap.norm]\nscroll_line_up = \"Ctrl+P\"\n");
    let map = cfg.keymaps.for_mode("norm");
    // The overridden action resolves to the new key...
    assert_eq!(
        map.resolve(KeySpec::parse("Ctrl+P").unwrap()),
        Some(Action::ScrollLineUp)
    );
    // ...its former default no longer resolves...
    assert_eq!(map.resolve(key(KeyCode::PageUp, KeyModifiers::SHIFT)), None);
    // ...and an unlisted action keeps its inherited default.
    assert_eq!(
        map.resolve(key(KeyCode::Home, KeyModifiers::NONE)),
        Some(Action::LineMoveStart)
    );
}

#[test]
fn unknown_mode_section_is_warned_and_ignored() {
    // A subtable for a mode kapollo does not know is ignored, leaving the
    // effective keymaps identical to the defaults (FR-013).
    let cfg = parse("[keymap.bogus]\nscroll_line_up = \"Ctrl+P\"\n");
    assert_eq!(cfg.keymaps, Config::default().keymaps);
}
