//! Selection state and routing for the grid rework (sprint 004).
//!
//! The [`SelectionController`] FSM (idle→dragging→active) is anchored to content
//! rows — `StableRowIndex` cast to `usize` — so a selection does not drift as new
//! output scrolls underneath it (FR-007/008, R6). [`extract_text`] slices the
//! selected cells out of a viewport cell grid with no off-by-one (SC-004). The
//! pure coordinate helpers live in [`coords`].

pub mod coords;

use coords::{content_to_screen, normalize, Cell};

/// FSM states for a click-drag-release selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelState {
    /// No selection in progress.
    Idle,
    /// A drag is in progress; the end follows the pointer.
    Dragging,
    /// A finalized selection awaiting copy or clear.
    Active,
}

/// Outcome of a left mouse press, telling the caller how to react.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftPress {
    /// A new drag began (anchor set); the caller extends it on subsequent moves.
    StartedDrag,
    /// An `Active` selection was cancelled (a click clears it); nothing copied.
    Cancelled,
    /// Shift was held — forward the event to the child / host (FR-016).
    ForwardToChild,
}

/// Outcome of a copy-trigger (right-press or `Ctrl-C`) given the current state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// No active selection + `Ctrl-C` → SIGINT to the child (FR-024).
    Sigint,
    /// No active selection + right-press → open the context menu (future).
    ContextMenu,
    /// Active selection → copy this normalized range, then deselect (FR-011).
    Copy(Cell, Cell),
}

/// Drives the Idle → Dragging → Active machine over content-row anchors. The
/// model is pure: the caller maps screen pixels to content cells via [`coords`]
/// and feeds them in, so the FSM never sees the viewport or scroll offset (R6).
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
    /// Create an idle controller.
    pub fn new() -> Self {
        Self {
            state: SelState::Idle,
            anchor: (0, 0),
            end: (0, 0),
        }
    }

    /// Current FSM state.
    pub fn state(&self) -> SelState {
        self.state
    }

    /// Whether a finalized selection exists.
    pub fn is_active(&self) -> bool {
        self.state == SelState::Active
    }

    /// Handle a left press. Shift forwards to the child/host; a press on an
    /// `Active` selection clears it; otherwise a new drag begins (FR-007/016).
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

    /// Extend the active end while dragging (no-op in other states) (FR-007).
    pub fn drag_to(&mut self, cell: Cell) {
        if self.state == SelState::Dragging {
            self.end = cell;
        }
    }

    /// Mouse release finalizes an in-progress drag into an `Active` selection
    /// (FR-008) — it does **not** copy.
    pub fn release(&mut self) {
        if self.state == SelState::Dragging {
            self.state = SelState::Active;
        }
    }

    /// ESC cancels an in-progress or finalized selection. Returns whether
    /// anything was cancelled.
    pub fn cancel(&mut self) -> bool {
        if matches!(self.state, SelState::Dragging | SelState::Active) {
            self.state = SelState::Idle;
            true
        } else {
            false
        }
    }

    /// Clear any selection when a command is submitted (FR-017). This resolves
    /// the 003 spike's flood-overrun caveat: a stale highlight never rides along
    /// as the next command floods output.
    pub fn on_command_submit(&mut self) {
        self.state = SelState::Idle;
    }

    /// `Ctrl-C`: copy+deselect when `Active` (FR-011), else SIGINT (FR-024).
    pub fn ctrl_c(&mut self) -> Trigger {
        if self.is_active() {
            let (a, b) = self.normalized();
            self.state = SelState::Idle;
            Trigger::Copy(a, b)
        } else {
            Trigger::Sigint
        }
    }

    /// Right-press: copy+deselect when `Active` (FR-011), else open the menu.
    pub fn right_press(&mut self) -> Trigger {
        if self.is_active() {
            let (a, b) = self.normalized();
            self.state = SelState::Idle;
            Trigger::Copy(a, b)
        } else {
            Trigger::ContextMenu
        }
    }

    /// Normalized range while `Dragging` or `Active`, for the highlight overlay.
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

/// Slice the selected text out of a viewport cell grid. `rows` is the visible
/// viewport as a row-major grid of single-cell strings (wide-cell continuation
/// columns are empty strings); `top_row` is the content row of `rows[0]`. The
/// anchors `a`/`b` are normalized content cells. Each output line is
/// right-trimmed and the lines joined with `\n`, with no off-by-one (SC-004).
pub fn extract_text(rows: &[Vec<String>], top_row: usize, a: Cell, b: Cell) -> String {
    let height = rows.len();
    if height == 0 {
        return String::new();
    }
    let sr = content_to_screen(top_row, a.0, height).unwrap_or(0);
    let er = content_to_screen(top_row, b.0, height).unwrap_or(height - 1);

    let mut out: Vec<String> = Vec::new();
    for (sy, row) in rows.iter().enumerate().take(er + 1).skip(sr) {
        let width = row.len();
        if width == 0 {
            out.push(String::new());
            continue;
        }
        let last = width - 1;
        let c0 = if sy == sr { a.1.min(last) } else { 0 };
        let c1 = if sy == er { b.1.min(last) } else { last };
        let text: String = row[c0..=c1].iter().map(String::as_str).collect();
        out.push(text.trim_end().to_string());
    }
    out.join("\n")
}

/// Per-row inclusive highlight column span for a selection over a viewport.
/// Returns one entry per visible row: `Some((c0, c1))` for rows the selection
/// covers, `None` otherwise. Used to overlay the highlight (FR-007).
pub fn highlight_spans(
    height: usize,
    cols: u16,
    top_row: usize,
    a: Cell,
    b: Cell,
) -> Vec<Option<(usize, usize)>> {
    let last = cols.saturating_sub(1) as usize;
    let sr = content_to_screen(top_row, a.0, height);
    let er = content_to_screen(top_row, b.0, height);
    // The whole selection lies off one edge of the viewport: nothing to draw.
    let below = a.0 >= top_row + height;
    let above = b.0 < top_row;
    if (sr.is_none() && er.is_none()) && (below || above) {
        return vec![None; height];
    }
    let s = sr.unwrap_or(0);
    let e = er.unwrap_or(height.saturating_sub(1));
    (0..height)
        .map(|y| {
            if y < s || y > e {
                return None;
            }
            let c0 = if y == s && sr.is_some() {
                a.1.min(last)
            } else {
                0
            };
            let c1 = if y == e && er.is_some() {
                b.1.min(last)
            } else {
                last
            };
            Some((c0, c1))
        })
        .collect()
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
    fn second_click_clears_active_selection() {
        let mut s = SelectionController::new();
        s.left_press((1, 1), false);
        s.release();
        assert!(s.is_active());
        assert_eq!(s.left_press((1, 1), false), LeftPress::Cancelled);
        assert_eq!(s.range(), None);
    }

    #[test]
    fn on_command_submit_clears() {
        let mut s = SelectionController::new();
        s.left_press((2, 0), false);
        s.drag_to((2, 9));
        s.release();
        assert!(s.is_active());
        s.on_command_submit();
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
    fn reversed_drag_normalizes() {
        let mut s = SelectionController::new();
        s.left_press((9, 3), false);
        s.drag_to((4, 7));
        assert_eq!(s.range(), Some(((4, 7), (9, 3))));
    }

    #[test]
    fn extract_text_single_row_is_char_exact() {
        let rows = vec![vec![
            "h".into(),
            "e".into(),
            "l".into(),
            "l".into(),
            "o".into(),
        ]];
        let text = extract_text(&rows, 100, (100, 1), (100, 3));
        assert_eq!(text, "ell");
    }

    #[test]
    fn extract_text_multi_row_join_has_no_off_by_one() {
        let rows = vec![
            vec!["a".into(), "b".into(), "c".into()],
            vec!["d".into(), "e".into(), "f".into()],
            vec!["g".into(), "h".into(), "i".into()],
        ];
        let text = extract_text(&rows, 0, (0, 1), (2, 1));
        assert_eq!(text, "bc\ndef\ngh");
    }
}
