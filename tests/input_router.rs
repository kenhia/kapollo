//! Input router tests (T030): slash detection, doubled-leader escape, and
//! non-slash passthrough (FR-021, FR-022).

use kapollo::input::router::{route, Routed};

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
