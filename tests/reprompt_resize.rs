//! kwi #48: the `/status` toggle resizes the PTY by one row, whose SIGWINCH
//! makes the shell repaint its prompt. A PTY probe of the real shells (fish
//! 3.7 + starship two-line prompt, bash 5.2) showed both repaint IN PLACE —
//! fish emits `\r ESC[A ESC[A ESC[J` then redraws, bash `\r ESC[K \r` then
//! redraws — so the grid must show exactly one prompt afterwards, not a
//! duplicate. These tests replay the probe's captured byte shapes against the
//! grid: shrink and grow by one row, then the shell's actual repaint sequence.

use kapollo::grid::Grid;

fn viewport_text(grid: &Grid) -> Vec<String> {
    grid.viewport_cells(0)
        .iter()
        .map(|row| row.concat().trim_end().to_string())
        .collect()
}

/// Paint scrollback filler, then a fish/starship-style two-line prompt at the
/// bottom: a blank separator, the info line, and the `λ` input line.
fn paint_history_and_prompt(grid: &mut Grid) {
    for i in 0..30 {
        grid.advance_bytes(format!("history-{i}\r\n").as_bytes());
    }
    // fish's initial prompt paint (probe shape): clear-below, blank line,
    // info line, then the input line with the cursor after "λ ".
    grid.advance_bytes(
        b"\r\x1b[J\x1b[K\r\nPROMPT-INFO-LINE\x1b[K\r\n\xce\xbb \x1b[K\r\x1b[C\x1b[C",
    );
}

/// fish's SIGWINCH repaint (probe shape): up two rows from the input line,
/// clear below, repaint the blank separator + both prompt lines.
const FISH_REPAINT: &[u8] =
    b"\r\x1b[A\x1b[A\x1b[J\x1b[K\r\nPROMPT-INFO-LINE\x1b[K\r\n\xce\xbb \x1b[K\r\x1b[C\x1b[C";

fn count_prompt_lines(grid: &Grid) -> usize {
    viewport_text(grid)
        .iter()
        .filter(|line| line.contains("PROMPT-INFO-LINE"))
        .count()
}

#[test]
fn one_row_shrink_then_fish_repaint_leaves_one_prompt() {
    // The `/status on` direction: the transcript pane loses one row.
    let mut grid = Grid::new(24, 80);
    paint_history_and_prompt(&mut grid);
    assert_eq!(count_prompt_lines(&grid), 1);

    grid.resize(23, 80);
    grid.advance_bytes(FISH_REPAINT);

    assert_eq!(
        count_prompt_lines(&grid),
        1,
        "repaint must not duplicate the prompt: {:?}",
        viewport_text(&grid)
    );
    // The input line is still the λ line, directly below the info line.
    let rows = viewport_text(&grid);
    let info = rows.iter().position(|l| l.contains("PROMPT-INFO-LINE"));
    let info = info.expect("prompt visible");
    assert_eq!(rows[info + 1], "λ", "input line follows the info line");
}

#[test]
fn one_row_grow_then_fish_repaint_leaves_one_prompt() {
    // The `/status off` direction: the transcript pane gains one row.
    let mut grid = Grid::new(23, 80);
    paint_history_and_prompt(&mut grid);
    assert_eq!(count_prompt_lines(&grid), 1);

    grid.resize(24, 80);
    grid.advance_bytes(FISH_REPAINT);

    assert_eq!(
        count_prompt_lines(&grid),
        1,
        "repaint must not duplicate the prompt: {:?}",
        viewport_text(&grid)
    );
}

#[test]
fn one_row_shrink_then_bash_repaint_leaves_one_prompt() {
    // bash's repaint (probe shape): `\r ESC[K \r` then the prompt again —
    // single-line, in place.
    let mut grid = Grid::new(24, 80);
    for i in 0..30 {
        grid.advance_bytes(format!("history-{i}\r\n").as_bytes());
    }
    grid.advance_bytes(b"bash-5.2$ ");

    grid.resize(23, 80);
    grid.advance_bytes(b"\r\x1b[K\rbash-5.2$ ");

    let rows = viewport_text(&grid);
    let prompts = rows.iter().filter(|l| l.contains("bash-5.2$")).count();
    assert_eq!(prompts, 1, "one prompt after repaint: {rows:?}");
}
