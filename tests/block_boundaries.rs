//! Block-boundary and exit-code tests for the output processor (T018). Covers
//! OSC 133 delimiting, exit-code capture, and the sentinel fallback path
//! (FR-005, FR-006).

use kapollo::config::Caps;
use kapollo::output::{Boundary, OutputProcessor};
use kapollo::session::Transcript;

#[test]
fn osc133_delimits_block_and_captures_exit() {
    let mut transcript = Transcript::new(Caps::default());
    let id = transcript.begin_block("echo hi".to_string());
    let mut current = Some(id);

    let mut processor = OutputProcessor::osc133();
    processor.begin_command();

    // The echoed command (before the `C` mark) must be excluded; only bytes
    // between `C` and `D` belong to the block.
    let bytes = b"echo hi\n\x1b]133;C\x07hi\n\x1b]133;D;0\x07";
    let boundaries = processor.apply(bytes, &mut transcript, &mut current);

    let block = &transcript.blocks()[0];
    assert_eq!(block.output_lossy(), "hi\n");
    assert_eq!(block.exit_code, Some(0));
    assert!(current.is_none(), "the block should be closed");
    assert!(boundaries
        .iter()
        .any(|b| matches!(b, Boundary::CommandEnd { exit_code: Some(0) })));
}

#[test]
fn osc133_captures_nonzero_exit_code() {
    let mut transcript = Transcript::new(Caps::default());
    let id = transcript.begin_block("false".to_string());
    let mut current = Some(id);

    let mut processor = OutputProcessor::osc133();
    processor.begin_command();
    processor.apply(
        b"\x1b]133;C\x07\x1b]133;D;1\x07",
        &mut transcript,
        &mut current,
    );

    assert_eq!(transcript.blocks()[0].exit_code, Some(1));
    assert!(current.is_none());
}

#[test]
fn sentinel_fallback_closes_block_with_exit_code() {
    let mut transcript = Transcript::new(Caps::default());
    let id = transcript.begin_block("echo hi".to_string());
    let mut current = Some(id);

    let mut processor = OutputProcessor::sentinel("NONCE123");
    processor.begin_command();
    processor.apply(b"hi\nNONCE123;0\n", &mut transcript, &mut current);

    let block = &transcript.blocks()[0];
    assert_eq!(block.output_lossy(), "hi\n");
    assert_eq!(block.exit_code, Some(0));
    assert!(current.is_none());
}
