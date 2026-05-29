//! Output cap + truncation tests (T019): per-block ring-buffer head-dropping
//! and whole-transcript eviction, with raw bytes retained and the truncation
//! flag set (FR-015, FR-016, SC-006).

use kapollo::config::Caps;
use kapollo::session::ringbuf::OutputBuffer;
use kapollo::session::Transcript;

#[test]
fn byte_cap_drops_head_and_retains_tail() {
    let mut buf = OutputBuffer::new(8, 0); // 8-byte cap, no line cap
    buf.push(b"0123456789"); // 10 bytes > cap
    assert!(buf.truncated());
    assert_eq!(buf.byte_len(), 8);
    assert_eq!(buf.to_vec(), b"23456789"); // newest bytes retained
}

#[test]
fn line_cap_drops_whole_leading_lines() {
    let mut buf = OutputBuffer::new(0, 2); // 2-line cap
    buf.push(b"a\nb\nc\n");
    assert!(buf.truncated());
    assert_eq!(buf.to_vec(), b"b\nc\n");
}

#[test]
fn untruncated_buffer_reports_false() {
    let mut buf = OutputBuffer::new(1024, 1024);
    buf.push(b"small\n");
    assert!(!buf.truncated());
    assert_eq!(buf.to_vec(), b"small\n");
}

#[test]
fn transcript_evicts_oldest_blocks_over_block_cap() {
    let caps = Caps {
        per_block_bytes: 1 << 20,
        per_block_lines: 50_000,
        transcript_bytes: 1 << 27,
        transcript_blocks: 2,
    };
    let mut transcript = Transcript::new(caps);
    transcript.begin_block("a".to_string());
    transcript.begin_block("b".to_string());
    transcript.begin_block("c".to_string());

    assert_eq!(transcript.blocks().len(), 2);
    assert_eq!(transcript.blocks()[0].command, "b"); // oldest "a" evicted
    assert_eq!(transcript.blocks()[1].command, "c");
}
