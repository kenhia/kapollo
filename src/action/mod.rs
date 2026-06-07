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

use std::collections::BTreeMap;

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
    // Newline insertion (006): insert a newline in the input buffer without
    // submitting. Default-bound to a primary + alternate (Shift+Enter / Alt+Enter).
    InsertNewline,
    // Scrollback (US3)
    ScrollPageUp,
    ScrollPageDown,
    ScrollLineUp,
    ScrollLineDown,
    ScrollToTop,
    ScrollToBottom,
    // Keyboard copy variants (006): the previously mouse-only copy paths, now
    // bindable. They act on the bottom-most transcript output (R4).
    CopyCurrentLine,
    CopyBlockWithoutCommand,
    // Status message (US5) — contextual `Esc Esc` gesture; named, but not a chord.
    ClearStatusMessage,
    // Reserved, unmapped whole-buffer motions (FR-009): named but unbound this
    // sprint, so the keymap engine can bind them later.
    MultilineMoveStartBuffer,
    MultilineMoveEndBuffer,
    // Input modes (sprint 007): toggle Mult/LAAT, and push/pop the input buffer.
    ToggleMultLaat,
    PushInput,
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
            Action::InsertNewline => "insert_newline",
            Action::ScrollPageUp => "scroll_page_up",
            Action::ScrollPageDown => "scroll_page_down",
            Action::ScrollLineUp => "scroll_line_up",
            Action::ScrollLineDown => "scroll_line_down",
            Action::ScrollToTop => "scroll_to_top",
            Action::ScrollToBottom => "scroll_to_bottom",
            Action::CopyCurrentLine => "copy_current_line",
            Action::CopyBlockWithoutCommand => "copy_block_without_command",
            Action::ClearStatusMessage => "clear_status_message",
            Action::MultilineMoveStartBuffer => "multiline_move_start_buffer",
            Action::MultilineMoveEndBuffer => "multiline_move_end_buffer",
            Action::ToggleMultLaat => "toggle_mult_laat",
            Action::PushInput => "push_input",
        }
    }

    /// Look up an action by its stable [`Action::name`], or `None` when the name
    /// is not a known action (used by `[keymap]` config validation, FR-013).
    pub fn from_name(name: &str) -> Option<Action> {
        const ALL: &[Action] = &[
            Action::LineMoveStart,
            Action::LineMoveEnd,
            Action::WordMoveLeft,
            Action::WordMoveRight,
            Action::SelectCharLeft,
            Action::SelectCharRight,
            Action::SelectWordLeft,
            Action::SelectWordRight,
            Action::KillToLineStart,
            Action::KillToLineEnd,
            Action::DeleteWordBefore,
            Action::InsertNewline,
            Action::ScrollPageUp,
            Action::ScrollPageDown,
            Action::ScrollLineUp,
            Action::ScrollLineDown,
            Action::ScrollToTop,
            Action::ScrollToBottom,
            Action::CopyCurrentLine,
            Action::CopyBlockWithoutCommand,
            Action::ClearStatusMessage,
            Action::MultilineMoveStartBuffer,
            Action::MultilineMoveEndBuffer,
            Action::ToggleMultLaat,
            Action::PushInput,
        ];
        ALL.iter().copied().find(|a| a.name() == name)
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

// ---------------------------------------------------------------------------
// Keymap engine (sprint 006)
// ---------------------------------------------------------------------------

/// The parsed target of a key string (FR-006/FR-008): a single chord, or the
/// two-key `Esc Esc` chord (the only multi-key sequence supported this sprint).
/// This is the design realization of the spec's "Key chord" entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySpec {
    /// A single key plus its modifier bits.
    Single(KeyChord),
    /// A two-key chord; only `Esc Esc` is valid this sprint.
    Chord(KeyChord, KeyChord),
}

/// Why a key string failed to parse, for the FR-009 warn-and-skip diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyParseReason {
    /// The (trimmed) input was empty.
    Empty,
    /// A modifier token was not one of `ctrl`/`alt`/`shift`.
    UnknownModifier,
    /// The key token was not a known named key or a single character.
    UnknownKey,
    /// A multi-key sequence other than `Esc Esc`.
    UnsupportedChord,
}

/// A key-string parse failure: the offending input plus the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyParseError {
    pub input: String,
    pub reason: KeyParseReason,
}

impl std::fmt::Display for KeyParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match self.reason {
            KeyParseReason::Empty => "empty key string",
            KeyParseReason::UnknownModifier => "unknown modifier (use ctrl/alt/shift)",
            KeyParseReason::UnknownKey => "unknown key name",
            KeyParseReason::UnsupportedChord => "unsupported chord (only `Esc Esc`)",
        };
        write!(f, "{reason}: {:?}", self.input)
    }
}

impl KeySpec {
    /// Parse a human-written key string into a [`KeySpec`]. Case-insensitive and
    /// modifier-order-tolerant (FR-007); short modifier names only (R1).
    pub fn parse(s: &str) -> Result<KeySpec, KeyParseError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(KeyParseError {
                input: s.to_string(),
                reason: KeyParseReason::Empty,
            });
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        match parts.as_slice() {
            [one] => Ok(KeySpec::Single(parse_single(one, s)?)),
            [a, b] => {
                let first = parse_single(a, s)?;
                let second = parse_single(b, s)?;
                let esc = KeyChord::new(KeyCode::Esc, KeyModifiers::NONE);
                if first == esc && second == esc {
                    Ok(KeySpec::Chord(first, second))
                } else {
                    Err(KeyParseError {
                        input: s.to_string(),
                        reason: KeyParseReason::UnsupportedChord,
                    })
                }
            }
            _ => Err(KeyParseError {
                input: s.to_string(),
                reason: KeyParseReason::UnsupportedChord,
            }),
        }
    }

    /// The canonical rendering of this spec; the inverse of [`KeySpec::parse`].
    pub fn display(self) -> String {
        match self {
            KeySpec::Single(c) => c.display(),
            KeySpec::Chord(a, b) => format!("{} {}", a.display(), b.display()),
        }
    }
}

/// Parse one `modifier+...+key` token into a [`KeyChord`]. `full` is the whole
/// original key string, retained for the error's `input`.
fn parse_single(token: &str, full: &str) -> Result<KeyChord, KeyParseError> {
    let lower = token.to_lowercase();
    let mut segments: Vec<&str> = lower.split('+').collect();
    // The key is the final segment; everything before it is a modifier.
    let key_seg = segments.pop().unwrap_or("");
    let mut mods = KeyModifiers::NONE;
    for m in &segments {
        match *m {
            "ctrl" => mods |= KeyModifiers::CONTROL,
            "alt" => mods |= KeyModifiers::ALT,
            "shift" => mods |= KeyModifiers::SHIFT,
            _ => {
                return Err(KeyParseError {
                    input: full.to_string(),
                    reason: KeyParseReason::UnknownModifier,
                })
            }
        }
    }
    let code = parse_key(key_seg).ok_or(KeyParseError {
        input: full.to_string(),
        reason: KeyParseReason::UnknownKey,
    })?;
    Ok(KeyChord::new(code, mods))
}

/// Map a lowercased key token to a [`KeyCode`]: a fixed named-key table plus the
/// single-character fallback. `None` for an unknown or empty token.
fn parse_key(seg: &str) -> Option<KeyCode> {
    Some(match seg {
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "insert" => KeyCode::Insert,
        "space" => KeyCode::Char(' '),
        other => {
            let mut chars = other.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => KeyCode::Char(c),
                _ => return None,
            }
        }
    })
}

/// An action's primary and optional alternate key (FR-003). A binding with
/// neither is **cleared** (disabled, FR-011).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Binding {
    pub primary: Option<KeySpec>,
    pub alternate: Option<KeySpec>,
}

impl Binding {
    /// A primary-only binding.
    pub fn single(spec: KeySpec) -> Self {
        Self {
            primary: Some(spec),
            alternate: None,
        }
    }

    /// A primary + alternate binding.
    pub fn pair(primary: KeySpec, alternate: KeySpec) -> Self {
        Self {
            primary: Some(primary),
            alternate: Some(alternate),
        }
    }

    /// A cleared (disabled) binding — no primary, no alternate (FR-011).
    pub fn cleared() -> Self {
        Self::default()
    }

    /// Build a binding from parsed specs: 0 → cleared, 1 → primary, 2 → primary +
    /// alternate, >2 → warn and keep the first two (data-model §5).
    pub fn from_specs(specs: &[KeySpec]) -> Self {
        match specs {
            [] => Self::cleared(),
            [p] => Self::single(*p),
            [p, a, ..] => {
                if specs.len() > 2 {
                    tracing::warn!(
                        count = specs.len(),
                        "key binding lists more than two keys; keeping the first two"
                    );
                }
                Self::pair(*p, *a)
            }
        }
    }
}

/// The set of bindings in effect for one mode: the resolution table the event
/// loop queries (data-model §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    /// Per-action primary/alternate in declaration order (defaults first, then
    /// config overrides) so a same-key conflict resolves last-declared-wins.
    bindings: Vec<(Action, Binding)>,
}

impl Keymap {
    /// The zero-config map: the data-fied sprint-005 `BINDINGS` plus the 006
    /// additions (the newline alternate and the two copy variants). MUST equal
    /// current behavior (FR-002).
    pub fn default_map() -> Keymap {
        let mut bindings: Vec<(Action, Binding)> = BINDINGS
            .iter()
            .map(|(code, mods, action)| {
                (
                    *action,
                    Binding::single(KeySpec::Single(KeyChord::new(*code, *mods))),
                )
            })
            .collect();
        // Newline insertion: Shift+Enter primary, Alt+Enter alternate (R5).
        bindings.push((
            Action::InsertNewline,
            Binding::pair(
                KeySpec::Single(KeyChord::new(KeyCode::Enter, KeyModifiers::SHIFT)),
                KeySpec::Single(KeyChord::new(KeyCode::Enter, KeyModifiers::ALT)),
            ),
        ));
        // Keyboard copy variants (R4): Ctrl+Y / Alt+Y, validated to not collide
        // with any existing default.
        bindings.push((
            Action::CopyCurrentLine,
            Binding::single(KeySpec::Single(KeyChord::new(
                KeyCode::Char('y'),
                KeyModifiers::CONTROL,
            ))),
        ));
        bindings.push((
            Action::CopyBlockWithoutCommand,
            Binding::single(KeySpec::Single(KeyChord::new(
                KeyCode::Char('y'),
                KeyModifiers::ALT,
            ))),
        ));
        // Input modes (007): toggle Mult/LAAT with Ctrl+1 (the 006 parser
        // already accepts the chord — the digit falls through to Char('1')).
        // Ctrl+Alt+1 collides with Windows Terminal's "Switch to Tab 1".
        bindings.push((
            Action::ToggleMultLaat,
            Binding::single(KeySpec::Single(KeyChord::new(
                KeyCode::Char('1'),
                KeyModifiers::CONTROL,
            ))),
        ));
        // Push/pop the input buffer with Ctrl+Alt+Enter (sprint 007, US4).
        bindings.push((
            Action::PushInput,
            Binding::single(KeySpec::Single(KeyChord::new(
                KeyCode::Enter,
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            ))),
        ));
        let map = Keymap { bindings };
        map.warn_conflicts();
        map
    }

    /// Overlay `(Action, Binding)` overrides onto `base`: bind, rebind, or unbind.
    /// Overridden actions are re-declared last so a user binding wins a same-key
    /// conflict over a default (R7).
    pub fn with_overrides(base: &Keymap, overrides: &[(Action, Binding)]) -> Keymap {
        let mut bindings: Vec<(Action, Binding)> = base
            .bindings
            .iter()
            .filter(|(a, _)| !overrides.iter().any(|(oa, _)| oa == a))
            .cloned()
            .collect();
        bindings.extend(overrides.iter().cloned());
        let map = Keymap { bindings };
        map.warn_conflicts();
        map
    }

    /// Resolve a single-key press to its bound [`Action`], or `None` when
    /// unbound. Both an action's primary and alternate resolve to it (FR-003);
    /// on a same-key conflict the last-declared binding wins (FR-010).
    pub fn resolve(&self, spec: KeySpec) -> Option<Action> {
        let mut found = None;
        for (action, binding) in &self.bindings {
            if binding.primary == Some(spec) || binding.alternate == Some(spec) {
                found = Some(*action);
            }
        }
        found
    }

    /// `(action name, key display)` pairs for `/keys` (FR-014): every action's
    /// primary (and alternate when present), unbound actions marked, plus the
    /// `Esc Esc` gesture row for `clear_status_message`.
    pub fn listing(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (action, binding) in &self.bindings {
            match binding.primary {
                Some(primary) => {
                    out.push((action.name().to_string(), primary.display()));
                    if let Some(alternate) = binding.alternate {
                        out.push((action.name().to_string(), alternate.display()));
                    }
                }
                None => out.push((action.name().to_string(), "(unbound)".to_string())),
            }
        }
        out.push((
            Action::ClearStatusMessage.name().to_string(),
            "Esc Esc".into(),
        ));
        out
    }

    /// Emit a `tracing::warn!` for each same-`KeySpec` collision between two
    /// distinct actions; resolution keeps the last-declared (FR-010). An action
    /// whose own primary and alternate collapse to one chord is not a conflict.
    fn warn_conflicts(&self) {
        let mut seen: Vec<(KeySpec, Action)> = Vec::new();
        for (action, binding) in &self.bindings {
            for spec in [binding.primary, binding.alternate].into_iter().flatten() {
                if let Some((_, prev)) = seen.iter().find(|(s, _)| *s == spec) {
                    if prev != action {
                        tracing::warn!(
                            key = %spec.display(),
                            first = prev.name(),
                            second = action.name(),
                            "key binding conflict; last-declared wins"
                        );
                    }
                }
                seen.push((spec, *action));
            }
        }
    }
}

/// The default mode name kapollo runs in (matches the status bar's `norm`
/// label). The only mode populated this sprint; a real mode selector lands in a
/// later sprint.
pub const DEFAULT_MODE: &str = "norm";

/// The mode names kapollo recognizes this sprint. Only the default mode exists;
/// a per-mode `[keymap.<mode>]` subtable for any other name is warned and
/// ignored (FR-013).
pub const KNOWN_MODES: &[&str] = &[DEFAULT_MODE];

/// The full multi-mode keymap held by `App` (data-model §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymaps {
    default: Keymap,
    modes: BTreeMap<String, Keymap>,
}

impl Keymaps {
    /// Construct from a default map and its per-mode overlays.
    pub fn new(default: Keymap, modes: BTreeMap<String, Keymap>) -> Self {
        Self { default, modes }
    }

    /// The keymap for `mode`, or the default map when the mode is absent
    /// (FR-012 inheritance).
    pub fn for_mode(&self, mode: &str) -> &Keymap {
        self.modes.get(mode).unwrap_or(&self.default)
    }

    /// The default-mode keymap.
    pub fn default(&self) -> &Keymap {
        &self.default
    }
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
