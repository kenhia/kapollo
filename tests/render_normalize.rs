//! Render-pipeline normalization tests (US1): the captured block text must be
//! clean printable text — no bare carriage returns, OSC responses, or residual
//! escape sequences leaking through as visible characters (FR-001, FR-002,
//! FR-004).

use kapollo::config::Caps;
use kapollo::output::OutputProcessor;
use kapollo::session::Transcript;

/// Capture the normalized output of a single OSC 133-delimited block.
fn capture(payload: &[u8]) -> String {
    let mut transcript = Transcript::new(Caps::default());
    let id = transcript.begin_block("cmd".to_string());
    let mut current = Some(id);

    let mut processor = OutputProcessor::osc133();
    processor.begin_command();

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"\x1b]133;C\x07");
    bytes.extend_from_slice(payload);
    bytes.extend_from_slice(b"\x1b]133;D;0\x07");
    processor.apply(&bytes, &mut transcript, &mut current);

    transcript.blocks()[0].output_lossy()
}

#[test]
fn strips_bare_carriage_returns() {
    // A bare CR (progress-bar style overwrite) has no grid model to honor and
    // must be dropped, not rendered as a control character.
    let out = capture(b"loading\rdone\n");
    assert!(!out.contains('\r'), "bare CR leaked: {out:?}");
    assert_eq!(out, "loadingdone\n");
}

#[test]
fn collapses_crlf_to_lf() {
    let out = capture(b"line one\r\nline two\r\n");
    assert!(!out.contains('\r'), "CR leaked: {out:?}");
    assert_eq!(out, "line one\nline two\n");
}

#[test]
fn swallows_osc_color_query_response() {
    // A terminal OSC 11 color-query *response* must never appear as visible
    // text in the captured block (the user-test `]11;rgb:...` bug).
    let out = capture(b"before\x1b]11;rgb:2020/2020/2020\x07after\n");
    assert!(!out.contains("]11"), "OSC response leaked: {out:?}");
    assert!(!out.contains("rgb:"), "OSC response leaked: {out:?}");
    assert_eq!(out, "beforeafter\n");
}

#[test]
fn swallows_residual_sgr_escapes() {
    // SGR color sequences are stripped (no styling rendered this sprint).
    let out = capture(b"\x1b[31mred\x1b[0m text\n");
    assert!(!out.contains('\x1b'), "escape leaked: {out:?}");
    assert!(!out.contains("[31m"), "SGR leaked: {out:?}");
    assert_eq!(out, "red text\n");
}

#[test]
fn preserves_tabs_and_printable_text() {
    let out = capture(b"col1\tcol2\tcol3\n");
    assert_eq!(out, "col1\tcol2\tcol3\n");
}

#[test]
fn output_contains_only_printable_plus_lf_and_tab() {
    let out = capture(b"a\rb\x1b]11;rgb:0/0/0\x07\tc\r\nd\x1b[1me\n");
    for ch in out.chars() {
        assert!(
            ch == '\n' || ch == '\t' || !ch.is_control(),
            "non-printable control char {ch:?} in normalized output {out:?}"
        );
    }
}

#[test]
fn first_line_normalizes_identically_to_later_lines() {
    // The first line must not be corrupted differently from subsequent lines
    // (FR-004): same input shape on every line yields the same output shape.
    let out = capture(b"\x1b[32malpha\x1b[0m\r\n\x1b[32mbeta\x1b[0m\r\n");
    assert_eq!(out, "alpha\nbeta\n");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines, vec!["alpha", "beta"]);
}
