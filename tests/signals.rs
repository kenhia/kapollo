//! SIGINT forwarding + clean-teardown test (T031): Ctrl-C interrupts the
//! running command, not kapollo; the wrapped shell survives and keeps
//! accepting input (FR-024, SC-004, SC-005).
//!
//! Integration test against a live shell per the Constitution III documented
//! exception (research R11).

use std::thread::sleep;
use std::time::{Duration, Instant};

use kapollo::pty::{PtyEvent, PtySession};

#[test]
fn ctrl_c_interrupts_command_but_not_the_shell() {
    let marker = "kapollo_survived_9b3c";
    let mut session = PtySession::spawn(None).expect("failed to spawn shell");

    // Start a long-running command, then interrupt it.
    session
        .send_command("sleep 30")
        .expect("failed to start sleep");
    sleep(Duration::from_millis(750)); // let the shell start `sleep`
    session.send_interrupt().expect("failed to send interrupt");

    // The shell must survive the interrupt and run a follow-up command.
    sleep(Duration::from_millis(250));
    session
        .send_command(&format!("echo {marker}"))
        .expect("failed to send follow-up command");

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut captured = Vec::new();
    let mut exited = false;
    while Instant::now() < deadline {
        match session.recv_timeout(Duration::from_millis(250)) {
            Ok(PtyEvent::Output(bytes)) => {
                captured.extend_from_slice(&bytes);
                if String::from_utf8_lossy(&captured).contains(marker) {
                    break;
                }
            }
            Ok(PtyEvent::Exited(_)) => {
                exited = true;
                break;
            }
            Err(_) => {}
        }
    }

    assert!(
        !exited,
        "the wrapped shell must not exit when Ctrl-C interrupts a command"
    );
    let text = String::from_utf8_lossy(&captured);
    assert!(
        text.contains(marker),
        "the shell should accept a new command after the interrupt; got: {text:?}"
    );
}
