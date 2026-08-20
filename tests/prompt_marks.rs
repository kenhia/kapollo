//! OSC 133 `A`/`B` prompt capture (kwi #47): the injected fish/bash hooks
//! bracket the shell prompt, the processor diverts the span's normalized text
//! into a `ProcessorEvent::Prompt`, and block capture is unaffected. The live
//! cases are shell integration tests — hook installation and prompt rendering
//! cannot be unit-tested in isolation.

use std::time::{Duration, Instant};

use kapollo::output::{Boundary, OutputProcessor, ProcessorEvent};
use kapollo::pty::{PtyEvent, PtySession};

fn prompts(events: &[ProcessorEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            ProcessorEvent::Prompt(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn prompt_span_between_a_and_b_becomes_a_prompt_event() {
    let mut processor = OutputProcessor::osc133();
    let events = processor.process(b"\x1b]133;A\x1b\\ken@host /tmp> \x1b]133;B\x1b\\");
    assert_eq!(prompts(&events), vec!["ken@host /tmp> ".to_string()]);
    // The span's text is diverted: no Output event leaks it into a block.
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, ProcessorEvent::Output(_))),
        "prompt text must not appear as block output: {events:?}"
    );
}

#[test]
fn prompt_span_survives_chunk_splits_and_strips_escapes() {
    let mut processor = OutputProcessor::osc133();
    let mut events = processor.process(b"\x1b]133;A\x1b\\\x1b[32mken@ho");
    events.extend(processor.process(b"st\x1b[0m /tmp> \x1b]133;B\x1b\\"));
    assert_eq!(prompts(&events), vec!["ken@host /tmp> ".to_string()]);
}

#[test]
fn multiline_prompt_keeps_its_line_structure() {
    // A starship-style two-line prompt paints with \r\n; the normalizer keeps
    // the newline so the divider fold can split head from tail (kwi #47).
    let mut processor = OutputProcessor::osc133();
    let events = processor.process(b"\x1b]133;A\x1b\\info line\r\n\xce\xbb \x1b]133;B\x1b\\");
    assert_eq!(prompts(&events), vec!["info line\n\u{03bb} ".to_string()]);
}

#[test]
fn a_span_missing_its_b_mark_is_discarded_not_miscaptured() {
    // A lost `B` (e.g. a prompt manager clobbered PS1) must not divert command
    // output into the prompt buffer: `C` aborts the span.
    let mut processor = OutputProcessor::osc133();
    let events =
        processor.process(b"\x1b]133;A\x1b\\prompt$ echoed\n\x1b]133;C\x1b\\real output\n");
    assert!(prompts(&events).is_empty(), "no Prompt without B");
    let output: Vec<u8> = events
        .iter()
        .filter_map(|e| match e {
            ProcessorEvent::Output(d) => Some(d.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(output, b"real output\n", "block capture unaffected");
}

#[test]
fn repaint_overwrites_the_previous_span() {
    // A SIGWINCH repaint re-runs the prompt hooks: each A resets the span, so
    // the latest prompt wins.
    let mut processor = OutputProcessor::osc133();
    let events = processor
        .process(b"\x1b]133;A\x1b\\old> \x1b]133;B\x1b\\\x1b]133;A\x1b\\new> \x1b]133;B\x1b\\");
    assert_eq!(
        prompts(&events),
        vec!["old> ".to_string(), "new> ".to_string()],
        "each completed span is surfaced; callers keep the newest"
    );
}

// --- live shells ---

/// Drive a live shell until a Prompt event arrives (or the deadline passes),
/// returning the last one seen.
fn capture_live_prompt(shell: &str) -> Option<String> {
    let session = PtySession::spawn(Some(shell)).expect("spawn shell");
    let mut processor = OutputProcessor::for_mode(session.boundary_mode(), session.nonce());
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last = None;
    let mut settled = Instant::now();
    while Instant::now() < deadline && settled.elapsed() < Duration::from_millis(600) {
        match session.recv_timeout(Duration::from_millis(100)) {
            Ok(PtyEvent::Output(bytes)) => {
                for event in processor.process(&bytes) {
                    if let ProcessorEvent::Prompt(text) = event {
                        last = Some(text);
                        settled = Instant::now();
                    }
                }
            }
            Ok(PtyEvent::Exited(_)) => break,
            Err(_) => {}
        }
    }
    last
}

#[test]
fn live_shells_bracket_their_prompts_with_a_and_b() {
    let mut ran = 0;
    for sh in ["/usr/bin/fish", "/usr/bin/bash"] {
        if !std::path::Path::new(sh).exists() {
            eprintln!("skipping prompt marks: {sh} not installed");
            continue;
        }
        let prompt = capture_live_prompt(sh);
        let prompt = prompt.unwrap_or_else(|| panic!("{sh}: no Prompt event captured"));
        assert!(
            !prompt.trim().is_empty(),
            "{sh}: captured prompt should have visible text, got {prompt:?}"
        );
        // Sanity: no boundary escape bytes leaked into the normalized text.
        assert!(
            !prompt.contains('\u{1b}'),
            "{sh}: escapes leaked: {prompt:?}"
        );
        ran += 1;
    }
    assert!(ran > 0, "no shell available to exercise prompt marks");
}

#[test]
fn cwd_boundary_still_arrives_inside_the_prompt_span() {
    // OSC 7 is emitted between A and B by the fish hook; it must surface as a
    // boundary even while the prompt span is open.
    let mut processor = OutputProcessor::osc133();
    let events =
        processor.process(b"\x1b]133;A\x1b\\\x1b]7;file://host/tmp\x1b\\prompt> \x1b]133;B\x1b\\");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ProcessorEvent::Boundary(Boundary::Cwd(p)) if p.to_str() == Some("/tmp"))),
        "OSC 7 surfaces during the prompt span: {events:?}"
    );
    assert_eq!(prompts(&events), vec!["prompt> ".to_string()]);
}
