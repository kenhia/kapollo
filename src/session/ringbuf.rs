//! A capped, head-dropping byte ring buffer for block output. Retains the tail
//! and records whether any bytes were dropped (FR-015, FR-016; research R6).

use std::collections::VecDeque;

/// A byte buffer with byte and line caps. When a cap is exceeded, the oldest
/// bytes (and whole leading lines for the line cap) are dropped and the buffer
/// is marked truncated. Cap enforcement is amortized O(1) per byte: the line
/// count is tracked incrementally and over-cap data is trimmed in bulk, never
/// byte-at-a-time (FR-014; research R3).
#[derive(Debug)]
pub struct OutputBuffer {
    bytes: VecDeque<u8>,
    cap_bytes: u64,
    cap_lines: u64,
    truncated: bool,
    /// Running count of `\n` in `bytes`, maintained incrementally (FR-014).
    line_count: u64,
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
            line_count: 0,
        }
    }

    /// Append `data`, enforcing caps afterward.
    pub fn push(&mut self, data: &[u8]) {
        // Tail fast-path (FR-016): a single push at least as large as the byte
        // cap can only leave its own trailing `cap_bytes`. Replace the buffer
        // wholesale so flood pushes cost O(cap_bytes), not O(buffer + data).
        if self.cap_bytes > 0 && data.len() as u64 >= self.cap_bytes {
            let tail = &data[data.len() - self.cap_bytes as usize..];
            if !self.bytes.is_empty() || data.len() as u64 > self.cap_bytes {
                self.truncated = true;
            }
            self.bytes.clear();
            self.bytes.extend(tail.iter().copied());
            self.line_count = count_newlines(tail);
            self.enforce_line_cap();
            return;
        }

        self.line_count += count_newlines(data);
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
        if self.cap_bytes > 0 && self.bytes.len() as u64 > self.cap_bytes {
            let overflow = self.bytes.len() - self.cap_bytes as usize;
            // Count newlines in the prefix being removed, then bulk-drain it.
            let dropped_newlines = self
                .bytes
                .iter()
                .take(overflow)
                .filter(|&&b| b == b'\n')
                .count();
            self.bytes.drain(..overflow);
            self.line_count -= dropped_newlines as u64;
            self.truncated = true;
        }

        self.enforce_line_cap();
    }

    fn enforce_line_cap(&mut self) {
        if self.cap_lines == 0 {
            return;
        }
        while self.line_count > self.cap_lines {
            // Bulk-drop the oldest whole line (up to and including its newline).
            match self.bytes.iter().position(|&b| b == b'\n') {
                Some(pos) => {
                    self.bytes.drain(..=pos);
                    self.line_count -= 1;
                    self.truncated = true;
                }
                // No newline left but the count says otherwise: nothing more to
                // trim by whole lines.
                None => break,
            }
        }
    }
}

/// Count `\n` bytes in a slice.
fn count_newlines(data: &[u8]) -> u64 {
    data.iter().filter(|&&b| b == b'\n').count() as u64
}
