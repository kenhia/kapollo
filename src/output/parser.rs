//! OSC 133 + alt-screen parsing via `vte`. The parser turns the raw PTY byte
//! stream into ordered [`ProcessorEvent`]s: spans of decoded output interleaved
//! with [`Boundary`] marks. Color/styling CSI sequences are dropped for the
//! MVP's plain-text rendering (research R2, R4).

use vte::{Params, Parser, Perform};

/// A block-boundary or screen-mode signal extracted from the byte stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Boundary {
    /// OSC 133 `A`: prompt start.
    PromptStart,
    /// OSC 133 `B`: command input accepted.
    CommandStart,
    /// OSC 133 `C`: command output starts here.
    OutputStart,
    /// OSC 133 `D;<exit>`: command finished with the given exit code.
    CommandEnd { exit_code: Option<i32> },
    /// Entered the alternate screen (e.g. `vim`, `less`); capture suspends.
    AltScreenEnter,
    /// Left the alternate screen; the split UI resumes.
    AltScreenLeave,
}

/// One unit of parsed PTY output, in stream order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessorEvent {
    /// A span of decoded output bytes belonging to the current block.
    Output(Vec<u8>),
    /// A boundary/mode mark.
    Boundary(Boundary),
}

/// Incremental OSC 133 parser. Maintains `vte` state across `feed` calls so
/// escape sequences and multi-byte UTF-8 split across PTY reads are handled.
#[derive(Default)]
pub struct Osc133Parser {
    parser: Parser,
}

impl Osc133Parser {
    /// Create a fresh parser.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed `bytes`, appending parsed events (in order) to `out`.
    pub fn feed(&mut self, bytes: &[u8], out: &mut Vec<ProcessorEvent>) {
        let mut performer = Performer {
            out,
            pending: Vec::new(),
        };
        for &byte in bytes {
            self.parser.advance(&mut performer, byte);
        }
        performer.flush();
    }
}

struct Performer<'a> {
    out: &'a mut Vec<ProcessorEvent>,
    pending: Vec<u8>,
}

impl Performer<'_> {
    fn flush(&mut self) {
        if !self.pending.is_empty() {
            self.out
                .push(ProcessorEvent::Output(std::mem::take(&mut self.pending)));
        }
    }

    fn boundary(&mut self, boundary: Boundary) {
        self.flush();
        self.out.push(ProcessorEvent::Boundary(boundary));
    }
}

impl Perform for Performer<'_> {
    fn print(&mut self, c: char) {
        let mut buf = [0u8; 4];
        self.pending
            .extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
    }

    fn execute(&mut self, byte: u8) {
        // Preserve whitespace control bytes (newline, carriage return, tab)
        // that shape the rendered output.
        if matches!(byte, b'\n' | b'\r' | b'\t') {
            self.pending.push(byte);
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.first().copied() != Some(b"133".as_ref()) {
            return;
        }
        match params.get(1).copied() {
            Some(b"A") => self.boundary(Boundary::PromptStart),
            Some(b"B") => self.boundary(Boundary::CommandStart),
            Some(b"C") => self.boundary(Boundary::OutputStart),
            Some(b"D") => {
                let exit_code = params
                    .get(2)
                    .and_then(|p| std::str::from_utf8(p).ok())
                    .and_then(|s| s.trim().parse::<i32>().ok());
                self.boundary(Boundary::CommandEnd { exit_code });
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, c: char) {
        if intermediates != [b'?'] || !matches!(c, 'h' | 'l') {
            return;
        }
        let mode = params.iter().next().and_then(|sub| sub.first().copied());
        if matches!(mode, Some(1049) | Some(1047) | Some(47)) {
            if c == 'h' {
                self.boundary(Boundary::AltScreenEnter);
            } else {
                self.boundary(Boundary::AltScreenLeave);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(bytes: &[u8]) -> Vec<ProcessorEvent> {
        let mut parser = Osc133Parser::new();
        let mut out = Vec::new();
        parser.feed(bytes, &mut out);
        out
    }

    #[test]
    fn delimits_output_between_osc133_marks() {
        let events = parse(b"\x1b]133;C\x07hello\n\x1b]133;D;0\x07");
        assert_eq!(
            events,
            vec![
                ProcessorEvent::Boundary(Boundary::OutputStart),
                ProcessorEvent::Output(b"hello\n".to_vec()),
                ProcessorEvent::Boundary(Boundary::CommandEnd { exit_code: Some(0) }),
            ]
        );
    }

    #[test]
    fn captures_nonzero_exit_code() {
        let events = parse(b"\x1b]133;D;7\x1b\\");
        assert_eq!(
            events,
            vec![ProcessorEvent::Boundary(Boundary::CommandEnd {
                exit_code: Some(7)
            })]
        );
    }

    #[test]
    fn detects_alt_screen_enter_and_leave() {
        let events = parse(b"\x1b[?1049h\x1b[?1049l");
        assert_eq!(
            events,
            vec![
                ProcessorEvent::Boundary(Boundary::AltScreenEnter),
                ProcessorEvent::Boundary(Boundary::AltScreenLeave),
            ]
        );
    }
}
