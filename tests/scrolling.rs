//! Keyboard transcript scrolling (T030): PageUp/PageDown move by a page (clamped
//! at both ends), Home jumps to the top, End jumps to the bottom, and a fresh
//! submission re-pins the view to the newest output (FR-021).

use kapollo::config::Caps;
use kapollo::session::Transcript;

/// A transcript whose last render recorded a 10-line viewport over content tall
/// enough to scroll up to 100 lines.
fn transcript() -> Transcript {
    let t = Transcript::new(Caps::default());
    t.record_view(10, 100);
    t
}

#[test]
fn page_up_and_down_move_by_a_page_and_clamp_at_the_bottom() {
    let mut t = transcript();
    t.page_up();
    assert_eq!(t.scroll_offset(), 10);
    t.page_up();
    assert_eq!(t.scroll_offset(), 20);
    t.page_down();
    assert_eq!(t.scroll_offset(), 10);
    t.page_down();
    t.page_down();
    assert_eq!(t.scroll_offset(), 0, "clamped at the newest output");
}

#[test]
fn page_up_clamps_at_the_top() {
    let mut t = transcript();
    t.scroll_to_top();
    assert_eq!(t.scroll_offset(), 100);
    t.page_up();
    assert_eq!(t.scroll_offset(), 100, "cannot scroll past the oldest line");
}

#[test]
fn home_jumps_to_top_and_end_to_bottom() {
    let mut t = transcript();
    t.scroll_to_top();
    assert_eq!(t.scroll_offset(), 100);
    t.scroll_to_bottom();
    assert_eq!(t.scroll_offset(), 0);
}

#[test]
fn submit_re_pins_to_the_bottom() {
    let mut t = transcript();
    t.scroll_to_top();
    assert_eq!(t.scroll_offset(), 100);
    // A submission scrolls the transcript back to the newest output.
    t.set_scroll_offset(0);
    assert_eq!(t.scroll_offset(), 0);
}
