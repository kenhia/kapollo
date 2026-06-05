//! Block store: the in-memory, canonical source of block text (R3, sprint 004).
//!
//! R3 supersedes D29's reconstruct-from-grid lean — retained output is the
//! source of truth for `/save`, `/filter`, and rendering. All callers reach text
//! only through the [`BlockText`] accessor seam, so a future SQLite secondary
//! backing is a drop-in with no caller changes (FR-019, FR-020, SC-010).
//!
//! Block boundaries come from OSC 133 marks (or the sentinel fallback), never
//! grid heuristics (R7): [`BlockStore::begin`] at `B`, [`BlockStore::set_start_row`]
//! at `C` (stamps `started_at`), [`BlockStore::seal`] at `D` (stamps `ended_at`).

use std::collections::VecDeque;
use std::ops::Range;
use std::path::PathBuf;
use std::time::Duration;

use wezterm_term::StableRowIndex;

use crate::config::Caps;
use crate::session::block::{Block, BlockId, BlockText};

/// The canonical, bounded, in-memory collection of blocks. Retains at most
/// `cap` blocks (from [`Caps::transcript_blocks`]); [`BlockStore::begin`] past
/// the cap evicts the oldest. Retained text survives grid scrollback eviction
/// (R3) — `block_at_row` for an evicted row returns `None`, but `text(id)` for
/// a still-retained block still returns its text.
#[derive(Debug)]
pub struct BlockStore {
    /// Blocks in insertion order, oldest at the front for O(1) eviction.
    blocks: VecDeque<Block>,
    /// Maximum retained blocks; `0` disables the bound.
    cap: usize,
    per_block_bytes: u64,
    per_block_lines: u64,
    next_id: BlockId,
}

impl BlockStore {
    /// Create an empty store bounded by `caps`.
    pub fn new(caps: &Caps) -> Self {
        Self {
            blocks: VecDeque::new(),
            cap: caps.transcript_blocks as usize,
            per_block_bytes: caps.per_block_bytes,
            per_block_lines: caps.per_block_lines,
            next_id: 1,
        }
    }

    /// Begin a new running block for `command` (OSC 133 `B`), returning its id.
    /// Evicts the oldest block when the cap is exceeded; the evicted id then
    /// returns `None` from every accessor.
    pub fn begin(&mut self, command: String, cwd: Option<PathBuf>) -> BlockId {
        let id = self.next_id;
        self.next_id += 1;
        let mut block = Block::new(id, command, self.per_block_bytes, self.per_block_lines);
        block.cwd = cwd;
        self.blocks.push_back(block);
        while self.cap > 0 && self.blocks.len() > self.cap {
            self.blocks.pop_front();
        }
        id
    }

    /// Append captured output bytes to the block `id`, if still retained.
    pub fn push_output(&mut self, id: BlockId, data: &[u8]) {
        if let Some(block) = self.find_mut(id) {
            block.push_output(data);
        }
    }

    /// Record the command-execution start (OSC 133 `C`): stamps `started_at`
    /// (wall-clock) and anchors the block's first grid row.
    pub fn set_start_row(&mut self, id: BlockId, start_row: StableRowIndex) {
        if let Some(block) = self.find_mut(id) {
            block.started_at = Some(std::time::SystemTime::now());
            block.row_range.start = start_row;
            if block.row_range.end < start_row {
                block.row_range.end = start_row;
            }
        }
    }

    /// Seal the block `id` (OSC 133 `D`): records the exit code and final grid
    /// row, and stamps `ended_at`.
    pub fn seal(&mut self, id: BlockId, exit_code: Option<i32>, end_row: StableRowIndex) {
        if let Some(block) = self.find_mut(id) {
            block.row_range.end = end_row.max(block.row_range.start);
            block.close(exit_code);
        }
    }

    /// Shared access to a block by id, or `None` if unknown or evicted.
    pub fn get(&self, id: BlockId) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }

    /// The id of the block whose sealed (or in-progress) row range contains
    /// `row`, or `None` for an evicted/unknown row.
    pub fn block_at_row(&self, row: StableRowIndex) -> Option<BlockId> {
        self.blocks
            .iter()
            .find(|b| b.row_range.contains(&row))
            .map(|b| b.id)
    }

    /// The block `id`'s output text, via the [`BlockText`] seam.
    pub fn text(&self, id: BlockId) -> Option<String> {
        self.get(id).map(BlockText::text)
    }

    /// The block `id`'s command-plus-output text, via the [`BlockText`] seam.
    pub fn text_with_command(&self, id: BlockId) -> Option<String> {
        self.get(id).map(BlockText::text_with_command)
    }

    /// Elapsed runtime of block `id`: `ended_at − started_at`; `None` until the
    /// block is sealed (or if it is unknown/evicted).
    pub fn duration(&self, id: BlockId) -> Option<Duration> {
        self.get(id).and_then(Block::duration)
    }

    /// The most recently begun block still retained, if any.
    pub fn last(&self) -> Option<&Block> {
        self.blocks.back()
    }

    /// Number of retained blocks.
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Whether the store holds no blocks.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Iterate retained blocks in insertion order (oldest first).
    pub fn iter(&self) -> impl Iterator<Item = &Block> {
        self.blocks.iter()
    }

    /// All retained grid row ranges paired with their block ids, oldest first.
    pub fn row_ranges(&self) -> impl Iterator<Item = (BlockId, Range<StableRowIndex>)> + '_ {
        self.blocks.iter().map(|b| (b.id, b.row_range.clone()))
    }

    fn find_mut(&mut self, id: BlockId) -> Option<&mut Block> {
        self.blocks.iter_mut().find(|b| b.id == id)
    }
}
