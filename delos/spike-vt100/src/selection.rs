//! S1 selection state machine over **content coordinates**, delegating range math to
//! `spike_support::coords`. Kept pure (no I/O) so the core transitions stay testable.
//!
//! Content coordinates: `(content_row, col)` where `content_row` is stable under
//! scrolling (see `main.rs` for the `top_row = BASE - vt100_scrollback` mapping).

use spike_support::coords::{normalize, Cell};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelState {
    Idle,
    Dragging,
    Active,
}

/// Outcome of a left mouse press, telling the caller how to react.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftPress {
    /// A new drag began (anchor set); caller extends it on subsequent moves.
    StartedDrag,
    /// An Active selection was cancelled (FR-018); nothing copied.
    Cancelled,
    /// Shift was held — forward the event to the child (FR-017).
    ForwardToChild,
}

/// Outcome of a copy-trigger (right-press or `Ctrl-C`) given the current state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// No active selection + `Ctrl-C` → SIGINT to child (FR-015).
    Sigint,
    /// No active selection + right-press → open context menu (FR-019).
    ContextMenu,
    /// Active selection → copy this normalized range, then deselect (FR-016).
    Copy(Cell, Cell),
}

/// Drives the Idle → Dragging → Active machine from the data model.
#[derive(Debug)]
pub struct SelectionController {
    state: SelState,
    anchor: Cell,
    end: Cell,
}

impl Default for SelectionController {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionController {
    pub fn new() -> Self {
        Self {
            state: SelState::Idle,
            anchor: (0, 0),
            end: (0, 0),
        }
    }

    #[allow(dead_code)]
    pub fn state(&self) -> SelState {
        self.state
    }

    pub fn is_active(&self) -> bool {
        self.state == SelState::Active
    }

    /// Handle a left press. Shift forwards to the child; a press on an Active
    /// selection cancels it (FR-018); otherwise a new drag begins.
    pub fn left_press(&mut self, cell: Cell, shift: bool) -> LeftPress {
        if shift {
            return LeftPress::ForwardToChild;
        }
        if self.state == SelState::Active {
            self.state = SelState::Idle;
            return LeftPress::Cancelled;
        }
        self.anchor = cell;
        self.end = cell;
        self.state = SelState::Dragging;
        LeftPress::StartedDrag
    }

    /// Extend the active end while dragging (no-op in other states).
    pub fn drag_to(&mut self, cell: Cell) {
        if self.state == SelState::Dragging {
            self.end = cell;
        }
    }

    /// Mouse release finalizes an in-progress drag into an Active selection
    /// (FR-011) — it does **not** copy.
    pub fn release(&mut self) {
        if self.state == SelState::Dragging {
            self.state = SelState::Active;
        }
    }

    /// ESC cancels an in-progress or finalized selection (FR-018). Returns whether
    /// anything was cancelled.
    pub fn cancel(&mut self) -> bool {
        if matches!(self.state, SelState::Dragging | SelState::Active) {
            self.state = SelState::Idle;
            true
        } else {
            false
        }
    }

    /// `Ctrl-C`: copy+deselect when Active (FR-016), else SIGINT (FR-015).
    pub fn ctrl_c(&mut self) -> Trigger {
        if self.is_active() {
            let (a, b) = self.normalized();
            self.state = SelState::Idle;
            Trigger::Copy(a, b)
        } else {
            Trigger::Sigint
        }
    }

    /// Right-press: copy+deselect when Active (FR-016), else open menu (FR-019).
    pub fn right_press(&mut self) -> Trigger {
        if self.is_active() {
            let (a, b) = self.normalized();
            self.state = SelState::Idle;
            Trigger::Copy(a, b)
        } else {
            Trigger::ContextMenu
        }
    }

    /// Current normalized range while Dragging or Active, for the highlight overlay.
    pub fn range(&self) -> Option<(Cell, Cell)> {
        match self.state {
            SelState::Idle => None,
            _ => Some(self.normalized()),
        }
    }

    fn normalized(&self) -> (Cell, Cell) {
        normalize(self.anchor, self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_press_starts_drag_and_release_activates() {
        let mut s = SelectionController::new();
        assert_eq!(s.left_press((10, 2), false), LeftPress::StartedDrag);
        assert_eq!(s.state(), SelState::Dragging);
        s.drag_to((12, 5));
        s.release();
        assert_eq!(s.state(), SelState::Active);
        assert_eq!(s.range(), Some(((10, 2), (12, 5))));
    }

    #[test]
    fn release_does_not_copy() {
        let mut s = SelectionController::new();
        s.left_press((1, 1), false);
        s.release();
        // Still Active and highlighted; copy only happens on an explicit trigger.
        assert!(s.is_active());
        assert!(s.range().is_some());
    }

    #[test]
    fn second_left_press_cancels_active() {
        let mut s = SelectionController::new();
        s.left_press((1, 1), false);
        s.release();
        assert_eq!(s.left_press((4, 4), false), LeftPress::Cancelled);
        assert_eq!(s.state(), SelState::Idle);
        assert_eq!(s.range(), None);
    }

    #[test]
    fn shift_forwards_to_child() {
        let mut s = SelectionController::new();
        assert_eq!(s.left_press((0, 0), true), LeftPress::ForwardToChild);
        assert_eq!(s.state(), SelState::Idle);
    }

    #[test]
    fn ctrl_c_sigint_when_idle_copy_when_active() {
        let mut s = SelectionController::new();
        assert_eq!(s.ctrl_c(), Trigger::Sigint);
        s.left_press((2, 0), false);
        s.drag_to((2, 9));
        s.release();
        assert_eq!(s.ctrl_c(), Trigger::Copy((2, 0), (2, 9)));
        assert_eq!(s.state(), SelState::Idle); // deselected after copy
    }

    #[test]
    fn right_press_menu_when_idle_copy_when_active() {
        let mut s = SelectionController::new();
        assert_eq!(s.right_press(), Trigger::ContextMenu);
        s.left_press((5, 1), false);
        s.release();
        assert_eq!(s.right_press(), Trigger::Copy((5, 1), (5, 1)));
        assert_eq!(s.state(), SelState::Idle);
    }

    #[test]
    fn drag_with_reversed_endpoints_normalizes() {
        let mut s = SelectionController::new();
        s.left_press((9, 3), false);
        s.drag_to((4, 7));
        assert_eq!(s.range(), Some(((4, 7), (9, 3))));
    }

    #[test]
    fn esc_cancels() {
        let mut s = SelectionController::new();
        s.left_press((1, 1), false);
        s.release();
        assert!(s.cancel());
        assert_eq!(s.state(), SelState::Idle);
        assert!(!s.cancel());
    }
}
