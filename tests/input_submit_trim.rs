//! Trailing whitespace-only line trimming on submit (sprint 005, US-polish;
//! kwi #46, walkthrough item 10). A multi-line submission drops trailing
//! blank/whitespace lines so a stray empty last line does not run an extra
//! command; interior blanks survive and single-line input is never altered.

use kapollo::input::InputPad;

fn submit(text: &str) -> String {
    let mut pad = InputPad::new();
    pad.set_contents(text);
    pad.take_submit()
}

#[test]
fn strips_trailing_blank_lines() {
    assert_eq!(submit("echo hi\n"), "echo hi");
    assert_eq!(submit("echo hi\n\n"), "echo hi");
    assert_eq!(submit("echo hi\n   \n\t\n"), "echo hi");
}

#[test]
fn preserves_interior_blank_lines() {
    assert_eq!(submit("echo a\n\necho b"), "echo a\n\necho b");
    // Interior blanks stay even when trailing blanks are stripped.
    assert_eq!(submit("echo a\n\necho b\n\n"), "echo a\n\necho b");
}

#[test]
fn single_line_is_returned_verbatim() {
    // No newline → single-line: trailing whitespace is left untouched.
    assert_eq!(submit("echo hi  "), "echo hi  ");
    assert_eq!(submit("echo hi"), "echo hi");
    assert_eq!(submit(""), "");
}

#[test]
fn take_submit_clears_the_pad() {
    let mut pad = InputPad::new();
    pad.set_contents("echo a\necho b\n\n");
    let line = pad.take_submit();
    assert_eq!(line, "echo a\necho b");
    assert!(pad.is_empty());
}
