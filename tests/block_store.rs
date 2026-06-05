//! Block-store contract tests (T029): the canonical in-memory source of block
//! text, behind the [`BlockText`] accessor seam (FR-018/019/020, SC-010;
//! research R3/R7). Boundaries come from OSC 133 marks, never grid heuristics.

use std::path::PathBuf;

use kapollo::config::Caps;
use kapollo::session::{BlockState, BlockStore};

fn caps(blocks: u64) -> Caps {
    Caps {
        per_block_bytes: 1 << 20,
        per_block_lines: 50_000,
        transcript_bytes: 1 << 30,
        transcript_blocks: blocks,
    }
}

#[test]
fn begin_then_seal_records_exit_and_row_range() {
    let mut store = BlockStore::new(&caps(16));
    let id = store.begin("echo hi".to_string(), Some(PathBuf::from("/tmp")));
    store.set_start_row(id, 5);
    store.push_output(id, b"hi\n");
    store.seal(id, Some(0), 7);

    let block = store.get(id).expect("block retained");
    assert_eq!(block.state, BlockState::Closed);
    assert_eq!(block.exit_code, Some(0));
    assert_eq!(block.row_range, 5..7);
    assert_eq!(block.cwd, Some(PathBuf::from("/tmp")));
}

#[test]
fn text_with_command_is_command_newline_text() {
    let mut store = BlockStore::new(&caps(16));
    let id = store.begin("ls".to_string(), None);
    store.push_output(id, b"a\nb\n");
    store.seal(id, Some(0), 0);

    let text = store.text(id).expect("text");
    let with = store.text_with_command(id).expect("text_with_command");
    assert_eq!(with, format!("ls\n{text}"));
}

#[test]
fn duration_is_none_before_seal_and_some_after() {
    let mut store = BlockStore::new(&caps(16));
    let id = store.begin("sleep".to_string(), None);
    store.set_start_row(id, 0);
    assert_eq!(store.duration(id), None, "no duration before seal");

    store.seal(id, Some(0), 1);
    let dur = store.duration(id).expect("duration after seal");
    let block = store.get(id).unwrap();
    // started_at ≤ ended_at when both set.
    assert!(block.started_at.unwrap() <= block.ended_at.unwrap());
    // Derived duration matches ended_at − started_at.
    assert_eq!(
        dur,
        block
            .ended_at
            .unwrap()
            .duration_since(block.started_at.unwrap())
            .unwrap()
    );
}

#[test]
fn cap_plus_one_evicts_exactly_the_oldest() {
    let mut store = BlockStore::new(&caps(2));
    let a = store.begin("a".to_string(), None);
    let b = store.begin("b".to_string(), None);
    let c = store.begin("c".to_string(), None); // evicts `a`

    assert_eq!(store.len(), 2);
    assert!(store.get(a).is_none(), "oldest evicted");
    assert!(store.text(a).is_none());
    assert!(store.text_with_command(a).is_none());
    assert!(store.duration(a).is_none());
    assert!(store.get(b).is_some());
    assert!(store.get(c).is_some());
}

#[test]
fn block_at_row_maps_sealed_range_and_none_outside() {
    let mut store = BlockStore::new(&caps(16));
    let id = store.begin("cmd".to_string(), None);
    store.set_start_row(id, 10);
    store.seal(id, Some(0), 13);

    assert_eq!(store.block_at_row(10), Some(id));
    assert_eq!(store.block_at_row(12), Some(id));
    assert_eq!(store.block_at_row(13), None, "end is exclusive");
    assert_eq!(store.block_at_row(99), None, "unknown row");
}

#[test]
fn iter_yields_insertion_order() {
    let mut store = BlockStore::new(&caps(16));
    store.begin("first".to_string(), None);
    store.begin("second".to_string(), None);
    store.begin("third".to_string(), None);

    let commands: Vec<&str> = store.iter().map(|b| b.command.as_str()).collect();
    assert_eq!(commands, ["first", "second", "third"]);
}
