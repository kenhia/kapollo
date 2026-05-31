//! Input router tests (T030): slash detection, doubled-leader escape, and
//! non-slash passthrough (FR-021, FR-022).

use kapollo::input::router::{route, Routed};
use kapollo::slash::builtins::help_text;
use kapollo::slash::{dispatch, Dispatch, SlashCommand};

#[test]
fn slash_prefixed_input_is_a_command() {
    assert_eq!(route("/help", '/'), Routed::Slash("help".to_string()));
    assert_eq!(route("/quit", '/'), Routed::Slash("quit".to_string()));
}

#[test]
fn doubled_leader_is_escaped_to_literal_shell_input() {
    // `//ls` should run `/ls` in the shell, not a slash command.
    assert_eq!(route("//ls", '/'), Routed::Shell("/ls".to_string()));
}

#[test]
fn plain_input_is_passed_through_to_the_shell() {
    assert_eq!(
        route("git status", '/'),
        Routed::Shell("git status".to_string())
    );
}

#[test]
fn respects_a_configured_leader_char() {
    assert_eq!(route(":help", ':'), Routed::Slash("help".to_string()));
    // A `/`-prefixed path is literal shell input when the leader is `:`.
    assert_eq!(
        route("/usr/bin", ':'),
        Routed::Shell("/usr/bin".to_string())
    );
}

#[test]
fn exit_is_an_alias_for_quit() {
    assert_eq!(dispatch("exit"), Dispatch::Command(SlashCommand::Quit));
    assert_eq!(dispatch("quit"), Dispatch::Command(SlashCommand::Quit));
}

#[test]
fn help_lists_exit_and_the_scrolling_keys() {
    let text = help_text('/');
    assert!(text.contains("/exit"), "help lists the /exit alias");
    assert!(text.contains("PageUp"));
    assert!(text.contains("PageDown"));
    assert!(text.contains("Home"));
    assert!(text.contains("End"));
}
