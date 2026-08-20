//! Block association for the parsed PTY event stream (kwi #34).
//!
//! The PTY byte stream is strictly ordered: a running command's output and its
//! end mark always arrive before a queued (typed-ahead) command's echo, `C`
//! mark, and output. The assembler therefore keeps a FIFO of in-flight
//! commands — submissions push, the end mark closes the FRONT and pops — so a
//! command submitted while another is running waits its turn instead of
//! stealing the running block's output or its end mark.

use std::collections::VecDeque;

use wezterm_term::StableRowIndex;

use crate::output::parser::{Boundary, ProcessorEvent};
use crate::session::{BlockId, BlockState, BlockStore, Transcript};

/// One submitted-but-unfinished command: its transcript block and its
/// canonical-store block, created together at submit time.
#[derive(Debug, Clone, Copy)]
struct InFlight {
    tx: BlockId,
    store: BlockId,
}

/// A command that just completed (OSC 133 `D` / the sentinel end mark).
#[derive(Debug, Clone, Copy)]
pub struct ClosedBlock {
    pub tx: BlockId,
    pub store: BlockId,
    pub exit_code: Option<i32>,
}

/// Associates ordered [`ProcessorEvent`]s with transcript + store blocks via
/// the in-flight FIFO. Owns no block data itself — only the queue of ids.
#[derive(Debug, Default)]
pub struct BlockAssembler {
    in_flight: VecDeque<InFlight>,
}

impl BlockAssembler {
    /// An assembler with nothing in flight.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a freshly submitted command's blocks at the back of the queue.
    pub fn begin(&mut self, tx: BlockId, store: BlockId) {
        self.in_flight.push_back(InFlight { tx, store });
    }

    /// Whether no command is awaiting its end mark.
    pub fn is_idle(&self) -> bool {
        self.in_flight.is_empty()
    }

    /// Apply one event to the block model. Output spans append to the front
    /// block; `C` anchors the front store block's start row; the end mark
    /// closes + seals the front block and pops it, returning it so the caller
    /// can react (exit-code bookkeeping, `/pipe` completion). `cursor_row` is
    /// the grid cursor's stable row after the chunk containing this event was
    /// applied — the best row anchor available without per-event grid replay.
    pub fn apply(
        &mut self,
        event: &ProcessorEvent,
        transcript: &mut Transcript,
        store: &mut BlockStore,
        cursor_row: StableRowIndex,
    ) -> Option<ClosedBlock> {
        match event {
            ProcessorEvent::Output(data) => {
                if let Some(front) = self.in_flight.front() {
                    if let Some(block) = transcript.block_mut(front.tx) {
                        block.push_output(data);
                    }
                }
                None
            }
            ProcessorEvent::Boundary(boundary) => match boundary {
                // Output start (OSC 133 `C`) anchors the store block's first
                // grid row and stamps `started_at` (R3, R7).
                Boundary::OutputStart => {
                    if let Some(front) = self.in_flight.front() {
                        store.set_start_row(front.store, cursor_row);
                    }
                    None
                }
                Boundary::CommandEnd { exit_code } => {
                    let front = self.in_flight.pop_front()?;
                    transcript.close_block(front.tx, *exit_code);
                    // Copy the captured text into the canonical store and seal
                    // it with the exit code and final row (R3, R7).
                    if let Some(block) = transcript.block(front.tx) {
                        store.push_output(front.store, &block.output.to_vec());
                    }
                    store.seal(front.store, *exit_code, cursor_row);
                    Some(ClosedBlock {
                        tx: front.tx,
                        store: front.store,
                        exit_code: *exit_code,
                    })
                }
                // A full-screen program owns the terminal: mark the block so
                // the UI can say so; capture gating happens in the processor.
                Boundary::AltScreenEnter => {
                    self.set_front_state(transcript, BlockState::Interactive);
                    None
                }
                Boundary::AltScreenLeave => {
                    self.set_front_state(transcript, BlockState::Running);
                    None
                }
                _ => None,
            },
            // Prompt spans belong to the app (the divider fold, kwi #47),
            // never to a block.
            ProcessorEvent::Prompt(_) => None,
        }
    }

    fn set_front_state(&self, transcript: &mut Transcript, state: BlockState) {
        if let Some(front) = self.in_flight.front() {
            if let Some(block) = transcript.block_mut(front.tx) {
                block.state = state;
            }
        }
    }
}
