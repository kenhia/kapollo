//! Block-boundary and exit-code tests for the output processor (T018). Covers
//! OSC 133 delimiting, exit-code capture, the sentinel fallback path, and the
//! kwi #34 typed-ahead regression (FR-005, FR-006).

use kapollo::config::Caps;
use kapollo::output::{BlockAssembler, Boundary, OutputProcessor, ProcessorEvent};
use kapollo::session::{BlockStore, Transcript};

/// Feed `bytes` through the processor + assembler, returning the boundaries
/// observed (mirrors the event loop's drain pass).
fn feed(
    processor: &mut OutputProcessor,
    assembler: &mut BlockAssembler,
    transcript: &mut Transcript,
    store: &mut BlockStore,
    bytes: &[u8],
) -> Vec<Boundary> {
    let mut boundaries = Vec::new();
    for event in processor.process(bytes) {
        assembler.apply(&event, transcript, store, 0);
        if let ProcessorEvent::Boundary(b) = event {
            boundaries.push(b);
        }
    }
    boundaries
}

#[test]
fn osc133_delimits_block_and_captures_exit() {
    let mut transcript = Transcript::new(Caps::default());
    let mut store = BlockStore::new(&Caps::default());
    let mut assembler = BlockAssembler::new();
    let id = transcript.begin_block("echo hi".to_string());
    let sid = store.begin("echo hi".to_string(), None);

    let mut processor = OutputProcessor::osc133();
    processor.begin_command();
    assembler.begin(id, sid);

    // The echoed command (before the `C` mark) must be excluded; only bytes
    // between `C` and `D` belong to the block.
    let bytes = b"echo hi\n\x1b]133;C\x07hi\n\x1b]133;D;0\x07";
    let boundaries = feed(
        &mut processor,
        &mut assembler,
        &mut transcript,
        &mut store,
        bytes,
    );

    let block = &transcript.blocks()[0];
    assert_eq!(block.output_lossy(), "hi\n");
    assert_eq!(block.exit_code, Some(0));
    assert!(assembler.is_idle(), "the block should be closed");
    assert!(boundaries
        .iter()
        .any(|b| matches!(b, Boundary::CommandEnd { exit_code: Some(0) })));
    // The canonical store sealed the same text and exit code.
    assert_eq!(store.text(sid).as_deref(), Some("hi\n"));
    assert_eq!(store.get(sid).and_then(|b| b.exit_code), Some(0));
}

#[test]
fn osc133_captures_nonzero_exit_code() {
    let mut transcript = Transcript::new(Caps::default());
    let mut store = BlockStore::new(&Caps::default());
    let mut assembler = BlockAssembler::new();
    let id = transcript.begin_block("false".to_string());
    let sid = store.begin("false".to_string(), None);

    let mut processor = OutputProcessor::osc133();
    processor.begin_command();
    assembler.begin(id, sid);
    feed(
        &mut processor,
        &mut assembler,
        &mut transcript,
        &mut store,
        b"\x1b]133;C\x07\x1b]133;D;1\x07",
    );

    assert_eq!(transcript.blocks()[0].exit_code, Some(1));
    assert!(assembler.is_idle());
}

#[test]
fn sentinel_fallback_closes_block_with_exit_code() {
    let mut transcript = Transcript::new(Caps::default());
    let mut store = BlockStore::new(&Caps::default());
    let mut assembler = BlockAssembler::new();
    let id = transcript.begin_block("echo hi".to_string());
    let sid = store.begin("echo hi".to_string(), None);

    let mut processor = OutputProcessor::sentinel("NONCE123");
    processor.begin_command();
    assembler.begin(id, sid);
    feed(
        &mut processor,
        &mut assembler,
        &mut transcript,
        &mut store,
        b"hi\nNONCE123;0\n",
    );

    let block = &transcript.blocks()[0];
    assert_eq!(block.output_lossy(), "hi\n");
    assert_eq!(block.exit_code, Some(0));
    assert!(assembler.is_idle());
}

#[test]
fn typed_ahead_command_keeps_both_blocks_intact() {
    // kwi #34 regression: a command submitted while another is running must
    // not steal the running block's remaining output or its end mark, and its
    // own output must land in its own block.
    let mut transcript = Transcript::new(Caps::default());
    let mut store = BlockStore::new(&Caps::default());
    let mut processor = OutputProcessor::osc133();
    let mut assembler = BlockAssembler::new();

    // First command submitted at an idle prompt; its output starts flowing.
    let a_tx = transcript.begin_block("sleep 5; echo foo".to_string());
    let a_store = store.begin("sleep 5; echo foo".to_string(), None);
    processor.begin_command();
    assembler.begin(a_tx, a_store);
    feed(
        &mut processor,
        &mut assembler,
        &mut transcript,
        &mut store,
        b"\x1b]133;C\x07",
    );

    // Second command typed ahead while the first still runs. The event loop
    // does NOT call begin_command here (the assembler is not idle), so the
    // running command's capture is untouched.
    let b_tx = transcript.begin_block("ls".to_string());
    let b_store = store.begin("ls".to_string(), None);
    assembler.begin(b_tx, b_store);

    // The first command's output and end mark arrive after the type-ahead…
    feed(
        &mut processor,
        &mut assembler,
        &mut transcript,
        &mut store,
        b"foo\n\x1b]133;D;0\x07",
    );
    // …then the shell prompts, echoes the queued command (both excluded from
    // capture), and runs it.
    feed(
        &mut processor,
        &mut assembler,
        &mut transcript,
        &mut store,
        b"prompt> ls\n\x1b]133;C\x07file_a\nfile_b\n\x1b]133;D;1\x07",
    );

    let a = transcript.block(a_tx).expect("first block retained");
    assert_eq!(a.output_lossy(), "foo\n", "running block kept its output");
    assert_eq!(
        a.exit_code,
        Some(0),
        "first end mark sealed the first block"
    );

    let b = transcript.block(b_tx).expect("second block retained");
    assert_eq!(b.output_lossy(), "file_a\nfile_b\n");
    assert_eq!(
        b.exit_code,
        Some(1),
        "second end mark sealed the second block"
    );

    assert!(assembler.is_idle(), "both blocks closed");
    assert_eq!(store.text(a_store).as_deref(), Some("foo\n"));
    assert_eq!(store.text(b_store).as_deref(), Some("file_a\nfile_b\n"));
}
