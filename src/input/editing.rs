//! Pure line-editing helpers for the input pad: current-line bounds and the two
//! word-boundary scanners (punctuation-aware motion vs. the readline whitespace
//! rule). These operate on a slice of `char`s so they are unit-testable in
//! isolation and reused by `InputPad` (sprint 005, US1; FR-002/006/007).
//!
//! See `specs/005-input-and-status/contracts/input-editing.md`.

/// Character classes used by punctuation-aware word motion (FR-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Whitespace,
    /// Alphanumeric or `_` — the "word" class.
    Word,
    /// Anything that is neither whitespace nor a word char.
    Punctuation,
}

fn class_of(c: char) -> CharClass {
    if c.is_whitespace() {
        CharClass::Whitespace
    } else if c.is_alphanumeric() || c == '_' {
        CharClass::Word
    } else {
        CharClass::Punctuation
    }
}

/// The `[start, end)` char bounds of the line `cursor` sits on, within `chars`
/// (a `\n`-delimited buffer). `start` is just after the preceding `\n` (or 0);
/// `end` is just before the next `\n` (or the buffer length). (FR-007)
pub(crate) fn current_line_bounds(chars: &[char], cursor: usize) -> (usize, usize) {
    let cursor = cursor.min(chars.len());
    let mut start = 0;
    for i in (0..cursor).rev() {
        if chars[i] == '\n' {
            start = i + 1;
            break;
        }
    }
    let mut end = chars.len();
    for (i, &c) in chars.iter().enumerate().skip(cursor) {
        if c == '\n' {
            end = i;
            break;
        }
    }
    (start, end)
}

/// Punctuation-aware word motion to the left: skip any whitespace immediately to
/// the left, then the contiguous run of the same class as the char now on the
/// left. Returns the new index within `line` (`0..=from`). (FR-002)
pub(crate) fn word_boundary_left(line: &[char], from: usize) -> usize {
    let mut i = from.min(line.len());
    while i > 0 && line[i - 1].is_whitespace() {
        i -= 1;
    }
    if i == 0 {
        return 0;
    }
    let class = class_of(line[i - 1]);
    while i > 0 && class_of(line[i - 1]) == class {
        i -= 1;
    }
    i
}

/// Punctuation-aware word motion to the right: skip any whitespace immediately to
/// the right, then the contiguous run of the same class as the char now on the
/// right. Returns the new index within `line` (`from..=line.len()`). (FR-002)
pub(crate) fn word_boundary_right(line: &[char], from: usize) -> usize {
    let n = line.len();
    let mut i = from.min(n);
    while i < n && line[i].is_whitespace() {
        i += 1;
    }
    if i == n {
        return n;
    }
    let class = class_of(line[i]);
    while i < n && class_of(line[i]) == class {
        i += 1;
    }
    i
}

/// Readline whitespace-rule word-rubout (`Ctrl+W`): consume any whitespace
/// immediately before `from`, then the preceding non-whitespace run (punctuation
/// included). Returns the start index of the span to delete (end is `from`).
/// (FR-006)
pub(crate) fn delete_word_before_start(line: &[char], from: usize) -> usize {
    let mut i = from.min(line.len());
    while i > 0 && line[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !line[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn line_bounds_single_line() {
        let c = chars("hello");
        assert_eq!(current_line_bounds(&c, 3), (0, 5));
    }

    #[test]
    fn line_bounds_multiline_interior() {
        let c = chars("ab\ncde\nf");
        // cursor on the middle line (index 4 -> 'd')
        assert_eq!(current_line_bounds(&c, 4), (3, 6));
        // cursor at the very start of the middle line
        assert_eq!(current_line_bounds(&c, 3), (3, 6));
        // cursor on the last line
        assert_eq!(current_line_bounds(&c, 7), (7, 8));
    }

    #[test]
    fn word_left_stops_at_punctuation() {
        let c = chars("foo.bar");
        assert_eq!(word_boundary_left(&c, 7), 4); // start of "bar"
        assert_eq!(word_boundary_left(&c, 4), 3); // the "." run
        assert_eq!(word_boundary_left(&c, 3), 0); // start of "foo"
        assert_eq!(word_boundary_left(&c, 0), 0); // clamped
    }

    #[test]
    fn word_left_skips_leading_whitespace() {
        // "ls  -la": from the end, the word run "la" stops at the '-' boundary.
        let c = chars("ls  -la");
        assert_eq!(word_boundary_left(&c, 7), 5); // start of "la"
        assert_eq!(word_boundary_left(&c, 5), 4); // the "-" run
        assert_eq!(word_boundary_left(&c, 4), 0); // skip "  " then "ls"
    }

    #[test]
    fn word_right_stops_at_punctuation() {
        let c = chars("foo.bar");
        assert_eq!(word_boundary_right(&c, 0), 3); // end of "foo"
        assert_eq!(word_boundary_right(&c, 3), 4); // the "." run
        assert_eq!(word_boundary_right(&c, 4), 7); // end of "bar"
        assert_eq!(word_boundary_right(&c, 7), 7); // clamped
    }

    #[test]
    fn delete_word_before_consumes_ws_then_run() {
        let c = chars("ls -la ");
        // trailing space, then "-la" run
        assert_eq!(delete_word_before_start(&c, 7), 3);
        // from there: "ls " has a space then "ls"
        assert_eq!(delete_word_before_start(&c, 3), 0);
    }

    #[test]
    fn delete_word_before_at_start_is_noop() {
        let c = chars("abc");
        assert_eq!(delete_word_before_start(&c, 0), 0);
    }
}
