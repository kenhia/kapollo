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
