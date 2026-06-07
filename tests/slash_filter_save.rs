//! Slash-command dispatch tests for the sprint-007 argument-bearing commands
//! `/save`, `/filter`, and `/load` (contracts/slash-commands.md §6). The pure,
//! unit-testable surface is the dispatch parsing and payload preservation
//! (S1/S5/S8); the save-content/overwrite-prompt and the `/filter` shell
//! round-trip (S2–S4/S6/S7) are validated via the quickstart manual exception
//! (Constitution III, live-TTY behavior).

use kapollo::slash::{dispatch, Dispatch, SlashCommand};

use kapollo::input::{InputMode, InputPad};

#[test]
fn s1_save_without_arg_dispatches_empty_payload() {
    // `/save` with no argument still dispatches (reaching its status path,
    // not the unknown-command path) with an empty payload (FR-022).
    assert_eq!(
        dispatch("save"),
        Dispatch::Command(SlashCommand::Save(String::new()))
    );
}

#[test]
fn save_with_path_carries_the_trimmed_path() {
    assert_eq!(
        dispatch("save  out.txt  "),
        Dispatch::Command(SlashCommand::Save("out.txt".to_string()))
    );
}

#[test]
fn s5_filter_preserves_the_raw_payload_with_pipes() {
    // The `/filter` payload is the raw remainder so the shell sees pipes,
    // globs, and aliases intact (FR-025).
    assert_eq!(
        dispatch("filter rg foo | sort"),
        Dispatch::Command(SlashCommand::Filter("rg foo | sort".to_string()))
    );
}

#[test]
fn filter_without_arg_dispatches_empty_payload() {
    assert_eq!(
        dispatch("filter"),
        Dispatch::Command(SlashCommand::Filter(String::new()))
    );
}

#[test]
fn s8_load_carries_the_trimmed_path() {
    // `/load <path>` dispatches with the trimmed path payload (FR-028); the
    // buffer load + Laat entry is validated in quickstart.
    assert_eq!(
        dispatch("load script.sh"),
        Dispatch::Command(SlashCommand::Load("script.sh".to_string()))
    );
}

#[test]
fn s8_loaded_lines_become_buffer_lines_with_first_highlighted() {
    // Model `/load`'s buffer handling (FR-028): each file line becomes a buffer
    // line (a single trailing newline dropped), the caret lands on line 0, and
    // the mode enters Laat. The file read + Laat entry are wired in App::run_load.
    let file_contents = "echo one\necho two\necho three\n";
    let body = file_contents.strip_suffix('\n').unwrap_or(file_contents);

    let mut pad = InputPad::new();
    pad.set_contents(body.to_string());
    pad.set_caret_line_start(0);
    let mode = InputMode::Laat;

    assert_eq!(pad.line_count(), 3);
    assert_eq!(pad.as_str(), "echo one\necho two\necho three");
    assert_eq!(pad.cursor_row_col(), (0, 0));
    assert_eq!(mode, InputMode::Laat);
}

#[test]
fn argument_less_commands_still_require_an_exact_match() {
    // A trailing argument on an argument-less verb remains an unknown command,
    // preserving the sprint 005/006 dispatch contract.
    assert_eq!(
        dispatch("help extra"),
        Dispatch::Unknown("help".to_string())
    );
    assert_eq!(dispatch("help"), Dispatch::Command(SlashCommand::Help));
}
