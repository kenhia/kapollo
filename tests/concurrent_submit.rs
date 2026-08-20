//! kwi #34 live regression: submit a command while the previous one is still
//! running (type-ahead) and assert both blocks capture their own output and
//! exit codes — no output silently dropped, no end mark sealing the wrong
//! block. Live-shell integration test: PTY type-ahead (kernel echo, deferred
//! read at the next prompt) cannot be unit-tested in isolation.

use std::time::{Duration, Instant};

use kapollo::config::Caps;
use kapollo::output::{BlockAssembler, OutputProcessor};
use kapollo::pty::{PtyEvent, PtySession};
use kapollo::session::{BlockStore, Transcript};

/// Absorb the shell's startup output so it does not leak into the first block.
fn drain_startup(session: &mut PtySession, processor: &mut OutputProcessor) {
    let idle_after = Duration::from_millis(400);
    let mut last = Instant::now();
    while last.elapsed() < idle_after {
        match session.recv_timeout(Duration::from_millis(100)) {
            Ok(PtyEvent::Output(bytes)) => {
                let _ = processor.process(&bytes);
                last = Instant::now();
            }
            Ok(PtyEvent::Exited(_)) => break,
            Err(_) => {}
        }
    }
}

fn typed_ahead_run(shell: &str) {
    let mut session = PtySession::spawn(Some(shell)).expect("spawn shell");
    let mut processor = OutputProcessor::for_mode(session.boundary_mode(), session.nonce());
    let mut assembler = BlockAssembler::new();
    let mut transcript = Transcript::new(Caps::default());
    let mut store = BlockStore::new(&Caps::default());

    drain_startup(&mut session, &mut processor);

    // First command: slow enough that the second submit happens mid-run.
    let first = "sleep 0.7; echo first_marker_xyz";
    let a = transcript.begin_block(first.to_string());
    let a_store = store.begin(first.to_string(), None);
    processor.begin_command();
    assembler.begin(a, a_store);
    session.send_command(first).expect("send first");

    // Second command typed ahead immediately, while the sleep is running.
    // Mirrors the event loop: the assembler is not idle, so begin_command is
    // NOT called and the running command's capture is untouched.
    let second = "echo second_marker_xyz";
    let b = transcript.begin_block(second.to_string());
    let b_store = store.begin(second.to_string(), None);
    assembler.begin(b, b_store);
    session.send_command(second).expect("send second");

    // Drain until both blocks close (or time out and let the asserts report).
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut closed = 0;
    while closed < 2 && Instant::now() < deadline {
        match session.recv_timeout(Duration::from_millis(250)) {
            Ok(PtyEvent::Output(bytes)) => {
                for event in processor.process(&bytes) {
                    if assembler
                        .apply(&event, &mut transcript, &mut store, 0)
                        .is_some()
                    {
                        closed += 1;
                    }
                }
            }
            Ok(PtyEvent::Exited(_)) => break,
            Err(_) => {}
        }
    }
    assert_eq!(closed, 2, "{shell}: both commands should complete");

    // The first block owns its own output. (It may ALSO contain the kernel
    // echo of the typed-ahead second command — that interleaving is exactly
    // what a real terminal shows while a command is running — so only the
    // second block's purity is asserted below.)
    let a_block = transcript.block(a).expect("first block");
    assert!(
        a_block.output_lossy().contains("first_marker_xyz"),
        "{shell}: first command's output lost: {:?}",
        a_block.output_lossy()
    );
    assert_eq!(a_block.exit_code, Some(0), "{shell}: first exit code");

    // The second block owns exactly its own output — nothing from the first.
    let b_block = transcript.block(b).expect("second block");
    assert!(
        b_block.output_lossy().contains("second_marker_xyz"),
        "{shell}: typed-ahead command's output lost: {:?}",
        b_block.output_lossy()
    );
    assert!(
        !b_block.output_lossy().contains("first_marker_xyz"),
        "{shell}: first command's output bled into the second block: {:?}",
        b_block.output_lossy()
    );
    assert_eq!(b_block.exit_code, Some(0), "{shell}: second exit code");

    // The canonical store sealed the same attribution.
    assert!(store.text(a_store).unwrap().contains("first_marker_xyz"));
    assert!(store.text(b_store).unwrap().contains("second_marker_xyz"));
}

#[test]
fn typed_ahead_submission_keeps_both_blocks_live() {
    let mut ran = 0;
    for sh in ["/usr/bin/fish", "/usr/bin/bash"] {
        if !std::path::Path::new(sh).exists() {
            eprintln!("skipping typed-ahead: {sh} not installed");
            continue;
        }
        typed_ahead_run(sh);
        ran += 1;
    }
    assert!(ran > 0, "no shell available to exercise the regression");
}
