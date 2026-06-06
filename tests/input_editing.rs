//! Input-line editing tests (sprint 005, US1; FR-001/002/005/006/007).
//! Exercises `InputPad` line/word motion and kill operations on single- and
//! multi-line buffers via the public API. Cursor position is asserted
//! behaviorally: after a motion, an inserted marker shows where the caret sits.

use kapollo::input::InputPad;

/// Build a pad holding `text` with the caret `cursor` chars from the start.
fn pad(text: &str, cursor: usize) -> InputPad {
    let mut p = InputPad::new();
    p.set_contents(text);
    let len = text.chars().count();
    for _ in cursor..len {
        p.move_left();
    }
    p
}

#[test]
fn line_move_start_and_end_single_line() {
    let mut p = pad("hello world", 11);
    p.line_move_start();
    p.insert_char('|');
    assert_eq!(p.as_str(), "|hello world");

    let mut p = pad("hello world", 0);
    p.line_move_end();
    p.insert_char('|');
    assert_eq!(p.as_str(), "hello world|");
}

#[test]
fn word_move_left_is_punctuation_aware() {
    let mut p = pad("foo.bar baz", 11);
    p.word_move_left();
    p.insert_char('|');
    assert_eq!(p.as_str(), "foo.bar |baz");
}

#[test]
fn word_move_right_is_punctuation_aware() {
    let mut p = pad("foo.bar baz", 0);
    p.word_move_right();
    p.insert_char('|');
    assert_eq!(p.as_str(), "foo|.bar baz");
}

#[test]
fn kill_to_line_end_removes_rest_of_line() {
    let mut p = pad("hello world", 5);
    p.kill_to_line_end();
    assert_eq!(p.as_str(), "hello");
}

#[test]
fn kill_to_line_start_removes_to_line_start() {
    let mut p = pad("hello world", 6);
    p.kill_to_line_start();
    assert_eq!(p.as_str(), "world");
}

#[test]
fn delete_word_before_uses_whitespace_rule() {
    let mut p = pad("ls -la", 6);
    p.delete_word_before();
    assert_eq!(p.as_str(), "ls ");
    p.delete_word_before();
    assert_eq!(p.as_str(), "");
}

#[test]
fn operations_stay_within_the_current_line() {
    // line_move_start lands at the start of the *current* line, not the buffer.
    let mut p = pad("ab\ncd", 5);
    p.line_move_start();
    p.insert_char('|');
    assert_eq!(p.as_str(), "ab\n|cd");

    // delete_word_before never crosses the preceding newline.
    let mut p = pad("ab\ncd", 5);
    p.delete_word_before();
    assert_eq!(p.as_str(), "ab\n");
}
