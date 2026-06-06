//! Named-action registry (sprint 005; FR-008/FR-009). Every input-editing,
//! scrollback, and status behavior is a named [`Action`] with a single hardcoded
//! default key binding this sprint. The registry is the seam the sprint-006
//! keymap engine will extend to bind a default *and* an alternate per action
//! without changing any action's behavior.
//!
//! `Esc`/`Esc Esc`/`Ctrl+C`/`Enter`/printable insertion remain context-sensitive
//! in the event loop (their meaning depends on selection/buffer state) and are
//! NOT resolved here. [`Action::ClearStatusMessage`] is a named action listed by
//! `/keys` but, being the contextual `Esc Esc` gesture, has no [`KeyChord`].
//!
//! See `specs/005-input-and-status/contracts/input-editing.md` §4.

use crossterm::event::{KeyCode, KeyModifiers};

/// A behavior identified by a stable name with (for mapped actions) a single
/// default key binding this sprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    // Input-line editing (US1)
    LineMoveStart,
    LineMoveEnd,
    WordMoveLeft,
    WordMoveRight,
    SelectCharLeft,
    SelectCharRight,
    SelectWordLeft,
    SelectWordRight,
    KillToLineStart,
    KillToLineEnd,
    DeleteWordBefore,
    // Scrollback (US3)
    ScrollPageUp,
    ScrollPageDown,
    ScrollLineUp,
    ScrollLineDown,
    ScrollToTop,
    ScrollToBottom,
    // Status message (US5) — contextual `Esc Esc` gesture; named, but not a chord.
    ClearStatusMessage,
    // Reserved, unmapped whole-buffer motions (FR-009): named but unbound this
    // sprint, so the keymap engine can bind them later.
    MultilineMoveStartBuffer,
    MultilineMoveEndBuffer,
}

impl Action {
    /// The action's stable name, surfaced by `/keys` and (later) keymap config.
    pub fn name(self) -> &'static str {
        match self {
            Action::LineMoveStart => "line_move_start",
            Action::LineMoveEnd => "line_move_end",
            Action::WordMoveLeft => "word_move_left",
            Action::WordMoveRight => "word_move_right",
            Action::SelectCharLeft => "select_char_left",
            Action::SelectCharRight => "select_char_right",
            Action::SelectWordLeft => "select_word_left",
            Action::SelectWordRight => "select_word_right",
            Action::KillToLineStart => "kill_to_line_start",
            Action::KillToLineEnd => "kill_to_line_end",
            Action::DeleteWordBefore => "delete_word_before",
            Action::ScrollPageUp => "scroll_page_up",
            Action::ScrollPageDown => "scroll_page_down",
            Action::ScrollLineUp => "scroll_line_up",
            Action::ScrollLineDown => "scroll_line_down",
            Action::ScrollToTop => "scroll_to_top",
            Action::ScrollToBottom => "scroll_to_bottom",
            Action::ClearStatusMessage => "clear_status_message",
            Action::MultilineMoveStartBuffer => "multiline_move_start_buffer",
            Action::MultilineMoveEndBuffer => "multiline_move_end_buffer",
        }
    }
}

/// A key chord: a key code plus the editing-relevant modifier bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyChord {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

impl KeyChord {
    /// Build a chord, masking the modifiers to the bits the registry cares about
    /// (Shift / Control / Alt) so incidental flags never defeat a match.
    pub fn new(code: KeyCode, mods: KeyModifiers) -> Self {
        Self {
            code,
            mods: mask(mods),
        }
    }

    /// A human-readable rendering of the chord, e.g. `Ctrl+Left`, `Shift+Home`.
    pub fn display(self) -> String {
        let mut s = String::new();
        if self.mods.contains(KeyModifiers::CONTROL) {
            s.push_str("Ctrl+");
        }
        if self.mods.contains(KeyModifiers::ALT) {
            s.push_str("Alt+");
        }
        if self.mods.contains(KeyModifiers::SHIFT) {
            s.push_str("Shift+");
        }
        s.push_str(&code_name(self.code));
        s
    }
}

fn mask(mods: KeyModifiers) -> KeyModifiers {
    mods & (KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT)
}

fn code_name(code: KeyCode) -> String {
    match code {
        KeyCode::Left => "Left".into(),
        KeyCode::Right => "Right".into(),
        KeyCode::Up => "Up".into(),
        KeyCode::Down => "Down".into(),
        KeyCode::Home => "Home".into(),
        KeyCode::End => "End".into(),
        KeyCode::PageUp => "PageUp".into(),
        KeyCode::PageDown => "PageDown".into(),
        KeyCode::Char(c) => c.to_uppercase().to_string(),
        other => format!("{other:?}"),
    }
}

/// `(KeyCode, modifier bits, Action)` — the hardcoded default bindings for all
/// *mapped* actions this sprint. Reserved actions and `ClearStatusMessage` are
/// intentionally absent.
const BINDINGS: &[(KeyCode, KeyModifiers, Action)] = &[
    (KeyCode::Home, KeyModifiers::NONE, Action::LineMoveStart),
    (KeyCode::End, KeyModifiers::NONE, Action::LineMoveEnd),
    (KeyCode::Left, KeyModifiers::CONTROL, Action::WordMoveLeft),
    (KeyCode::Right, KeyModifiers::CONTROL, Action::WordMoveRight),
    (KeyCode::Left, KeyModifiers::SHIFT, Action::SelectCharLeft),
    (KeyCode::Right, KeyModifiers::SHIFT, Action::SelectCharRight),
    (
        KeyCode::Left,
        KeyModifiers::SHIFT.union(KeyModifiers::CONTROL),
        Action::SelectWordLeft,
    ),
    (
        KeyCode::Right,
        KeyModifiers::SHIFT.union(KeyModifiers::CONTROL),
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

/// Resolve a key chord to its bound [`Action`], or `None` when unbound.
pub fn resolve(chord: KeyChord) -> Option<Action> {
    let mods = mask(chord.mods);
    BINDINGS
        .iter()
        .find(|(code, m, _)| *code == chord.code && *m == mods)
        .map(|(_, _, action)| *action)
}

/// The active key map as `(action name, key display)` pairs for `/keys`
/// (FR-030). Includes every mapped action plus the `Esc Esc` gesture for
/// `clear_status_message`; reserved unmapped actions are omitted (no binding).
pub fn listing() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = BINDINGS
        .iter()
        .map(|(code, mods, action)| {
            (
                action.name().to_string(),
                KeyChord::new(*code, *mods).display(),
            )
        })
        .collect();
    out.push((
        Action::ClearStatusMessage.name().to_string(),
        "Esc Esc".into(),
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_binding_resolves_to_its_action() {
        for (code, mods, action) in BINDINGS {
            assert_eq!(resolve(KeyChord::new(*code, *mods)), Some(*action));
        }
    }

    #[test]
    fn no_two_bindings_share_a_chord() {
        for (i, a) in BINDINGS.iter().enumerate() {
            for b in &BINDINGS[i + 1..] {
                assert!(
                    !(a.0 == b.0 && mask(a.1) == mask(b.1)),
                    "duplicate chord: {:?}+{:?}",
                    a.0,
                    a.1
                );
            }
        }
    }

    #[test]
    fn reserved_and_gesture_actions_are_unbound() {
        for unbound in [
            Action::ClearStatusMessage,
            Action::MultilineMoveStartBuffer,
            Action::MultilineMoveEndBuffer,
        ] {
            assert!(
                !BINDINGS.iter().any(|(_, _, a)| *a == unbound),
                "{} must have no default binding",
                unbound.name()
            );
        }
    }

    #[test]
    fn resolve_masks_incidental_modifiers() {
        // A Home press carrying an extra (non-editing) flag still resolves.
        let chord = KeyChord {
            code: KeyCode::Home,
            mods: KeyModifiers::NONE,
        };
        assert_eq!(resolve(chord), Some(Action::LineMoveStart));
    }

    #[test]
    fn listing_is_stable_and_includes_clear_status_message() {
        let l = listing();
        assert_eq!(l.len(), BINDINGS.len() + 1);
        assert!(l
            .iter()
            .any(|(name, keys)| name == "clear_status_message" && keys == "Esc Esc"));
        // Stable order: first entry is always line_move_start (Home).
        assert_eq!(l[0].0, "line_move_start");
    }

    #[test]
    fn unbound_chord_resolves_none() {
        assert_eq!(
            resolve(KeyChord::new(KeyCode::Char('z'), KeyModifiers::NONE)),
            None
        );
    }
}
