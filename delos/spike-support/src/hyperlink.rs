//! OSC 8 hyperlink framing. A spike uses this only to emit a single clickable link so we
//! can verify, by eye, that hyperlinks round-trip to the host terminal (alacritty/wezterm
//! both expose the *model* side via `cell.hyperlink()`; auto-wrapping raw URLs is a
//! kapollo-proper feature, not spike-worthy). Pure and unit-tested.

/// Frame `text` as an OSC 8 hyperlink pointing at `url`:
/// `ESC ] 8 ; ; <url> ST <text> ESC ] 8 ; ; ST`, where `ST` is `ESC \`.
pub fn osc8(url: &str, text: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_text_in_osc8_open_and_close() {
        let s = osc8("https://example.com", "link");
        assert_eq!(s, "\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\");
    }

    #[test]
    fn close_sequence_has_empty_url() {
        let s = osc8("https://x", "y");
        assert!(s.ends_with("\x1b]8;;\x1b\\"));
    }

    #[test]
    fn empty_url_and_text_still_balanced() {
        let s = osc8("", "");
        assert_eq!(s, "\x1b]8;;\x1b\\\x1b]8;;\x1b\\");
    }
}
