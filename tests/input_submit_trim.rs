//! Whitespace-only line suppression on submit (kwi #46). The default —
//! shipped in sprint 005 as T034 — strips the trailing run of blank lines from
//! a multi-line submission; sprint 010 added the config surface: suppress ALL
//! whitespace-only lines, or suppress nothing, resolved by
//! `WhitespaceSuppression::from_flags` with the documented precedence.

use kapollo::input::{InputPad, WhitespaceSuppression};

fn submit_with(text: &str, suppression: WhitespaceSuppression) -> String {
    let mut pad = InputPad::new();
    pad.set_contents(text);
    pad.take_submit(suppression)
}

fn submit(text: &str) -> String {
    submit_with(text, WhitespaceSuppression::Trailing)
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
    let line = pad.take_submit(WhitespaceSuppression::Trailing);
    assert_eq!(line, "echo a\necho b");
    assert!(pad.is_empty());
}

#[test]
fn all_policy_suppresses_interior_blank_lines_too() {
    // kwi #46: suppress_multiline_whitespace = true.
    let all = WhitespaceSuppression::All;
    assert_eq!(submit_with("echo a\n\necho b\n\n", all), "echo a\necho b");
    assert_eq!(submit_with("echo a\n \t \necho b", all), "echo a\necho b");
    // Single-line input is never altered under any policy.
    assert_eq!(submit_with("echo hi  ", all), "echo hi  ");
    // A buffer of only whitespace lines submits as empty.
    assert_eq!(submit_with("   \n\t\n", all), "");
}

#[test]
fn none_policy_submits_the_buffer_as_typed() {
    // kwi #46: both knobs off — no stripping at all.
    let none = WhitespaceSuppression::None;
    assert_eq!(submit_with("echo a\n\n", none), "echo a\n\n");
    assert_eq!(submit_with("echo a\n   \n", none), "echo a\n   \n");
}

#[test]
fn all_flag_overrides_the_trailing_flag() {
    // kwi #46 documented precedence: (1) overrides (2).
    assert_eq!(
        WhitespaceSuppression::from_flags(true, true),
        WhitespaceSuppression::All
    );
    assert_eq!(
        WhitespaceSuppression::from_flags(true, false),
        WhitespaceSuppression::All
    );
    assert_eq!(
        WhitespaceSuppression::from_flags(false, true),
        WhitespaceSuppression::Trailing
    );
    assert_eq!(
        WhitespaceSuppression::from_flags(false, false),
        WhitespaceSuppression::None
    );
}
