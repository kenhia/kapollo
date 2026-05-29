//! Passthrough tests (T042): entering the alternate screen suspends transcript
//! capture and marks the block `Interactive`; leaving it resumes capture and
//! restores the block, with the prior transcript intact (FR-018, FR-020,
//! SC-003).

use kapollo::config::Caps;
use kapollo::output::OutputProcessor;
use kapollo::session::{BlockState, Transcript};

fn transcript() -> Transcript {
    Transcript::new(Caps::default())
}

#[test]
fn alt_screen_enter_marks_block_interactive_and_suspends_capture() {
    let mut processor = OutputProcessor::osc133();
    let mut tx = transcript();
    let id = tx.begin_block("vim notes.txt".to_string());
    let mut current = Some(id);

    // Output starts, then the program switches to the alternate screen.
    processor.apply(b"\x1b]133;C\x07", &mut tx, &mut current);
    processor.apply(b"\x1b[?1049h", &mut tx, &mut current);

    assert!(processor.in_alt_screen(), "processor tracks passthrough");
    let block = tx.block(id).expect("block exists");
    assert_eq!(block.state, BlockState::Interactive);

    // Full-screen drawing bytes are not captured into the transcript.
    processor.apply(b"editor screen contents", &mut tx, &mut current);
    let block = tx.block(id).expect("block exists");
    assert!(
        block.output_lossy().is_empty(),
        "alt-screen output is passed through, not captured"
    );
}

#[test]
fn alt_screen_leave_restores_the_block_and_resumes_capture() {
    let mut processor = OutputProcessor::osc133();
    let mut tx = transcript();
    let id = tx.begin_block("vim notes.txt".to_string());
    let mut current = Some(id);

    processor.apply(b"\x1b]133;C\x07", &mut tx, &mut current);
    processor.apply(b"\x1b[?1049h", &mut tx, &mut current);
    processor.apply(b"\x1b[?1049l", &mut tx, &mut current);

    assert!(!processor.in_alt_screen(), "passthrough has ended");
    let block = tx.block(id).expect("block exists");
    assert_eq!(block.state, BlockState::Running);

    // Trailing output after the program exits is captured again.
    processor.apply(b"done\n", &mut tx, &mut current);
    processor.apply(b"\x1b]133;D;0\x07", &mut tx, &mut current);
    let block = tx.block(id).expect("block exists");
    assert!(block.output_lossy().contains("done"));
    assert_eq!(block.state, BlockState::Closed);
}
