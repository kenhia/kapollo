//! Context-preserving scrollback tests (sprint 005, US3; FR-013/014/015/016).
//! `scroll_page_up/down(context)` advance by `(viewport - context).max(1)`,
//! clamped to `[0, max_scroll]`, with a hard ≥1-line floor on a short pad.

use kapollo::config::Caps;
use kapollo::session::Transcript;

/// A transcript whose last render recorded a `viewport`-line view over content
/// tall enough to scroll up to `max` lines.
fn transcript(viewport: usize, max: usize) -> Transcript {
    let t = Transcript::new(Caps::default());
    t.record_view(viewport, max);
    t
}

#[test]
fn page_up_advances_by_viewport_minus_context() {
    let mut t = transcript(10, 100);
    t.scroll_page_up(3);
    assert_eq!(t.scroll_offset(), 7); // 10 - 3
    t.scroll_page_up(3);
    assert_eq!(t.scroll_offset(), 14);
}

#[test]
fn page_down_advances_by_viewport_minus_context() {
    let mut t = transcript(10, 100);
    t.set_scroll_offset(20);
    t.scroll_page_down(3);
    assert_eq!(t.scroll_offset(), 13); // 20 - 7
}

#[test]
fn page_up_clamps_at_max_scroll() {
    let mut t = transcript(10, 100);
    t.set_scroll_offset(96);
    t.scroll_page_up(3); // 96 + 7 = 103 -> clamped to 100
    assert_eq!(t.scroll_offset(), 100);
}

#[test]
fn page_down_clamps_at_bottom() {
    let mut t = transcript(10, 100);
    t.set_scroll_offset(4);
    t.scroll_page_down(3); // 4 - 7 -> saturates to 0
    assert_eq!(t.scroll_offset(), 0);
}

#[test]
fn short_pad_still_advances_at_least_one_line() {
    // viewport (2) <= context (3): advance must floor to 1, never 0.
    let mut t = transcript(2, 100);
    t.scroll_page_up(3);
    assert_eq!(t.scroll_offset(), 1);
    t.scroll_page_up(3);
    assert_eq!(t.scroll_offset(), 2);
    t.scroll_page_down(3);
    assert_eq!(t.scroll_offset(), 1);
}

#[test]
fn context_equal_to_viewport_floors_to_one() {
    let mut t = transcript(3, 100);
    t.scroll_page_up(3); // (3 - 3).max(1) = 1
    assert_eq!(t.scroll_offset(), 1);
}

#[test]
fn line_scroll_moves_exactly_one() {
    let mut t = transcript(10, 100);
    t.scroll_line_up();
    assert_eq!(t.scroll_offset(), 1);
    t.scroll_line_up();
    assert_eq!(t.scroll_offset(), 2);
    t.scroll_line_down();
    assert_eq!(t.scroll_offset(), 1);
}

#[test]
fn line_scroll_up_clamps_at_the_top() {
    // Regression (smoke test item 14): scrolling up past the oldest line must
    // not inflate the offset beyond `max_scroll`, or scrolling back down would
    // need extra phantom presses before the view moves.
    let mut t = transcript(10, 100);
    t.scroll_to_top();
    assert_eq!(t.scroll_offset(), 100);
    for _ in 0..5 {
        t.scroll_line_up();
    }
    assert_eq!(t.scroll_offset(), 100, "must stay pinned at the top");
    // A single line-down now moves immediately (no phantom region to unwind).
    t.scroll_line_down();
    assert_eq!(t.scroll_offset(), 99);
}

#[test]
fn top_and_bottom_jump() {
    let mut t = transcript(10, 100);
    t.scroll_to_top();
    assert_eq!(t.scroll_offset(), 100);
    t.scroll_to_bottom();
    assert_eq!(t.scroll_offset(), 0);
}
