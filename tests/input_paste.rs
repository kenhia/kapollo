//! Input paste tests (sprint 005, US2; FR-010/011/012). Pasted text is inserted
//! as a single unit at the caret with line endings normalized to `\n`, never
//! auto-submitting, with the caret landing at the end of the inserted text.

use kapollo::input::InputPad;

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
fn paste_inserts_text_and_leaves_caret_at_end() {
    let mut p = InputPad::new();
    p.insert_paste("abc");
    assert_eq!(p.as_str(), "abc");
    p.insert_char('|');
    assert_eq!(p.as_str(), "abc|");
}

#[test]
fn paste_normalizes_crlf_and_cr() {
    let mut p = InputPad::new();
    p.insert_paste("a\r\nb\rc");
    assert_eq!(p.as_str(), "a\nb\nc");
}

#[test]
fn paste_does_not_submit_on_trailing_newline() {
    let mut p = InputPad::new();
    p.insert_paste("ls -la\n");
    assert_eq!(p.as_str(), "ls -la\n");
}

#[test]
fn paste_splices_at_the_cursor() {
    let mut p = pad("XY", 1);
    p.insert_paste("AB");
    p.insert_char('|');
    assert_eq!(p.as_str(), "XAB|Y");
}

#[test]
fn empty_paste_is_a_noop() {
    let mut p = pad("hi", 1);
    p.insert_paste("");
    p.insert_char('|');
    assert_eq!(p.as_str(), "h|i");
}
