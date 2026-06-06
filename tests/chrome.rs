//! Chrome redesign tests (US2): borderless transcript, a blank line between
//! blocks, a colorized `λ` prompt prefix, and the split-pad layout with the
//! fixed status bar pinned beneath the input (FR-005…FR-011, FR-017).

use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::widgets::Paragraph;
use ratatui::Terminal;

use kapollo::session::Block;
use kapollo::ui::transcript;

fn block(id: u64, command: &str, output: &[u8], exit: Option<i32>) -> Block {
    let mut b = Block::new(id, command.to_string(), 1 << 20, 50_000);
    b.push_output(output);
    b.close(exit);
    b
}

// --- Layout: transcript, dividing rule, input, fixed bottom status bar ---

#[test]
fn full_chrome_stacks_transcript_divider_input_status() {
    let area = ratatui::layout::Rect::new(0, 0, 80, 24);
    let layout = kapollo::ui::chrome_layout(area, 3, true, true);
    let divider_rect = layout.divider.expect("divider present when enabled");
    let status_rect = layout.status.expect("status present when enabled");

    // transcript → divider → input → status, each directly below the last.
    assert_eq!(
        layout.transcript.y + layout.transcript.height,
        divider_rect.y,
        "divider must sit directly below the transcript"
    );
    assert_eq!(divider_rect.height, 1, "divider is a single line");
    assert_eq!(
        divider_rect.y + divider_rect.height,
        layout.input.y,
        "input must sit directly below the divider"
    );
    assert_eq!(
        layout.input.y + layout.input.height,
        status_rect.y,
        "status bar must sit directly below the input"
    );
    assert_eq!(
        status_rect.y + status_rect.height,
        area.y + area.height,
        "status bar must be pinned to the very bottom"
    );
    assert_eq!(status_rect.height, 1, "status bar must be a single line");
    assert_eq!(layout.input.height, 3, "input height is passed through");
}

#[test]
fn divider_and_status_are_omitted_when_disabled() {
    let area = ratatui::layout::Rect::new(0, 0, 80, 24);
    let layout = kapollo::ui::chrome_layout(area, 3, false, false);
    assert!(layout.divider.is_none(), "no divider when disabled");
    assert!(layout.status.is_none(), "no status when disabled");
    assert_eq!(
        layout.transcript.y + layout.transcript.height,
        layout.input.y,
        "input sits directly below the transcript when both are off"
    );
    assert_eq!(
        layout.input.y + layout.input.height,
        area.y + area.height,
        "input runs to the bottom when no status bar"
    );
}

#[test]
fn transcript_has_no_border() {
    let blocks = vec![block(1, "ls", b"file.txt\n", Some(0))];
    let lines = transcript::lines(&blocks, 'λ', Color::Red, true);
    let para = Paragraph::new(lines);

    let backend = TestBackend::new(20, 6);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| f.render_widget(&para, f.area())).unwrap();
    let buffer = terminal.backend().buffer().clone();

    // No box-drawing border characters anywhere in the transcript render.
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let sym = buffer[(x, y)].symbol();
            assert!(
                !matches!(sym, "│" | "─" | "┌" | "┐" | "└" | "┘"),
                "found border char {sym:?} at ({x},{y})"
            );
        }
    }
}

// --- Command echo: λ prefix, colorized (FR-010/011) ---

#[test]
fn command_echo_uses_prompt_char_not_dollar() {
    let blocks = vec![block(1, "echo hi", b"hi\n", Some(0))];
    let lines = transcript::lines(&blocks, 'λ', Color::Red, true);
    let first = lines[0].to_string();
    assert!(
        first.starts_with('λ'),
        "command not prefixed with λ: {first:?}"
    );
    assert!(
        !first.contains('$'),
        "legacy $ prompt still present: {first:?}"
    );
}

#[test]
fn prompt_char_is_configurable() {
    let blocks = vec![block(1, "echo hi", b"hi\n", Some(0))];
    let lines = transcript::lines(&blocks, '❯', Color::Cyan, true);
    assert!(lines[0].to_string().starts_with('❯'));
}

#[test]
fn prompt_char_is_colorized_when_color_enabled() {
    let blocks = vec![block(1, "echo hi", b"hi\n", Some(0))];
    let lines = transcript::lines(&blocks, 'λ', Color::Red, true);
    // The first span carries the prompt char and must wear the prompt color.
    let span = &lines[0].spans[0];
    assert!(span.content.contains('λ'));
    assert_eq!(span.style.fg, Some(Color::Red));
}

#[test]
fn prompt_char_is_unstyled_when_color_disabled() {
    let blocks = vec![block(1, "echo hi", b"hi\n", Some(0))];
    let lines = transcript::lines(&blocks, 'λ', Color::Red, false);
    let span = &lines[0].spans[0];
    assert!(span.content.contains('λ'));
    assert_eq!(
        span.style.fg, None,
        "color must be suppressed under NO_COLOR"
    );
}

// --- Blank line between blocks (FR-009) ---

#[test]
fn blank_line_separates_consecutive_blocks() {
    let blocks = vec![
        block(1, "first", b"a\n", Some(0)),
        block(2, "second", b"b\n", Some(0)),
    ];
    let rendered: Vec<String> = transcript::lines(&blocks, 'λ', Color::Red, true)
        .iter()
        .map(|l| l.to_string())
        .collect();
    // Expect: "λfirst", "a", "", "λsecond", "b"
    let blank_positions: Vec<usize> = rendered
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim().is_empty())
        .map(|(i, _)| i)
        .collect();
    assert!(
        !blank_positions.is_empty(),
        "no blank line separating blocks: {rendered:?}"
    );
    // The blank line must come between the two blocks' content.
    let second_cmd = rendered.iter().position(|l| l.contains("second")).unwrap();
    assert!(
        blank_positions.iter().any(|&p| p < second_cmd && p > 0),
        "blank line not positioned between blocks: {rendered:?}"
    );
}

// --- Tab expansion: output tabs become spaces, not raw \t (FR-001/002) ---

#[test]
fn output_tabs_are_expanded_to_spaces() {
    // ratatui would emit a literal tab byte to the host terminal for a stored
    // `\t`, corrupting later rows. The renderer must expand tabs to spaces.
    let blocks = vec![block(1, "cat data", b"one\ttwo\tthree\n", Some(0))];
    let rendered: Vec<String> = transcript::lines(&blocks, 'λ', Color::Red, true)
        .iter()
        .map(|l| l.to_string())
        .collect();
    let output_line = rendered
        .iter()
        .find(|l| l.contains("one"))
        .expect("output line missing");
    assert!(
        !output_line.contains('\t'),
        "raw tab leaked into render: {output_line:?}"
    );
    // `one` (3 cols) → tab stop 8 → `one` + 5 spaces + `two`.
    assert_eq!(output_line, "one     two     three");
}
