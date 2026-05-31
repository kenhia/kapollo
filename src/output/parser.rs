//! OSC 133 + alt-screen parsing via `vte`. The parser turns the raw PTY byte
//! stream into ordered [`ProcessorEvent`]s: spans of decoded output interleaved
//! with [`Boundary`] marks. Color/styling CSI sequences are dropped for the
//! MVP's plain-text rendering (research R2, R4).

use vte::{Params, Parser, Perform};

use std::path::PathBuf;

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
    /// OSC 7: the shell reported a new working directory (FR-019).
    Cwd(PathBuf),
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
        // Normalize captured output to clean printable text (FR-001): keep only
        // newline and tab. Bare carriage returns have no grid model to honor and
        // would otherwise render as visible artifacts (and turn `\r\n` into a
        // stray control byte), so they are dropped along with all other C0
        // controls. OSC/CSI/DCS escape sequences are consumed by `vte` and never
        // reach `execute`/`print`, so residual styling never leaks as text.
        if matches!(byte, b'\n' | b'\t') {
            self.pending.push(byte);
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        match params.first().copied() {
            Some(b"133") => match params.get(1).copied() {
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
            },
            // OSC 7: `file://host/abs-path` cwd report (FR-019).
            Some(b"7") => {
                if let Some(path) = params.get(1).and_then(|p| parse_osc7_cwd(p)) {
                    self.boundary(Boundary::Cwd(path));
                }
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

/// Parse an OSC 7 `file://host/abs-path` payload into the absolute working
/// directory it reports, dropping the `file://` scheme and host and
/// percent-decoding the path (FR-019). Returns `None` for payloads that are not
/// a well-formed `file://` URI with an absolute path.
fn parse_osc7_cwd(raw: &[u8]) -> Option<PathBuf> {
    let s = std::str::from_utf8(raw).ok()?;
    let rest = s.strip_prefix("file://")?;
    // After the scheme comes an optional host, then the absolute path beginning
    // at the first `/`.
    let slash = rest.find('/')?;
    let decoded = percent_decode(&rest[slash..]);
    Some(PathBuf::from(decoded))
}

/// Minimal percent-decoding for OSC 7 paths (`%20` → space, etc.). Invalid or
/// truncated escapes are passed through literally.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
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
