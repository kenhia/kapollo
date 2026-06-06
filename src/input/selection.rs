//! Input-pad selection: a character-anchored range within the input buffer
//! (sprint 005, US1; FR-003/FR-004). At most one selection is active in the pad
//! at a time; cross-pad arbitration (input vs. transcript) is handled by the app
//! (US5). See `specs/005-input-and-status/contracts/input-editing.md` §3.

/// A character-anchored selection range within the input buffer. `anchor` is the
/// fixed end (where the selection began); `caret` tracks the moving end and
/// mirrors the pad's cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputSelection {
    pub anchor: usize,
    pub caret: usize,
}

impl InputSelection {
    /// Start a zero-width selection anchored at `at`.
    pub fn new(at: usize) -> Self {
        Self {
            anchor: at,
            caret: at,
        }
    }

    /// The normalized `(min, max)` char range.
    pub fn range(&self) -> (usize, usize) {
        if self.anchor <= self.caret {
            (self.anchor, self.caret)
        } else {
            (self.caret, self.anchor)
        }
    }

    /// Whether the selection covers no characters.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.caret
    }
}

/// Which pad, if any, currently owns *the* selection (sprint 005, US5; FR-027).
/// At most one pad holds a selection at a time, and the sum type makes that
/// invariant **structural** — representing two simultaneous selections is
/// unrepresentable. The `Input` variant embeds the pad's [`InputSelection`]; the
/// `Transcript` variant carries the normalized 004 content-cell range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveSelection {
    /// Neither pad has a selection.
    None,
    /// The input pad owns the selection.
    Input(InputSelection),
    /// The transcript pad owns the selection (normalized content-cell range).
    Transcript(
        crate::selection::coords::Cell,
        crate::selection::coords::Cell,
    ),
}

impl ActiveSelection {
    /// Whether no pad holds a selection.
    pub fn is_none(&self) -> bool {
        matches!(self, ActiveSelection::None)
    }

    /// Whether some pad holds a selection.
    pub fn is_active(&self) -> bool {
        !self.is_none()
    }

    /// The input-pad selection, if the input pad owns it.
    pub fn input(&self) -> Option<InputSelection> {
        match self {
            ActiveSelection::Input(sel) => Some(*sel),
            _ => None,
        }
    }

    /// The transcript range, if the transcript pad owns it.
    pub fn transcript(
        &self,
    ) -> Option<(
        crate::selection::coords::Cell,
        crate::selection::coords::Cell,
    )> {
        match self {
            ActiveSelection::Transcript(a, b) => Some((*a, *b)),
            _ => None,
        }
    }
}

/// The effect a single `Esc` press has, given the current interaction state
/// (sprint 005, US5; FR-029). `esc_pending` is true when the immediately
/// preceding key was also `Esc` — the double-Esc gesture, tracked by a keypress
/// flag with **no** wall clock (research R6). The double-Esc additionally clears
/// the status message (FR-026), an independent effect the caller applies
/// whenever `esc_pending` is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscAction {
    /// A selection was active → cancel it (first `Esc`).
    CancelSelection,
    /// No selection → clear the caret's current line (first `Esc`).
    ClearCurrentLine,
    /// Second consecutive `Esc` on a multi-line buffer → clear the whole buffer.
    ClearWholeBuffer,
    /// Second consecutive `Esc` with nothing left to clear (single-line buffer).
    None,
}

/// Decide what a single `Esc` press does (FR-029). The first `Esc` cancels an
/// active selection, else clears the caret's current line; a second consecutive
/// `Esc` (`esc_pending`) clears the whole buffer when it spans multiple lines.
pub fn esc_action(esc_pending: bool, has_selection: bool, multiline: bool) -> EscAction {
    if esc_pending {
        if multiline {
            EscAction::ClearWholeBuffer
        } else {
            EscAction::None
        }
    } else if has_selection {
        EscAction::CancelSelection
    } else {
        EscAction::ClearCurrentLine
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_is_normalized() {
        let mut s = InputSelection::new(5);
        s.caret = 2;
        assert_eq!(s.range(), (2, 5));
        s.caret = 9;
        assert_eq!(s.range(), (5, 9));
    }

    #[test]
    fn fresh_selection_is_empty() {
        assert!(InputSelection::new(3).is_empty());
    }

    #[test]
    fn active_selection_is_structurally_exclusive() {
        let none = ActiveSelection::None;
        assert!(none.is_none() && !none.is_active());
        assert_eq!(none.input(), None);

        let input = ActiveSelection::Input(InputSelection::new(2));
        assert!(input.is_active());
        assert_eq!(input.input(), Some(InputSelection::new(2)));
        assert_eq!(input.transcript(), None);

        let transcript = ActiveSelection::Transcript((1, 0), (1, 4));
        assert!(transcript.is_active());
        assert_eq!(transcript.transcript(), Some(((1, 0), (1, 4))));
        assert_eq!(transcript.input(), None);
    }

    #[test]
    fn esc_action_follows_fr029_precedence() {
        // First Esc: cancel an active selection, else clear the current line.
        assert_eq!(esc_action(false, true, false), EscAction::CancelSelection);
        assert_eq!(esc_action(false, false, false), EscAction::ClearCurrentLine);
        assert_eq!(esc_action(false, false, true), EscAction::ClearCurrentLine);
        // Second consecutive Esc: clear the whole multi-line buffer; otherwise
        // nothing further (the message clears independently).
        assert_eq!(esc_action(true, false, true), EscAction::ClearWholeBuffer);
        assert_eq!(esc_action(true, false, false), EscAction::None);
    }
}
