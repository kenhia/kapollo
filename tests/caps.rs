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

/// Reference cap enforcement matching the original byte-at-a-time semantics,
/// used to prove the incremental/bulk implementation is behavior-preserving
/// (T006, FR-014).
fn reference(cap_bytes: u64, cap_lines: u64, pushes: &[&[u8]]) -> (Vec<u8>, bool) {
    use std::collections::VecDeque;
    let mut bytes: VecDeque<u8> = VecDeque::new();
    let mut truncated = false;
    for data in pushes {
        bytes.extend(data.iter().copied());
        if cap_bytes > 0 {
            while bytes.len() as u64 > cap_bytes {
                bytes.pop_front();
                truncated = true;
            }
        }
        if cap_lines > 0 {
            let mut lines = bytes.iter().filter(|&&b| b == b'\n').count() as u64;
            while lines > cap_lines {
                while let Some(byte) = bytes.pop_front() {
                    truncated = true;
                    if byte == b'\n' {
                        break;
                    }
                }
                lines -= 1;
            }
        }
    }
    (bytes.iter().copied().collect(), truncated)
}

#[test]
fn incremental_enforcement_matches_reference() {
    let cases: &[(u64, u64, &[&[u8]])] = &[
        (8, 0, &[b"0123456789"]),
        (0, 2, &[b"a\nb\nc\n"]),
        (16, 3, &[b"one\ntwo\n", b"three\nfour\nfive\n"]),
        (10, 0, &[b"abc", b"def", b"ghijklmnop"]),
        (
            0,
            1,
            &[b"no newline yet", b" still none", b"\nnow a line\n"],
        ),
        (5, 5, &[b"\n\n\n\n\n\n\n\n"]),
        (1, 1, &[b"x", b"\n", b"y", b"\n", b"z"]),
    ];
    for (cap_bytes, cap_lines, pushes) in cases {
        let (want_bytes, want_trunc) = reference(*cap_bytes, *cap_lines, pushes);
        let mut buf = OutputBuffer::new(*cap_bytes, *cap_lines);
        for data in *pushes {
            buf.push(data);
        }
        assert_eq!(
            buf.to_vec(),
            want_bytes,
            "bytes mismatch for caps=({cap_bytes},{cap_lines}) pushes={pushes:?}"
        );
        assert_eq!(
            buf.byte_len(),
            want_bytes.len() as u64,
            "byte_len mismatch for caps=({cap_bytes},{cap_lines})"
        );
        assert_eq!(
            buf.truncated(),
            want_trunc,
            "truncated mismatch for caps=({cap_bytes},{cap_lines})"
        );
    }
}

#[test]
fn tail_fast_path_keeps_last_cap_bytes_on_huge_push() {
    let mut buf = OutputBuffer::new(8, 0); // 8-byte cap, no line cap
    let mut data = Vec::new();
    data.extend_from_slice(b"AAAAAAAAAAAAAAAA"); // 16 bytes
    data.extend_from_slice(b"12345678"); // last 8 bytes
    buf.push(&data);
    assert!(buf.truncated());
    assert_eq!(buf.byte_len(), 8);
    assert_eq!(buf.to_vec(), b"12345678");
}

#[test]
fn tail_fast_path_applies_line_cap() {
    let mut buf = OutputBuffer::new(16, 2); // 16-byte cap, 2-line cap
    let data = b"l1\nl2\nl3\nl4\nl5\n"; // 15 bytes, 5 lines, all within byte cap
    buf.push(data);
    assert!(buf.truncated());
    // Only the last two lines survive the line cap.
    assert_eq!(buf.to_vec(), b"l4\nl5\n");
}

#[test]
fn flood_enforcement_stays_within_time_budget() {
    use std::time::Instant;
    // Per-block defaults: 1 MiB / 50k lines. Simulate `yes | head -n 5000000`:
    // five million short lines pushed individually. The original implementation
    // rescanned the whole buffer and popped byte-at-a-time per push (O(n^2));
    // the incremental/bulk implementation must keep this near-instant.
    let mut buf = OutputBuffer::new(1 << 20, 50_000);
    let line = b"y\n";
    let start = Instant::now();
    for _ in 0..5_000_000u64 {
        buf.push(line);
    }
    let elapsed = start.elapsed();
    assert!(buf.truncated());
    assert!(buf.byte_len() <= 1 << 20);
    assert!(
        elapsed.as_secs() < 5,
        "flood enforcement took too long: {elapsed:?}"
    );
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
