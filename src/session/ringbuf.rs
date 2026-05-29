//! A capped, head-dropping byte ring buffer for block output. Retains the tail
//! and records whether any bytes were dropped (FR-015, FR-016; research R6).

use std::collections::VecDeque;

/// A byte buffer with byte and line caps. When a cap is exceeded, the oldest
/// bytes (and whole leading lines for the line cap) are dropped and the buffer
/// is marked truncated.
#[derive(Debug)]
pub struct OutputBuffer {
    bytes: VecDeque<u8>,
    cap_bytes: u64,
    cap_lines: u64,
    truncated: bool,
}

impl OutputBuffer {
    /// Create a buffer bounded by `cap_bytes` and `cap_lines`. A cap of `0`
    /// disables that dimension.
    pub fn new(cap_bytes: u64, cap_lines: u64) -> Self {
        Self {
            bytes: VecDeque::new(),
            cap_bytes,
            cap_lines,
            truncated: false,
        }
    }

    /// Append `data`, enforcing caps afterward.
    pub fn push(&mut self, data: &[u8]) {
        self.bytes.extend(data.iter().copied());
        self.enforce_caps();
    }

    /// Whether any bytes have been dropped to honor a cap.
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// Number of retained bytes.
    pub fn byte_len(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Copy the retained bytes into a contiguous `Vec`.
    pub fn to_vec(&self) -> Vec<u8> {
        self.bytes.iter().copied().collect()
    }

    fn enforce_caps(&mut self) {
        if self.cap_bytes > 0 {
            while self.bytes.len() as u64 > self.cap_bytes {
                self.bytes.pop_front();
                self.truncated = true;
            }
        }

        if self.cap_lines > 0 {
            let mut lines = self.count_lines();
            while lines > self.cap_lines {
                // Drop bytes up to and including the next newline.
                while let Some(byte) = self.bytes.pop_front() {
                    self.truncated = true;
                    if byte == b'\n' {
                        break;
                    }
                }
                lines -= 1;
            }
        }
    }

    fn count_lines(&self) -> u64 {
        self.bytes.iter().filter(|&&b| b == b'\n').count() as u64
    }
}
