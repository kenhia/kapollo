//! Clipboard helpers: OSC 52 framing for terminal-mediated copy (the default path,
//! R3/FR-020) and an optional local `arboard` fallback (FR-021).

use base64::Engine as _;

/// Frame arbitrary bytes as an OSC 52 clipboard-set sequence:
/// `ESC ] 52 ; c ; <base64> ST`, where `ST` is `ESC \`. The selection target is the
/// system clipboard (`c`). Pure and unit-tested (FR-020, R3).
pub fn osc52_frame(data: &[u8]) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(data);
    format!("\x1b]52;c;{encoded}\x1b\\")
}

/// Optional local fallback: copy text directly to the OS clipboard via `arboard`
/// for hosts where OSC 52 is unavailable (FR-021).
pub fn copy_local(text: &str) -> anyhow::Result<()> {
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.set_text(text.to_owned())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_with_osc52_prefix_and_st_terminator() {
        let framed = osc52_frame(b"hi");
        assert!(framed.starts_with("\x1b]52;c;"));
        assert!(framed.ends_with("\x1b\\"));
    }

    #[test]
    fn encodes_payload_as_base64() {
        // "hello" -> aGVsbG8=
        assert_eq!(osc52_frame(b"hello"), "\x1b]52;c;aGVsbG8=\x1b\\");
    }

    #[test]
    fn empty_payload_yields_empty_base64() {
        assert_eq!(osc52_frame(b""), "\x1b]52;c;\x1b\\");
    }

    #[test]
    fn handles_non_ascii_bytes() {
        // UTF-8 for "é" = 0xC3 0xA9 -> base64 w6k=
        assert_eq!(osc52_frame("é".as_bytes()), "\x1b]52;c;w6k=\x1b\\");
    }
}
