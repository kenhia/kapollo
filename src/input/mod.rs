//! The input pad: the editable bottom region of the UI, plus kapollo's own
//! input history. Supports multiline editing (Enter submits the whole buffer;
//! Shift+Enter / Alt+Enter insert a newline) and Up/Down history recall
//! (FR-010, FR-011, FR-012, FR-013).

mod editing;
pub mod router;
pub mod selection;

pub use selection::InputSelection;

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

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Number of visual lines in the buffer (at least one).
    pub fn line_count(&self) -> usize {
        self.buffer.split('\n').count()
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
