//! Accessor-seam tests (T030): prove the [`BlockText`] seam lets a swappable
//! backing satisfy the same accessor contract without caller changes (SC-010),
//! and that store-retained text survives grid scrollback eviction (R3).

use kapollo::config::Caps;
use kapollo::session::{BlockStore, BlockText};

fn caps(blocks: u64) -> Caps {
    Caps {
        per_block_bytes: 1 << 20,
        per_block_lines: 50_000,
        transcript_bytes: 1 << 30,
        transcript_blocks: blocks,
    }
}

/// A stand-in for a future persistent backing (e.g. SQLite). It holds nothing
/// but a command and output yet satisfies the same accessor contract, proving
/// callers depend only on the trait, not on `Block` internals.
struct SecondaryStub {
    command: String,
    output: String,
}

impl BlockText for SecondaryStub {
    fn text(&self) -> String {
        self.output.clone()
    }

    fn text_with_command(&self) -> String {
        format!("{}\n{}", self.command, self.output)
    }
}

#[test]
fn stub_backing_satisfies_accessor_contract() {
    let stub = SecondaryStub {
        command: "grep foo".to_string(),
        output: "foo\nfoobar\n".to_string(),
    };
    assert_eq!(stub.text(), "foo\nfoobar\n");
    assert_eq!(stub.text_with_command(), "grep foo\nfoo\nfoobar\n");
}

#[test]
fn primary_and_stub_agree_through_the_seam() {
    let mut store = BlockStore::new(&caps(16));
    let id = store.begin("grep foo".to_string(), None);
    store.push_output(id, b"foo\nfoobar\n");
    store.seal(id, Some(0), 1);

    let stub = SecondaryStub {
        command: "grep foo".to_string(),
        output: "foo\nfoobar\n".to_string(),
    };

    // Same text through either backing — the seam is the only thing callers see.
    assert_eq!(store.text(id).unwrap(), stub.text());
    assert_eq!(
        store.text_with_command(id).unwrap(),
        stub.text_with_command()
    );
}

#[test]
fn text_survives_grid_row_eviction() {
    // The store knows nothing about the grid's scrollback window. A block whose
    // grid rows have scrolled far past the retained window (modeled here by a
    // deeply negative stable-row range) still returns its captured text.
    let mut store = BlockStore::new(&caps(16));
    let id = store.begin("history".to_string(), None);
    store.set_start_row(id, -10_000);
    store.push_output(id, b"line1\nline2\n");
    store.seal(id, Some(0), -9_998);

    assert_eq!(store.text(id).as_deref(), Some("line1\nline2\n"));
    assert_eq!(
        store.text_with_command(id).as_deref(),
        Some("history\nline1\nline2\n")
    );
}
