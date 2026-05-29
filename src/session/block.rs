//! A block: one submitted command together with its captured output and exit
//! code. The atomic unit of the transcript and the foundation for later
//! features (D8). See `data-model.md` for the full entity description.

use std::time::SystemTime;

use crate::session::ringbuf::OutputBuffer;

/// Monotonically increasing block identifier, unique within a session.
pub type BlockId = u64;

/// Lifecycle state of a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockState {
    /// The command is running and output is being captured.
    Running,
    /// The command has finished; the block is closed.
    Closed,
    /// The command entered an alt-screen program; capture is suspended.
    Interactive,
}

/// One command plus its captured output and exit code.
#[derive(Debug)]
pub struct Block {
    pub id: BlockId,
    pub command: String,
    pub started_at: SystemTime,
    pub ended_at: Option<SystemTime>,
    pub output: OutputBuffer,
    pub exit_code: Option<i32>,
    pub state: BlockState,
    /// Reserved for the post-MVP privacy feature (D13); always `false` in MVP.
    pub private: bool,
    /// Reserved for the post-MVP output-retention feature (D13); `true` in MVP.
    pub save_output: bool,
}

impl Block {
    /// Create a new `Running` block for `command`, with an output buffer bound
    /// by the given caps.
    pub fn new(id: BlockId, command: String, cap_bytes: u64, cap_lines: u64) -> Self {
        Self {
            id,
            command,
            started_at: SystemTime::now(),
            ended_at: None,
            output: OutputBuffer::new(cap_bytes, cap_lines),
            exit_code: None,
            state: BlockState::Running,
            private: false,
            save_output: true,
        }
    }

    /// Append captured output bytes to this block.
    pub fn push_output(&mut self, data: &[u8]) {
        self.output.push(data);
    }

    /// Close the block, recording its end time and exit code.
    pub fn close(&mut self, exit_code: Option<i32>) {
        self.ended_at = Some(SystemTime::now());
        self.exit_code = exit_code;
        self.state = BlockState::Closed;
    }

    /// Whether this block's output was truncated to honor a cap.
    pub fn truncated(&self) -> bool {
        self.output.truncated()
    }

    /// The captured output decoded lossily as UTF-8 for rendering.
    pub fn output_lossy(&self) -> String {
        String::from_utf8_lossy(&self.output.to_vec()).into_owned()
    }
}
