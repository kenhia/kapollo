//! Output processor orchestration: routes raw PTY bytes through the active
//! boundary detector (OSC 133 or sentinel), appends decoded output to the
//! current block's ring buffer, and closes blocks on the end mark (FR-004,
//! FR-006, FR-009; research R6).

pub mod parser;
pub mod sentinel;

pub use parser::{Boundary, Osc133Parser, ProcessorEvent};
pub use sentinel::SentinelScanner;

use crate::pty::BoundaryMode;
use crate::session::{BlockId, BlockState, Transcript};

/// Drives block-boundary detection and applies the results to a [`Transcript`].
pub struct OutputProcessor {
    osc: Option<Osc133Parser>,
    sentinel: Option<SentinelScanner>,
    alt_screen: bool,
    capturing: bool,
}

impl OutputProcessor {
    /// Construct a processor matching the session's boundary mode.
    pub fn for_mode(mode: BoundaryMode, nonce: &str) -> Self {
        match mode {
            BoundaryMode::Osc133 => Self::osc133(),
            BoundaryMode::Sentinel => Self::sentinel(nonce),
        }
    }

    /// OSC 133 mode: output is captured only between the `C` and `D` marks, so
    /// the echoed command line and prompt are excluded.
    pub fn osc133() -> Self {
        Self {
            osc: Some(Osc133Parser::new()),
            sentinel: None,
            alt_screen: false,
            capturing: false,
        }
    }

    /// Sentinel mode: there is no output-start mark, so capture runs for the
    /// whole span between command submit and the nonce.
    pub fn sentinel(nonce: &str) -> Self {
        Self {
            osc: None,
            sentinel: Some(SentinelScanner::new(nonce)),
            alt_screen: false,
            capturing: true,
        }
    }

    /// Whether the wrapped program is currently in the alternate screen.
    pub fn in_alt_screen(&self) -> bool {
        self.alt_screen
    }

    /// Reset capture state for a freshly submitted command.
    pub fn begin_command(&mut self) {
        // In OSC 133 mode wait for the `C` mark; in sentinel mode capture now.
        self.capturing = self.sentinel.is_some();
    }

    fn parse(&mut self, bytes: &[u8], out: &mut Vec<ProcessorEvent>) {
        if let Some(osc) = self.osc.as_mut() {
            osc.feed(bytes, out);
        } else if let Some(sentinel) = self.sentinel.as_mut() {
            sentinel.feed(bytes, out);
        }
    }

    /// Process `bytes` from the PTY, mutating `transcript` and the
    /// `current_block` cursor. Returns the boundaries observed, newest last,
    /// so the caller can react (e.g. update the last exit code).
    pub fn apply(
        &mut self,
        bytes: &[u8],
        transcript: &mut Transcript,
        current_block: &mut Option<BlockId>,
    ) -> Vec<Boundary> {
        let mut events = Vec::new();
        self.parse(bytes, &mut events);

        let mut boundaries = Vec::new();
        for event in events {
            match event {
                ProcessorEvent::Output(data) => {
                    if self.capturing {
                        if let Some(id) = *current_block {
                            if let Some(block) = transcript.block_mut(id) {
                                block.push_output(&data);
                            }
                        }
                    }
                }
                ProcessorEvent::Boundary(boundary) => {
                    self.handle_boundary(&boundary, transcript, current_block);
                    boundaries.push(boundary);
                }
            }
        }
        boundaries
    }

    fn handle_boundary(
        &mut self,
        boundary: &Boundary,
        transcript: &mut Transcript,
        current_block: &mut Option<BlockId>,
    ) {
        match boundary {
            Boundary::OutputStart => self.capturing = true,
            Boundary::CommandEnd { exit_code } => {
                self.capturing = false;
                if let Some(id) = current_block.take() {
                    transcript.close_block(id, *exit_code);
                }
            }
            Boundary::AltScreenEnter => {
                self.alt_screen = true;
                // Suspend transcript capture: the full-screen program owns the
                // terminal and its raw output is passed through, not recorded
                // into the block (FR-018).
                self.capturing = false;
                if let Some(id) = *current_block {
                    if let Some(block) = transcript.block_mut(id) {
                        block.state = BlockState::Interactive;
                    }
                }
            }
            Boundary::AltScreenLeave => {
                self.alt_screen = false;
                // Resume capture for any trailing output before the command's
                // end mark.
                self.capturing = true;
                if let Some(id) = *current_block {
                    if let Some(block) = transcript.block_mut(id) {
                        block.state = BlockState::Running;
                    }
                }
            }
            Boundary::PromptStart | Boundary::CommandStart => {}
            // OSC 7 cwd updates are surfaced to the caller via the returned
            // boundaries; they do not affect capture state (FR-019).
            Boundary::Cwd(_) => {}
        }
    }
}
