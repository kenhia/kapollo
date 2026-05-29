//! Sentinel fallback boundary detection (research R3). When OSC 133 marks are
//! unavailable, kapollo wraps each command so the shell prints a session-unique
//! nonce followed by the exit status. This scanner watches the byte stream for
//! that nonce, emitting the preceding bytes as output and a synthetic
//! [`Boundary::CommandEnd`] when it fires.

use crate::output::parser::{Boundary, ProcessorEvent};

/// Scans output bytes for the sentinel nonce + exit code line.
pub struct SentinelScanner {
    nonce: Vec<u8>,
    buf: Vec<u8>,
}

impl SentinelScanner {
    /// Create a scanner for the given session nonce.
    pub fn new(nonce: impl Into<String>) -> Self {
        Self {
            nonce: nonce.into().into_bytes(),
            buf: Vec::new(),
        }
    }

    /// Feed `bytes`, appending parsed events (in order) to `out`.
    pub fn feed(&mut self, bytes: &[u8], out: &mut Vec<ProcessorEvent>) {
        self.buf.extend_from_slice(bytes);
        self.scan(out);
    }

    fn scan(&mut self, out: &mut Vec<ProcessorEvent>) {
        loop {
            let Some(pos) = find(&self.buf, &self.nonce) else {
                // No complete nonce yet. Emit everything that cannot still be
                // the start of a nonce, retaining a tail of `nonce.len() - 1`
                // bytes in case the marker is split across feeds.
                let keep = self.nonce.len().saturating_sub(1);
                if self.buf.len() > keep {
                    let emit = self.buf.len() - keep;
                    let chunk: Vec<u8> = self.buf.drain(..emit).collect();
                    out.push(ProcessorEvent::Output(chunk));
                }
                return;
            };

            // Output preceding the nonce belongs to the current block.
            if pos > 0 {
                let chunk: Vec<u8> = self.buf.drain(..pos).collect();
                out.push(ProcessorEvent::Output(chunk));
            }

            // The marker line is `<nonce>;<exit>\n`. Wait until the trailing
            // newline has arrived before consuming it.
            let after = self.nonce.len();
            let Some(nl_rel) = self.buf[after..].iter().position(|&b| b == b'\n') else {
                return; // exit code not fully received yet
            };
            let exit_code = std::str::from_utf8(&self.buf[after + 1..after + nl_rel])
                .ok()
                .and_then(|s| s.trim().parse::<i32>().ok());
            self.buf.drain(..after + nl_rel + 1);
            out.push(ProcessorEvent::Boundary(Boundary::CommandEnd { exit_code }));
        }
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_output_then_command_end() {
        let mut scanner = SentinelScanner::new("NONCE");
        let mut out = Vec::new();
        scanner.feed(b"world\nNONCE;0\n", &mut out);
        assert_eq!(
            out,
            vec![
                ProcessorEvent::Output(b"world\n".to_vec()),
                ProcessorEvent::Boundary(Boundary::CommandEnd { exit_code: Some(0) }),
            ]
        );
    }

    #[test]
    fn handles_nonce_split_across_feeds() {
        let mut scanner = SentinelScanner::new("NONCE");
        let mut out = Vec::new();
        scanner.feed(b"out NO", &mut out);
        scanner.feed(b"NCE;3\n", &mut out);
        let merged: Vec<u8> = out
            .iter()
            .flat_map(|e| match e {
                ProcessorEvent::Output(b) => b.clone(),
                ProcessorEvent::Boundary(_) => Vec::new(),
            })
            .collect();
        assert_eq!(merged, b"out ");
        assert!(
            out.contains(&ProcessorEvent::Boundary(Boundary::CommandEnd {
                exit_code: Some(3)
            }))
        );
    }
}
