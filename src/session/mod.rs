//! Session model: the transcript and its blocks.

pub mod block;
pub mod ringbuf;

pub use block::{Block, BlockId, BlockState};

use crate::config::Caps;

/// The ordered collection of blocks for the running kapollo instance — the
/// source of truth the UI renders from. Enforces the whole-transcript caps,
/// evicting the oldest blocks first (FR-016).
#[derive(Debug)]
pub struct Transcript {
    blocks: Vec<Block>,
    caps: Caps,
    scroll_offset: usize,
    next_id: BlockId,
}

impl Transcript {
    /// Create an empty transcript bounded by `caps`.
    pub fn new(caps: Caps) -> Self {
        Self {
            blocks: Vec::new(),
            caps,
            scroll_offset: 0,
            next_id: 1,
        }
    }

    /// Begin a new block for `command`, returning its id. Enforces caps.
    pub fn begin_block(&mut self, command: String) -> BlockId {
        let id = self.next_id;
        self.next_id += 1;
        self.blocks.push(Block::new(
            id,
            command,
            self.caps.per_block_bytes,
            self.caps.per_block_lines,
        ));
        self.enforce_caps();
        id
    }

    /// Mutable access to a block by id.
    pub fn block_mut(&mut self, id: BlockId) -> Option<&mut Block> {
        self.blocks.iter_mut().find(|b| b.id == id)
    }

    /// Shared access to a block by id.
    pub fn block(&self, id: BlockId) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }

    /// Close the block with `id`, recording its exit code.
    pub fn close_block(&mut self, id: BlockId, exit_code: Option<i32>) {
        if let Some(block) = self.block_mut(id) {
            block.close(exit_code);
        }
    }

    /// All blocks, oldest first.
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// Clear the visible transcript (FR-023, `/clear`).
    pub fn clear(&mut self) {
        self.blocks.clear();
        self.scroll_offset = 0;
    }

    /// Current scroll position (independent of the input pad; FR-014).
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Set the scroll position.
    pub fn set_scroll_offset(&mut self, offset: usize) {
        self.scroll_offset = offset;
    }

    /// Scroll the transcript up (toward older output) by `n` lines (FR-014).
    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(n);
    }

    /// Scroll the transcript down (toward newer output) by `n` lines (FR-014).
    pub fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Total retained output bytes across all blocks.
    pub fn total_bytes(&self) -> u64 {
        self.blocks.iter().map(|b| b.output.byte_len()).sum()
    }

    fn enforce_caps(&mut self) {
        while self.blocks.len() as u64 > self.caps.transcript_blocks {
            self.blocks.remove(0);
        }
        while self.total_bytes() > self.caps.transcript_bytes && self.blocks.len() > 1 {
            self.blocks.remove(0);
        }
    }
}
