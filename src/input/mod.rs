//! The input pad: the editable bottom region of the UI, plus kapollo's own
//! input history. Supports multiline editing (Enter submits the whole buffer;
//! Shift+Enter / Alt+Enter insert a newline) and Up/Down history recall
//! (FR-010, FR-011, FR-012, FR-013).

pub mod router;

/// The editable input buffer with a character cursor.
#[derive(Debug, Default)]
pub struct InputPad {
    buffer: String,
    /// Cursor position as a count of characters from the start of the buffer.
    cursor: usize,
}

impl InputPad {
    /// Create an empty input pad.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a character at the cursor and advance.
    pub fn insert_char(&mut self, c: char) {
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
        if self.cursor == 0 {
            return;
        }
        let start = self.byte_index(self.cursor - 1);
        self.buffer.remove(start);
        self.cursor -= 1;
    }

    /// Move the cursor one character left.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move the cursor one character right.
    pub fn move_right(&mut self) {
        if self.cursor < self.char_count() {
            self.cursor += 1;
        }
    }

    /// Clear the buffer and reset the cursor.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
    }

    /// Replace the buffer contents (e.g. on history recall), placing the cursor
    /// at the end.
    pub fn set_contents(&mut self, text: impl Into<String>) {
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
        let mut row = 0;
        let mut col = 0;
        for c in self.buffer.chars().take(self.cursor) {
            if c == '\n' {
                row += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (row, col)
    }

    /// Take the buffer contents for submission, leaving the pad empty.
    pub fn take_submit(&mut self) -> String {
        self.cursor = 0;
        std::mem::take(&mut self.buffer)
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
