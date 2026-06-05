//! Clipboard helpers: OSC 52 framing for terminal-mediated copy (the default
//! path, SSH-friendly) and an optional local `arboard` fallback for hosts where
//! OSC 52 is unavailable (D28, FR-020/FR-021).
//!
//! Promoted from the 003 spike (`spike-support::clipboard`).

use base64::Engine as _;

/// Frame arbitrary bytes as an OSC 52 clipboard-set sequence:
/// `ESC ] 52 ; c ; <base64> ST`, where `ST` is `ESC \`. The selection target is
/// the system clipboard (`c`). Pure and unit-tested (FR-020).
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

/// How a copy was carried out, so the caller can act on it (OSC 52 must be
/// written to the terminal; local copy is already done).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyMethod {
    /// Emit these bytes to the terminal to set the clipboard via OSC 52.
    Osc52(String),
    /// The text was placed on the local OS clipboard via `arboard`.
    Local,
}

/// Copy `text` to the clipboard, preferring OSC 52 (terminal-mediated,
/// SSH-friendly) and falling back to the local OS clipboard, per the enabled
/// methods (FR-010/011). Returns the method used so the caller can flush OSC 52
/// bytes to the terminal. Errors only when every enabled method fails or none is
/// enabled, so the caller can surface a visible notice — never a silent drop
/// (FR-013).
pub fn copy(text: &str, osc52: bool, local_fallback: bool) -> anyhow::Result<CopyMethod> {
    if osc52 {
        return Ok(CopyMethod::Osc52(osc52_frame(text.as_bytes())));
    }
    if local_fallback {
        copy_local(text)?;
        return Ok(CopyMethod::Local);
    }
    anyhow::bail!("no clipboard method enabled")
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

    #[test]
    fn copy_prefers_osc52_when_enabled() {
        let method = copy("hello", true, true).unwrap();
        assert_eq!(method, CopyMethod::Osc52(osc52_frame(b"hello")));
    }

    #[test]
    fn copy_errors_when_no_method_enabled() {
        assert!(copy("hello", false, false).is_err());
    }
}
