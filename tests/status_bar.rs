//! Fixed status bar layout (sprint 005, US4): the pure `fit` formatter and the
//! visibility predicate (FR-017..FR-024). The bar reads
//! `mode | cwd<greedypad>| message | exit`, fits exactly to width, truncates the
//! message before the cwd, never breaks the mode/exit fields, and never wraps.

use kapollo::ui::status;

#[test]
fn roomy_bar_anchors_mode_left_and_exit_right() {
    let s = status::fit(40, "norm", "/home/ken", None, Some(0));
    assert_eq!(s.chars().count(), 40, "bar must fill exactly the width");
    // prefix(7) + cwd(9) + pad + "| 0"(3) == 40 -> pad == 21
    assert_eq!(s, format!("norm | /home/ken{}| 0", " ".repeat(21)));
}

#[test]
fn message_is_right_justified_against_exit() {
    let s = status::fit(50, "norm", "/home/ken", Some("copied"), Some(0));
    assert_eq!(s.chars().count(), 50);
    // prefix(7) + cwd(9) + pad + "| copied | 0"(12) == 50 -> pad == 22
    assert_eq!(s, format!("norm | /home/ken{}| copied | 0", " ".repeat(22)));
}

#[test]
fn no_exit_field_when_exit_is_none() {
    let s = status::fit(30, "norm", "/home/ken", None, None);
    assert_eq!(s.chars().count(), 30);
    // prefix(7) + cwd(9) + trailing pad(14), no right cluster.
    assert_eq!(s, format!("norm | /home/ken{}", " ".repeat(14)));
    assert_eq!(s.matches('|').count(), 1, "only the mode separator: {s:?}");
}

#[test]
fn mode_field_is_four_columns() {
    // A short mode is right-padded; a long mode is clipped to four columns.
    let short = status::fit(20, "x", "/a", None, None);
    assert!(short.starts_with("x    | "), "short mode pad: {short:?}");
    let long = status::fit(20, "verbose", "/a", None, None);
    assert!(long.starts_with("verb | "), "long mode clip: {long:?}");
}

#[test]
fn message_truncates_before_the_cwd() {
    // Narrow bar: the message is shortened (trailing ellipsis) while the cwd and
    // exit field stay intact.
    let s = status::fit(34, "norm", "/x", Some("hello world this is long"), Some(0));
    assert_eq!(s.chars().count(), 34);
    assert!(s.starts_with("norm | /x"), "cwd preserved: {s:?}");
    assert!(s.ends_with("| 0"), "exit preserved: {s:?}");
    assert!(s.contains('…'), "message should be ellipsized: {s:?}");
}

#[test]
fn cwd_middle_ellipsizes_only_after_message_is_gone() {
    let s = status::fit(24, "norm", "/home/ken/src/tools/kapollo", None, Some(0));
    assert_eq!(s.chars().count(), 24);
    assert!(s.starts_with("norm | "), "mode intact: {s:?}");
    assert!(s.ends_with("| 0"), "exit intact: {s:?}");
    assert!(s.contains('…'), "cwd should be middle-ellipsized: {s:?}");
    // The trailing path component stays legible.
    assert!(s.contains("kapollo"), "trailing component kept: {s:?}");
}

#[test]
fn never_exceeds_width_under_pressure() {
    for width in 8..=60usize {
        let s = status::fit(
            width,
            "norm",
            "/very/long/working/directory/path/segment",
            Some("a transient notice message that is also quite long"),
            Some(137),
        );
        assert_eq!(
            s.chars().count(),
            width,
            "bar must be exactly {width} columns, got {:?}",
            s
        );
        assert!(!s.contains('\n'), "bar must never wrap: {s:?}");
        assert!(s.starts_with("norm"), "mode never broken: {s:?}");
    }
}

#[test]
fn zero_width_is_empty() {
    assert_eq!(status::fit(0, "norm", "/a", None, Some(0)), "");
}

#[test]
fn hidden_on_short_terminal_or_when_disabled() {
    assert!(status::is_visible(true, 10));
    assert!(status::is_visible(true, 40));
    assert!(!status::is_visible(true, 9), "hidden below 10 rows");
    assert!(!status::is_visible(false, 40), "hidden when disabled");
}
