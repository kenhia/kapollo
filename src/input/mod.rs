//! The input pad: the editable bottom region of the UI, plus kapollo's own
//! input history. Supports multiline editing (Enter submits the whole buffer;
//! Shift+Enter / Alt+Enter insert a newline) and Up/Down history recall
//! (FR-010, FR-011, FR-012, FR-013).

mod editing;
pub mod router;
pub mod selection;

use std::collections::BTreeSet;

pub use selection::InputSelection;

/// The editing mode of the input buffer, surfaced in the status bar's reserved
/// 4-column mode field (sprint 007; see
/// `specs/007-laat-mode/contracts/input-modes.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    /// Single-line / default editing; `Up`/`Down` recall history.
    #[default]
    Norm,
    /// Multi-line editing; `Up`/`Down` move the caret, with chat-style edge recall.
    Mult,
    /// `Mult` + highlight + step + exit-code gating (Line-At-A-Time).
    Laat,
}

impl InputMode {
    /// The 4-column status-bar label for this mode (`norm` / `Mult` / `1T`).
    pub fn label(self) -> &'static str {
        match self {
            InputMode::Norm => "norm",
            InputMode::Mult => "Mult",
            InputMode::Laat => "1T",
        }
    }

    /// The `ToggleMultLaat` transition (`Ctrl+1`) given whether the buffer is
    /// multi-line: `Norm → Mult` (even empty/single line, FR-015); `Mult ↔ Laat`
    /// only when multi-line (FR-016); a single-line `Mult` stays `Mult` (LAAT
    /// needs multiple lines to step through).
    pub fn toggled_mult_laat(self, multiline: bool) -> InputMode {
        match self {
            InputMode::Norm => InputMode::Mult,
            InputMode::Mult if multiline => InputMode::Laat,
            InputMode::Laat => InputMode::Mult,
            other => other,
        }
    }
}

/// The advance/flag outcome of applying an exit code to a pending LAAT line
/// (sprint 007; `specs/007-laat-mode/contracts/laat-engine.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaatOutcome {
    /// Exit `0` (or no reported code): advance the highlight, clear the flag.
    Advance,
    /// Non-zero exit: flag the line as a *probable* failure, keep the highlight.
    Flag,
}

/// The LAAT stepping state — present only while `mode == Laat`. A highlight
/// steps line-by-line; `Enter` submits the highlighted line and arms `pending`;
/// the next `CommandEnd` exit code advances or flags it (sprint 007).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaatState {
    /// The currently highlighted buffer line (0-based).
    pub highlight: usize,
    /// Lines flagged as *probable* failures.
    pub failed_lines: BTreeSet<usize>,
    /// The line last submitted, awaiting a `CommandEnd` boundary.
    pub pending: Option<usize>,
}

impl LaatState {
    /// A fresh LAAT state: highlight on line 0, no flags, nothing pending.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `line` has been submitted and is awaiting completion.
    pub fn submit_line(&mut self, line: usize) {
        self.pending = Some(line);
    }

    /// Apply a `CommandEnd` exit code to the pending line (pure gating, FR-004).
    /// Exit `0` — or a missing code (no failure reported) — advances the
    /// highlight past the line and clears its flag; a non-zero exit flags the
    /// line as a probable failure and keeps the highlight. Returns `None` when
    /// nothing was pending.
    pub fn apply_exit_code(&mut self, exit: Option<i32>) -> Option<LaatOutcome> {
        let line = self.pending.take()?;
        if matches!(exit, Some(0) | None) {
            self.failed_lines.remove(&line);
            self.highlight = line + 1;
            Some(LaatOutcome::Advance)
        } else {
            self.failed_lines.insert(line);
            self.highlight = line;
            Some(LaatOutcome::Flag)
        }
    }

    /// Whether `line` is flagged as a probable failure (for rendering).
    pub fn is_failed(&self, line: usize) -> bool {
        self.failed_lines.contains(&line)
    }
}

/// A one-item snapshot of the composing input, saved by `PushInput` and
/// restored on the next submit (sprint 007 push/pop, FR-018…FR-020). Captures
/// everything needed to resume composition exactly: the buffer, caret, mode, the
/// chat-style stashed draft (FR-011), and the LAAT stepping state when in `Laat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputSnapshot {
    pub buffer: String,
    pub cursor: usize,
    pub mode: InputMode,
    pub stash: Option<String>,
    pub laat: Option<LaatState>,
}

impl InputSnapshot {
    /// Capture the composing input from its constituent pieces.
    pub fn capture(
        input: &InputPad,
        mode: InputMode,
        stash: Option<String>,
        laat: Option<LaatState>,
    ) -> Self {
        Self {
            buffer: input.as_str().to_string(),
            cursor: input.cursor(),
            mode,
            stash,
            laat,
        }
    }

    /// Restore the snapshot's buffer and caret into `input`, returning the saved
    /// `(mode, stash, laat)` for the caller to reinstate on its own state.
    pub fn restore(self, input: &mut InputPad) -> (InputMode, Option<String>, Option<LaatState>) {
        input.restore(self.buffer, self.cursor);
        (self.mode, self.stash, self.laat)
    }
}

/// The editable input buffer with a character cursor.
#[derive(Debug, Default)]
pub struct InputPad {
    buffer: String,
    /// Cursor position as a count of characters from the start of the buffer.
    cursor: usize,
    /// The active selection, if any (sprint 005, US1). Plain motion and edits
    /// collapse it; Shift-motion creates/extends it.
    selection: Option<InputSelection>,
}

impl InputPad {
    /// Create an empty input pad.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a character at the cursor and advance.
    pub fn insert_char(&mut self, c: char) {
        self.selection = None;
        let idx = self.byte_index(self.cursor);
        self.buffer.insert(idx, c);
        self.cursor += 1;
    }

    /// Insert a literal newline at the cursor without submitting (Shift+Enter /
    /// Alt+Enter).
    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        self.selection = None;
        if self.cursor == 0 {
            return;
        }
        let start = self.byte_index(self.cursor - 1);
        self.buffer.remove(start);
        self.cursor -= 1;
    }

    /// Move the cursor one character left.
    pub fn move_left(&mut self) {
        self.selection = None;
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move the cursor one character right.
    pub fn move_right(&mut self) {
        self.selection = None;
        if self.cursor < self.char_count() {
            self.cursor += 1;
        }
    }

    /// Whether the caret sits on the first buffer line (sprint 007).
    pub fn caret_on_first_line(&self) -> bool {
        self.cursor_row_col().0 == 0
    }

    /// Whether the caret sits on the last buffer line (sprint 007).
    pub fn caret_on_last_line(&self) -> bool {
        self.cursor_row_col().0 + 1 >= self.line_count()
    }

    /// Move the caret up one buffer line, preserving the visual column where the
    /// target line is long enough and clamping to its end otherwise. A no-op on
    /// the first line. Collapses any selection (sprint 007;
    /// `specs/007-laat-mode/contracts/input-modes.md` §2).
    pub fn caret_line_up(&mut self) {
        self.selection = None;
        let (row, col) = self.cursor_row_col();
        if row == 0 {
            return;
        }
        self.cursor = self.offset_at_line_col(row - 1, col);
    }

    /// Move the caret down one buffer line, preserving the visual column (clamped
    /// to the target line's end). A no-op on the last line. Collapses any
    /// selection (sprint 007).
    pub fn caret_line_down(&mut self) {
        self.selection = None;
        let (row, col) = self.cursor_row_col();
        if row + 1 >= self.line_count() {
            return;
        }
        self.cursor = self.offset_at_line_col(row + 1, col);
    }

    /// Move the caret to the start of buffer line `row`, clamping a row beyond
    /// the end to the buffer's end (sprint 007 LAAT highlight follow).
    pub fn set_caret_line_start(&mut self, row: usize) {
        self.selection = None;
        self.cursor = self.offset_at_line_col(row, 0);
    }

    /// Char offset of `(line, col)`, clamping `col` to that line's length and a
    /// line beyond the end to the buffer's end.
    fn offset_at_line_col(&self, line: usize, col: usize) -> usize {
        let mut offset = 0;
        for (i, l) in self.buffer.split('\n').enumerate() {
            let llen = l.chars().count();
            if i == line {
                return offset + col.min(llen);
            }
            offset += llen + 1; // +1 for the '\n' separator
        }
        self.char_count()
    }

    /// Clear the buffer and reset the cursor.
    pub fn clear(&mut self) {
        self.selection = None;
        self.buffer.clear();
        self.cursor = 0;
    }

    /// Replace the buffer contents (e.g. on history recall), placing the cursor
    /// at the end.
    pub fn set_contents(&mut self, text: impl Into<String>) {
        self.selection = None;
        self.buffer = text.into();
        self.cursor = self.char_count();
    }

    /// Borrow the current buffer contents.
    pub fn as_str(&self) -> &str {
        &self.buffer
    }

    /// The cursor position as a count of characters from the buffer start, for
    /// snapshotting (sprint 007 push/pop).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Restore a snapshotted buffer and cursor, clamping the cursor to the
    /// buffer length and clearing any selection (sprint 007 push/pop).
    pub fn restore(&mut self, buffer: impl Into<String>, cursor: usize) {
        self.selection = None;
        self.buffer = buffer.into();
        self.cursor = cursor.min(self.char_count());
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Number of visual lines in the buffer (at least one).
    pub fn line_count(&self) -> usize {
        self.buffer.split('\n').count()
    }

    /// The text of buffer line `row` (0-based), or an empty string when out of
    /// range (sprint 007 LAAT line submission).
    pub fn line_text(&self, row: usize) -> String {
        self.buffer.split('\n').nth(row).unwrap_or("").to_string()
    }

    /// The cursor's `(row, column)` in characters, for rendering.
    pub fn cursor_row_col(&self) -> (usize, usize) {
        self.offset_row_col(self.cursor)
    }

    /// The `(row, column)` in characters of the `n`-th character offset.
    pub fn offset_row_col(&self, n: usize) -> (usize, usize) {
        let mut row = 0;
        let mut col = 0;
        for c in self.buffer.chars().take(n) {
            if c == '\n' {
                row += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (row, col)
    }

    /// The active selection, if any (for rendering / copy).
    pub fn selection(&self) -> Option<InputSelection> {
        self.selection
    }

    /// Cancel any active selection (Esc), leaving the cursor where it is.
    pub fn cancel_selection(&mut self) {
        self.selection = None;
    }

    /// Whether a non-empty selection is active.
    pub fn has_selection(&self) -> bool {
        self.selection.is_some_and(|s| !s.is_empty())
    }

    /// The currently selected text, if any (for copy).
    pub fn selected_text(&self) -> Option<String> {
        let sel = self.selection?;
        if sel.is_empty() {
            return None;
        }
        let (a, b) = sel.range();
        Some(self.buffer.chars().skip(a).take(b - a).collect())
    }

    /// Move the cursor to the start of the current line (Home). (FR-001)
    pub fn line_move_start(&mut self) {
        self.selection = None;
        let (_chars, start, _end, _col) = self.line_context();
        self.cursor = start;
    }

    /// Move the cursor to the end of the current line (End). (FR-001)
    pub fn line_move_end(&mut self) {
        self.selection = None;
        let (_chars, _start, end, _col) = self.line_context();
        self.cursor = end;
    }

    /// Move the cursor one word left within the current line (Ctrl+Left).
    /// (FR-002)
    pub fn word_move_left(&mut self) {
        self.selection = None;
        let (chars, start, end, col) = self.line_context();
        let new_col = editing::word_boundary_left(&chars[start..end], col);
        self.cursor = start + new_col;
    }

    /// Move the cursor one word right within the current line (Ctrl+Right).
    /// (FR-002)
    pub fn word_move_right(&mut self) {
        self.selection = None;
        let (chars, start, end, col) = self.line_context();
        let new_col = editing::word_boundary_right(&chars[start..end], col);
        self.cursor = start + new_col;
    }

    /// Extend the selection one character left (Shift+Left). (FR-003)
    pub fn select_char_left(&mut self) {
        let target = self.cursor.saturating_sub(1);
        self.extend_selection_to(target);
    }

    /// Extend the selection one character right (Shift+Right). (FR-003)
    pub fn select_char_right(&mut self) {
        let target = (self.cursor + 1).min(self.char_count());
        self.extend_selection_to(target);
    }

    /// Extend the selection one word left within the current line
    /// (Shift+Ctrl+Left). (FR-004)
    pub fn select_word_left(&mut self) {
        let (chars, start, end, col) = self.line_context();
        let new_col = editing::word_boundary_left(&chars[start..end], col);
        self.extend_selection_to(start + new_col);
    }

    /// Extend the selection one word right within the current line
    /// (Shift+Ctrl+Right). (FR-004)
    pub fn select_word_right(&mut self) {
        let (chars, start, end, col) = self.line_context();
        let new_col = editing::word_boundary_right(&chars[start..end], col);
        self.extend_selection_to(start + new_col);
    }

    /// Delete from the current-line start to the cursor (Ctrl+U). (FR-005)
    pub fn kill_to_line_start(&mut self) {
        self.selection = None;
        let (_chars, start, _end, _col) = self.line_context();
        self.delete_char_range(start, self.cursor);
        self.cursor = start;
    }

    /// Delete from the cursor to the current-line end (Ctrl+K). (FR-005)
    pub fn kill_to_line_end(&mut self) {
        self.selection = None;
        let (_chars, _start, end, _col) = self.line_context();
        self.delete_char_range(self.cursor, end);
    }

    /// Delete the word before the cursor using the readline whitespace rule
    /// (Ctrl+W). (FR-006)
    pub fn delete_word_before(&mut self) {
        self.selection = None;
        let (chars, start, end, col) = self.line_context();
        let new_col = editing::delete_word_before_start(&chars[start..end], col);
        let from = start + new_col;
        self.delete_char_range(from, self.cursor);
        self.cursor = from;
    }

    /// Clear the characters of the caret's current line, leaving the other lines
    /// and the line structure intact (single `Esc` with no selection; FR-029).
    /// On a single-line buffer this empties the buffer.
    pub fn clear_current_line(&mut self) {
        self.selection = None;
        let (_chars, start, end, _col) = self.line_context();
        self.delete_char_range(start, end);
        self.cursor = start;
    }

    /// Insert pasted text as a single unit at the cursor: line endings are
    /// normalized to `\n`, no submit is triggered, and the caret lands at the end
    /// of the inserted text. Empty pastes are a no-op. (FR-010/011/012)
    pub fn insert_paste(&mut self, text: &str) {
        self.selection = None;
        if text.is_empty() {
            return;
        }
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let idx = self.byte_index(self.cursor);
        self.buffer.insert_str(idx, &normalized);
        self.cursor += normalized.chars().count();
    }

    /// Take the buffer contents for submission, leaving the pad empty. Trailing
    /// whitespace-only lines of a multi-line buffer are stripped (interior
    /// blanks are preserved; a single-line buffer is returned verbatim) so a
    /// stray blank last line from editing or paste does not submit an extra
    /// empty command (kwi #46; default behavior).
    pub fn take_submit(&mut self) -> String {
        self.selection = None;
        self.cursor = 0;
        strip_trailing_blank_lines(std::mem::take(&mut self.buffer))
    }

    /// Buffer chars, current-line `[start, end)` char bounds, and the cursor's
    /// column within that line.
    fn line_context(&self) -> (Vec<char>, usize, usize, usize) {
        let chars: Vec<char> = self.buffer.chars().collect();
        let (start, end) = editing::current_line_bounds(&chars, self.cursor);
        let col = self.cursor - start;
        (chars, start, end, col)
    }

    /// Ensure a selection exists (anchored at the cursor), then move the cursor
    /// to `target` and track it as the selection's caret.
    fn extend_selection_to(&mut self, target: usize) {
        let mut sel = self
            .selection
            .unwrap_or_else(|| InputSelection::new(self.cursor));
        sel.caret = target;
        self.cursor = target;
        self.selection = Some(sel);
    }

    /// Delete the char range `[a, b)` from the buffer.
    fn delete_char_range(&mut self, a: usize, b: usize) {
        if a >= b {
            return;
        }
        let ba = self.byte_index(a);
        let bb = self.byte_index(b);
        self.buffer.replace_range(ba..bb, "");
    }

    fn char_count(&self) -> usize {
        self.buffer.chars().count()
    }

    /// Byte offset of the `n`-th character (or the buffer length).
    fn byte_index(&self, n: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(n)
            .map(|(i, _)| i)
            .unwrap_or(self.buffer.len())
    }
}

/// Strip trailing whitespace-only lines from a multi-line submission, keeping
/// interior blank lines intact (kwi #46; default behavior). A single-line buffer
/// — one with no `\n` — is returned unchanged, including any trailing whitespace,
/// so single-command editing is never altered. At least one line is always kept.
fn strip_trailing_blank_lines(buffer: String) -> String {
    if !buffer.contains('\n') {
        return buffer;
    }
    let mut lines: Vec<&str> = buffer.split('\n').collect();
    while lines.len() > 1 && lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// kapollo's own history of submitted inputs, separate from the wrapped shell's
/// history (D20). In-session only for the MVP.
#[derive(Debug, Default)]
pub struct InputHistory {
    entries: Vec<String>,
    /// Current recall position; `None` means "not recalling" (at the live draft).
    cursor: Option<usize>,
    /// The live draft stashed when chat-style edge recall begins, restored when
    /// stepping newer past the newest entry (FR-010/FR-011). Part of the input
    /// snapshot so it survives a push/pop round-trip (sprint 007).
    stash: Option<String>,
}

impl InputHistory {
    /// Create an empty history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a submitted input and reset the recall cursor (FR-013).
    pub fn push(&mut self, entry: impl Into<String>) {
        let entry = entry.into();
        if !entry.is_empty() {
            self.entries.push(entry);
        }
        self.cursor = None;
    }

    /// Recall the previous (older) entry. Returns `None` when there is no older
    /// entry or the history is empty.
    pub fn recall_older(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let next = match self.cursor {
            None => self.entries.len() - 1,
            Some(0) => 0, // already at the oldest
            Some(i) => i - 1,
        };
        self.cursor = Some(next);
        self.entries.get(next).map(String::as_str)
    }

    /// Recall the next (newer) entry. Returns `Some("")` when moving past the
    /// newest entry back to the live draft, or `None` when not recalling.
    pub fn recall_newer(&mut self) -> Option<&str> {
        match self.cursor {
            None => None,
            Some(i) if i + 1 < self.entries.len() => {
                self.cursor = Some(i + 1);
                self.entries.get(i + 1).map(String::as_str)
            }
            Some(_) => {
                // Past the newest entry: return to an empty draft.
                self.cursor = None;
                Some("")
            }
        }
    }

    /// Chat-style edge recall: starting from the live draft, stash `draft` and
    /// recall the previous (older) entry; continued calls walk older entries
    /// without re-stashing (FR-010; [contracts/input-modes.md] §3). `None` when
    /// the history is empty.
    pub fn edge_recall_older(&mut self, draft: &str) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        if self.cursor.is_none() {
            self.stash = Some(draft.to_string());
        }
        self.recall_older()
    }

    /// Chat-style edge restore: walk newer entries; stepping past the newest
    /// entry restores the stashed draft byte-for-byte and returns to the
    /// live-draft state (FR-011; [contracts/input-modes.md] §3). `Down` never
    /// recalls older entries. `None` when not currently recalling.
    pub fn edge_recall_newer(&mut self) -> Option<String> {
        match self.cursor {
            None => None,
            Some(i) if i + 1 < self.entries.len() => {
                self.cursor = Some(i + 1);
                self.entries.get(i + 1).map(String::from)
            }
            Some(_) => {
                // Past the newest entry: restore the stashed draft.
                self.cursor = None;
                Some(self.stash.take().unwrap_or_default())
            }
        }
    }

    /// The stashed draft, if any — for inclusion in an input snapshot (sprint 007).
    pub fn stash(&self) -> Option<&str> {
        self.stash.as_deref()
    }

    /// Restore a stashed draft (e.g. when popping an input snapshot, sprint 007).
    pub fn set_stash(&mut self, stash: Option<String>) {
        self.stash = stash;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_recall_stashes_draft_and_restores_it() {
        let mut history = InputHistory::new();
        history.push("old");
        // First older step from the live draft stashes it and recalls "old".
        assert_eq!(history.edge_recall_older("draft"), Some("old"));
        assert_eq!(history.stash(), Some("draft"));
        // Stepping newer past the newest entry restores the draft byte-for-byte.
        assert_eq!(history.edge_recall_newer().as_deref(), Some("draft"));
        assert_eq!(history.stash(), None);
    }

    #[test]
    fn edge_recall_older_walks_without_re_stashing() {
        let mut history = InputHistory::new();
        history.push("first");
        history.push("second");
        assert_eq!(history.edge_recall_older("draft"), Some("second"));
        // Continued older steps do not re-stash the (now recalled) buffer.
        assert_eq!(history.edge_recall_older("ignored"), Some("first"));
        assert_eq!(history.stash(), Some("draft"));
    }

    #[test]
    fn edge_recall_older_on_empty_history_is_none() {
        let mut history = InputHistory::new();
        assert_eq!(history.edge_recall_older("draft"), None);
        assert_eq!(history.stash(), None);
    }

    #[test]
    fn edge_recall_newer_when_not_recalling_is_none() {
        let mut history = InputHistory::new();
        history.push("old");
        assert_eq!(history.edge_recall_newer(), None);
    }

    #[test]
    fn newline_does_not_submit_and_grows_lines() {
        let mut pad = InputPad::new();
        pad.insert_char('a');
        pad.insert_newline();
        pad.insert_char('b');
        assert_eq!(pad.as_str(), "a\nb");
        assert_eq!(pad.line_count(), 2);
    }

    #[test]
    fn cursor_insertion_and_backspace() {
        let mut pad = InputPad::new();
        for c in "abc".chars() {
            pad.insert_char(c);
        }
        pad.move_left();
        pad.insert_char('X');
        assert_eq!(pad.as_str(), "abXc");
        pad.backspace();
        assert_eq!(pad.as_str(), "abc");
    }

    #[test]
    fn take_submit_returns_buffer_and_clears() {
        let mut pad = InputPad::new();
        pad.set_contents("line1\nline2");
        assert_eq!(pad.take_submit(), "line1\nline2");
        assert!(pad.is_empty());
    }

    #[test]
    fn history_recall_walks_older_then_newer() {
        let mut history = InputHistory::new();
        history.push("first");
        history.push("second");

        assert_eq!(history.recall_older(), Some("second"));
        assert_eq!(history.recall_older(), Some("first"));
        assert_eq!(history.recall_older(), Some("first")); // clamped at oldest
        assert_eq!(history.recall_newer(), Some("second"));
        assert_eq!(history.recall_newer(), Some("")); // back to draft
        assert_eq!(history.recall_newer(), None);
    }

    #[test]
    fn empty_history_recall_is_none() {
        let mut history = InputHistory::new();
        assert_eq!(history.recall_older(), None);
        assert_eq!(history.recall_newer(), None);
    }

    #[test]
    fn mode_labels_are_the_status_strings() {
        assert_eq!(InputMode::Norm.label(), "norm");
        assert_eq!(InputMode::Mult.label(), "Mult");
        assert_eq!(InputMode::Laat.label(), "1T");
    }

    #[test]
    fn toggle_mult_laat_transitions() {
        // Norm enters Mult even when single-line/empty (FR-015).
        assert_eq!(InputMode::Norm.toggled_mult_laat(false), InputMode::Mult);
        // Mult <-> Laat only when multi-line (FR-016).
        assert_eq!(InputMode::Mult.toggled_mult_laat(true), InputMode::Laat);
        assert_eq!(InputMode::Laat.toggled_mult_laat(true), InputMode::Mult);
        // A single-line Mult stays Mult (LAAT needs multiple lines).
        assert_eq!(InputMode::Mult.toggled_mult_laat(false), InputMode::Mult);
    }

    #[test]
    fn caret_line_up_down_preserve_column_and_clamp() {
        let mut pad = InputPad::new();
        pad.set_contents("abc\nde\nfghi");
        // Cursor at end (line 2, col 4). Up clamps to line 1's length (2).
        assert!(pad.caret_on_last_line());
        pad.caret_line_up();
        assert_eq!(pad.cursor_row_col(), (1, 2));
        // Up again to line 0 keeps the remembered/clamped column (2).
        pad.caret_line_up();
        assert_eq!(pad.cursor_row_col(), (0, 2));
        assert!(pad.caret_on_first_line());
        // Up on the first line is a no-op.
        pad.caret_line_up();
        assert_eq!(pad.cursor_row_col(), (0, 2));
    }

    #[test]
    fn caret_motion_does_not_touch_history_or_buffer() {
        // C4: caret motion changes only the cursor, never the buffer.
        let mut pad = InputPad::new();
        pad.set_contents("a\nb");
        assert!(pad.caret_on_last_line());
        pad.caret_line_up();
        assert_eq!(pad.cursor_row_col(), (0, 1));
        assert_eq!(pad.as_str(), "a\nb");
    }

    #[test]
    fn laat_exit_zero_advances_and_clears_flag() {
        // L2/L5: exit 0 advances the highlight and clears the line's flag.
        let mut laat = LaatState::new();
        assert_eq!(laat.highlight, 0);
        laat.failed_lines.insert(1);
        laat.submit_line(1);
        assert_eq!(laat.apply_exit_code(Some(0)), Some(LaatOutcome::Advance));
        assert_eq!(laat.highlight, 2);
        assert!(!laat.is_failed(1));
        assert_eq!(laat.pending, None);
    }

    #[test]
    fn laat_nonzero_flags_and_keeps_highlight() {
        // L3: a non-zero exit flags the line and keeps the highlight.
        let mut laat = LaatState::new();
        laat.highlight = 1;
        laat.submit_line(1);
        assert_eq!(laat.apply_exit_code(Some(7)), Some(LaatOutcome::Flag));
        assert_eq!(laat.highlight, 1);
        assert!(laat.is_failed(1));
        assert_eq!(laat.pending, None);
    }

    #[test]
    fn laat_apply_with_nothing_pending_is_none() {
        let mut laat = LaatState::new();
        assert_eq!(laat.apply_exit_code(Some(0)), None);
    }
}
