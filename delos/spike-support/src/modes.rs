//! Stream-level detection of DEC private mode changes that matter to the spike:
//! alt-screen enter/exit and child-driven mouse modes. Parsing the byte stream
//! (rather than relying on each crate's flags) keeps mode routing identical across
//! all three stages (R4, FR-013/FR-014). Pure and unit-tested.

/// A mode transition observed in the child's output byte stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeEvent {
    /// Child entered the alternate screen (`?1049h`, legacy `?47h`/`?1047h`).
    AltScreenEnter,
    /// Child left the alternate screen (`?1049l`, legacy `?47l`/`?1047l`).
    AltScreenExit,
    /// Child enabled a mouse-tracking mode (e.g. `?1000h`, `?1002h`, `?1003h`, `?1006h`).
    MouseEnable(u16),
    /// Child disabled a mouse-tracking mode.
    MouseDisable(u16),
}

/// Scan a byte stream and return the alt-screen / mouse-mode transitions it contains,
/// in order. Recognizes CSI DEC-private set/reset sequences `ESC [ ? <params> (h|l)`,
/// where `<params>` may carry several `;`-separated mode numbers.
pub fn detect_mode(bytes: &[u8]) -> Vec<ModeEvent> {
    let mut events = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        // Looking for the start of a CSI sequence: ESC '['.
        if bytes[i] != 0x1b || i + 1 >= bytes.len() || bytes[i + 1] != b'[' {
            i += 1;
            continue;
        }
        let mut j = i + 2;
        // DEC private marker.
        if j >= bytes.len() || bytes[j] != b'?' {
            i += 1;
            continue;
        }
        j += 1;
        let params_start = j;
        while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == b';') {
            j += 1;
        }
        if j >= bytes.len() || (bytes[j] != b'h' && bytes[j] != b'l') {
            i += 1;
            continue;
        }
        let set = bytes[j] == b'h';
        let params = &bytes[params_start..j];
        for part in params.split(|&c| c == b';') {
            if part.is_empty() {
                continue;
            }
            let Ok(text) = std::str::from_utf8(part) else {
                continue;
            };
            let Ok(mode) = text.parse::<u16>() else {
                continue;
            };
            match mode {
                1049 | 47 | 1047 => events.push(if set {
                    ModeEvent::AltScreenEnter
                } else {
                    ModeEvent::AltScreenExit
                }),
                1000 | 1002 | 1003 | 1006 => events.push(if set {
                    ModeEvent::MouseEnable(mode)
                } else {
                    ModeEvent::MouseDisable(mode)
                }),
                _ => {}
            }
        }
        i = j + 1;
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_alt_screen_enter_and_exit() {
        assert_eq!(detect_mode(b"\x1b[?1049h"), vec![ModeEvent::AltScreenEnter]);
        assert_eq!(detect_mode(b"\x1b[?1049l"), vec![ModeEvent::AltScreenExit]);
    }

    #[test]
    fn detects_legacy_alt_screen_modes() {
        assert_eq!(detect_mode(b"\x1b[?47h"), vec![ModeEvent::AltScreenEnter]);
        assert_eq!(detect_mode(b"\x1b[?1047l"), vec![ModeEvent::AltScreenExit]);
    }

    #[test]
    fn detects_mouse_modes() {
        assert_eq!(
            detect_mode(b"\x1b[?1000h"),
            vec![ModeEvent::MouseEnable(1000)]
        );
        assert_eq!(
            detect_mode(b"\x1b[?1006l"),
            vec![ModeEvent::MouseDisable(1006)]
        );
    }

    #[test]
    fn detects_multiple_params_in_one_sequence() {
        assert_eq!(
            detect_mode(b"\x1b[?1002;1006h"),
            vec![ModeEvent::MouseEnable(1002), ModeEvent::MouseEnable(1006)]
        );
    }

    #[test]
    fn detects_events_interleaved_with_text() {
        let stream = b"hello\x1b[?1049hworld\x1b[?1000h\x1b[?1049lbye";
        assert_eq!(
            detect_mode(stream),
            vec![
                ModeEvent::AltScreenEnter,
                ModeEvent::MouseEnable(1000),
                ModeEvent::AltScreenExit,
            ]
        );
    }

    #[test]
    fn ignores_non_private_and_unknown_modes() {
        assert_eq!(detect_mode(b"\x1b[2J"), vec![]); // not DEC-private
        assert_eq!(detect_mode(b"\x1b[?25h"), vec![]); // cursor visibility, ignored
        assert_eq!(detect_mode(b"plain text"), vec![]);
    }

    #[test]
    fn ignores_truncated_sequences() {
        assert_eq!(detect_mode(b"\x1b[?1049"), vec![]); // no final h/l
        assert_eq!(detect_mode(b"\x1b[?"), vec![]);
        assert_eq!(detect_mode(b"\x1b"), vec![]);
    }
}
