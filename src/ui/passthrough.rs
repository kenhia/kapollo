//! Full-screen passthrough (User Story 3). When the wrapped program enters the
//! alternate screen (`vim`, `less`, `top`), kapollo suspends its own rendering
//! and capture, writes the program's raw bytes straight to the terminal, and
//! forwards keystrokes to the PTY verbatim. On exit the split-pad UI is
//! repainted with the transcript intact (FR-018, FR-019, FR-020).

use std::io::{self, Write};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Write raw program output to the terminal during passthrough, bypassing the
/// transcript pad so full-screen programs render natively (FR-018).
pub fn write_output(bytes: &[u8]) -> io::Result<()> {
    let mut out = io::stdout();
    out.write_all(bytes)?;
    out.flush()
}

/// Forward raw terminal input to the child verbatim during passthrough.
///
/// Unlike [`encode_key`], this performs no `KeyEvent` decoding/re-encoding, so
/// terminal query responses that arrive on stdin (e.g. an OSC 11
/// background-color report, a cursor-position report, or a Device Attributes
/// reply) reach the program intact instead of being mangled into spurious
/// visible input (FR-012).
pub fn forward_stdin(raw: &[u8]) -> &[u8] {
    raw
}

/// Explicit reset emitted to the host terminal when a full-screen program
/// leaves the alternate screen, before the split-pad UI is repainted (FR-013):
/// an SGR reset so no residual style bleeds through, and a show-cursor so a
/// program that hid the cursor cannot leave it hidden.
pub const RESET_SEQUENCE: &[u8] = b"\x1b[0m\x1b[?25h";

/// Emit [`RESET_SEQUENCE`] to stdout when leaving passthrough (FR-013).
pub fn reset_on_exit() -> io::Result<()> {
    let mut out = io::stdout();
    out.write_all(RESET_SEQUENCE)?;
    out.flush()
}

/// Toggle non-blocking mode on stdin so the passthrough loop can drain whatever
/// the terminal has sent without blocking, then hand it to the child verbatim
/// (FR-012). Enabled on entering passthrough and cleared on leaving so the
/// normal split-pad event reader keeps its blocking semantics.
#[cfg(unix)]
pub fn set_stdin_nonblocking(enable: bool) -> io::Result<()> {
    use std::os::unix::io::AsRawFd;
    let fd = io::stdin().as_raw_fd();
    // SAFETY: `fd` is the live stdin descriptor; fcntl with F_GETFL/F_SETFL only
    // reads and writes the descriptor's status flags.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        let new = if enable {
            flags | libc::O_NONBLOCK
        } else {
            flags & !libc::O_NONBLOCK
        };
        if libc::fcntl(fd, libc::F_SETFL, new) < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn set_stdin_nonblocking(_enable: bool) -> io::Result<()> {
    Ok(())
}

/// Read whatever bytes are currently available on stdin without blocking,
/// returning the number read (0 when nothing is pending). Requires stdin to be
/// in non-blocking mode (see [`set_stdin_nonblocking`]); used by the passthrough
/// loop to forward terminal input verbatim (FR-012).
pub fn read_available_stdin(buf: &mut [u8]) -> io::Result<usize> {
    use std::io::Read;
    match io::stdin().lock().read(buf) {
        Ok(n) => Ok(n),
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
        Err(e) => Err(e),
    }
}

/// Encode a key event into the byte sequence a terminal would send to the
/// program over the PTY. Returns `None` for keys with no passthrough mapping.
pub fn encode_key(key: KeyEvent) -> Option<Vec<u8>> {
    // Ctrl + letter maps to the corresponding control byte (e.g. Ctrl-C = 0x03,
    // Ctrl-D = 0x04), which is how interactive programs receive those chords.
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = key.code {
            let upper = c.to_ascii_uppercase();
            if upper.is_ascii_uppercase() {
                return Some(vec![(upper as u8) - b'A' + 1]);
            }
        }
    }

    match key.code {
        KeyCode::Char(c) => {
            let mut buf = [0u8; 4];
            Some(c.encode_utf8(&mut buf).as_bytes().to_vec())
        }
        KeyCode::Enter => Some(vec![b'\r']),
        KeyCode::Tab => Some(vec![b'\t']),
        KeyCode::BackTab => Some(b"\x1b[Z".to_vec()),
        KeyCode::Backspace => Some(vec![0x7f]),
        KeyCode::Esc => Some(vec![0x1b]),
        KeyCode::Up => Some(b"\x1b[A".to_vec()),
        KeyCode::Down => Some(b"\x1b[B".to_vec()),
        KeyCode::Right => Some(b"\x1b[C".to_vec()),
        KeyCode::Left => Some(b"\x1b[D".to_vec()),
        KeyCode::Home => Some(b"\x1b[H".to_vec()),
        KeyCode::End => Some(b"\x1b[F".to_vec()),
        KeyCode::PageUp => Some(b"\x1b[5~".to_vec()),
        KeyCode::PageDown => Some(b"\x1b[6~".to_vec()),
        KeyCode::Delete => Some(b"\x1b[3~".to_vec()),
        KeyCode::Insert => Some(b"\x1b[2~".to_vec()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn printable_chars_pass_through_verbatim() {
        assert_eq!(encode_key(key(KeyCode::Char('q'))), Some(b"q".to_vec()));
    }

    #[test]
    fn enter_becomes_carriage_return() {
        assert_eq!(encode_key(key(KeyCode::Enter)), Some(vec![b'\r']));
    }

    #[test]
    fn arrows_become_csi_sequences() {
        assert_eq!(encode_key(key(KeyCode::Up)), Some(b"\x1b[A".to_vec()));
        assert_eq!(encode_key(key(KeyCode::Left)), Some(b"\x1b[D".to_vec()));
    }

    #[test]
    fn ctrl_letters_become_control_bytes() {
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(encode_key(ctrl_c), Some(vec![0x03]));
    }
}
