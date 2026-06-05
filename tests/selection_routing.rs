//! Mouse routing + copy-path integration (T021, US2): the routing decision
//! table sends shift to the host, a full-screen / mouse-grabbing child its own
//! events, and everything else to kapollo's selection layer; the output
//! processor surfaces the mouse-tracking and alt-screen mode changes that feed
//! that decision; and the copy path frames OSC 52 and selects a method per
//! config (FR-009/012/015/016, D28).

use kapollo::clipboard::{self, CopyMethod};
use kapollo::input::router::{route_mouse, MouseRoute};
use kapollo::output::{Boundary, OutputProcessor};
use kapollo::session::Transcript;

fn apply(payload: &[u8]) -> Vec<Boundary> {
    let mut processor = OutputProcessor::osc133();
    let mut tx = Transcript::new(kapollo::config::Caps::default());
    let mut current = None;
    processor.apply(payload, &mut tx, &mut current)
}

#[test]
fn shift_always_bypasses_to_host() {
    // Shift wins regardless of alt-screen / child-mouse context (FR-016).
    assert_eq!(route_mouse(true, false, false), MouseRoute::Bypass);
    assert_eq!(route_mouse(true, true, false), MouseRoute::Bypass);
    assert_eq!(route_mouse(true, false, true), MouseRoute::Bypass);
    assert_eq!(route_mouse(true, true, true), MouseRoute::Bypass);
}

#[test]
fn alt_screen_or_child_mouse_forwards_to_child() {
    assert_eq!(route_mouse(false, true, false), MouseRoute::ToChild);
    assert_eq!(route_mouse(false, false, true), MouseRoute::ToChild);
    assert_eq!(route_mouse(false, true, true), MouseRoute::ToChild);
}

#[test]
fn plain_event_is_consumed_by_kapollo() {
    assert_eq!(route_mouse(false, false, false), MouseRoute::Consumed);
}

#[test]
fn mouse_tracking_enable_is_surfaced() {
    let boundaries = apply(b"\x1b[?1000h");
    assert!(
        boundaries
            .iter()
            .any(|b| matches!(b, Boundary::MouseTrackingEnable(_))),
        "got {boundaries:?}"
    );
}

#[test]
fn mouse_tracking_disable_is_surfaced() {
    let boundaries = apply(b"\x1b[?1000l");
    assert!(
        boundaries
            .iter()
            .any(|b| matches!(b, Boundary::MouseTrackingDisable(_))),
        "got {boundaries:?}"
    );
}

#[test]
fn alt_screen_enter_is_surfaced() {
    let boundaries = apply(b"\x1b[?1049h");
    assert!(
        boundaries.contains(&Boundary::AltScreenEnter),
        "got {boundaries:?}"
    );
}

#[test]
fn osc52_frames_the_clipboard_set_sequence() {
    let frame = clipboard::osc52_frame(b"hi");
    // ESC ] 52 ; c ; <base64("hi")="aGk=" > ST
    assert_eq!(frame, "\x1b]52;c;aGk=\x1b\\");
}

#[test]
fn copy_prefers_osc52_when_enabled() {
    let method = clipboard::copy("hi", true, true).expect("copy");
    assert_eq!(method, CopyMethod::Osc52(clipboard::osc52_frame(b"hi")));
}

#[test]
fn copy_errors_when_no_method_enabled() {
    assert!(clipboard::copy("hi", false, false).is_err());
}
