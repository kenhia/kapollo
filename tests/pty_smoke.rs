//! Headless PTY smoke test (T017): spawn the real shell in a pseudo-terminal,
//! run a command, and assert its output is captured (SC-001).
//!
//! This is an integration test against a live shell per the Constitution III
//! documented exception for interactive terminal behavior (research R11).

use std::time::{Duration, Instant};

use kapollo::pty::{PtyEvent, PtySession};

#[test]
fn echo_output_is_captured_from_the_pty() {
    let marker = "kapollo_smoke_marker_4f2a";
    let mut session = PtySession::spawn(None).expect("failed to spawn shell");

    session
        .send_command(&format!("echo {marker}"))
        .expect("failed to send command");

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut captured = Vec::new();
    while Instant::now() < deadline {
        match session.recv_timeout(Duration::from_millis(250)) {
            Ok(PtyEvent::Output(bytes)) => {
                captured.extend_from_slice(&bytes);
                let text = String::from_utf8_lossy(&captured);
                // The marker appears once as the echoed input and again as the
                // command's output; two occurrences confirm it actually ran.
                if text.matches(marker).count() >= 2 {
                    return;
                }
            }
            Ok(PtyEvent::Exited(_)) => break,
            Err(_) => {}
        }
    }

    let text = String::from_utf8_lossy(&captured);
    assert!(
        text.contains(marker),
        "expected captured PTY output to contain {marker:?}, got: {text:?}"
    );
}
