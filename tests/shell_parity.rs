//! T058 parity check: run an identical command sequence under fish and bash
//! through the full PTY -> OutputProcessor -> Transcript pipeline, and confirm
//! blocks, captured output, exit codes, and shell-state persistence match
//! (SC-009). This is a live-shell integration test (Constitution III
//! documented exception).

use std::time::{Duration, Instant};

use kapollo::config::Caps;
use kapollo::output::{Boundary, OutputProcessor};
use kapollo::pty::{PtyEvent, PtySession};
use kapollo::session::{BlockId, Transcript};

/// Absorb the shell's startup output (banner, first prompt, and any boundary
/// marks the rcfile/init emits before a command runs) so it does not leak into
/// the first command's block. Marks are fed with no current block, mirroring
/// the real app where `current_block` is `None` at startup.
fn drain_startup(session: &mut PtySession, processor: &mut OutputProcessor) {
    let mut transcript = Transcript::new(Caps::default());
    let mut current: Option<BlockId> = None;
    let idle_after = Duration::from_millis(400);
    let mut last = Instant::now();
    while last.elapsed() < idle_after {
        match session.recv_timeout(Duration::from_millis(100)) {
            Ok(PtyEvent::Output(bytes)) => {
                processor.apply(&bytes, &mut transcript, &mut current);
                last = Instant::now();
            }
            Ok(PtyEvent::Exited(_)) => break,
            Err(_) => {}
        }
    }
}

/// Drive one command end-to-end and return the closed block's (output, exit).
fn run_command(
    session: &mut PtySession,
    processor: &mut OutputProcessor,
    transcript: &mut Transcript,
    command: &str,
) -> (String, Option<i32>) {
    let id: BlockId = transcript.begin_block(command.to_string());
    let mut current = Some(id);
    processor.begin_command();
    session.send_command(command).expect("send command");

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        match session.recv_timeout(Duration::from_millis(250)) {
            Ok(PtyEvent::Output(bytes)) => {
                let boundaries = processor.apply(&bytes, transcript, &mut current);
                if boundaries
                    .iter()
                    .any(|b| matches!(b, Boundary::CommandEnd { .. }))
                {
                    break;
                }
            }
            Ok(PtyEvent::Exited(_)) => break,
            Err(_) => {}
        }
    }

    let block = transcript.block(id).expect("block exists");
    (block.output_lossy(), block.exit_code)
}

fn parity_run(shell: &str) -> Vec<(String, Option<i32>)> {
    let mut session = PtySession::spawn(Some(shell)).expect("spawn shell");
    let mut processor = OutputProcessor::for_mode(session.boundary_mode(), session.nonce());
    let mut transcript = Transcript::new(Caps::default());

    // Discard startup output (incl. bash's initial PROMPT_COMMAND mark) so it
    // does not desync the first command's block.
    drain_startup(&mut session, &mut processor);

    let mut results = Vec::new();
    // cd then pwd proves shell state persists across commands.
    run_command(&mut session, &mut processor, &mut transcript, "cd /tmp");
    results.push(run_command(
        &mut session,
        &mut processor,
        &mut transcript,
        "pwd",
    ));
    results.push(run_command(
        &mut session,
        &mut processor,
        &mut transcript,
        "echo parity_ok",
    ));
    results.push(run_command(
        &mut session,
        &mut processor,
        &mut transcript,
        "false",
    ));
    results
}

#[test]
fn fish_and_bash_core_run_loop_match() {
    let fish = parity_run("/usr/bin/fish");
    let bash = parity_run("/usr/bin/bash");

    // pwd persisted across the prior `cd /tmp` in both shells.
    assert!(fish[0].0.contains("/tmp"), "fish pwd: {:?}", fish[0]);
    assert!(bash[0].0.contains("/tmp"), "bash pwd: {:?}", bash[0]);

    // echo output captured identically.
    assert!(fish[1].0.contains("parity_ok"), "fish echo: {:?}", fish[1]);
    assert!(bash[1].0.contains("parity_ok"), "bash echo: {:?}", bash[1]);

    // non-zero exit code captured for `false` in both shells.
    assert_eq!(fish[2].1, Some(1), "fish false exit: {:?}", fish[2]);
    assert_eq!(bash[2].1, Some(1), "bash false exit: {:?}", bash[2]);
}
