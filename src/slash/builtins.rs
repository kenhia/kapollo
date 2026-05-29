//! Built-in slash commands: the text and side-effect-free rendering helpers for
//! `/help`, `/clear`, and `/quit` (FR-023, contracts/slash-commands.md). The
//! actual state changes (clearing the transcript, quitting) are applied by the
//! event loop; this module owns only the user-facing text.

/// The `/help` body, parameterized by the active leader char so it stays
/// accurate when the leader is remapped.
pub fn help_text(leader: char) -> String {
    format!(
        "kapollo slash commands (leader '{leader}'):\n\
         \x20 {leader}help     Show this help.\n\
         \x20 {leader}clear    Clear the visible transcript.\n\
         \x20 {leader}quit     Exit kapollo and restore the terminal.\n\
         \n\
         Keys:\n\
         \x20 Enter            Submit the current input.\n\
         \x20 Ctrl-C           Interrupt the running command (not kapollo).\n\
         \n\
         To send literal input beginning with '{leader}', double it \
         (e.g. '{leader}{leader}path')."
    )
}

/// The error body shown when an unknown slash command is entered.
pub fn unknown_text(name: &str, leader: char) -> String {
    format!("Unknown command '{leader}{name}'. Try '{leader}help'.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_text_mentions_each_command() {
        let text = help_text('/');
        assert!(text.contains("/help"));
        assert!(text.contains("/clear"));
        assert!(text.contains("/quit"));
    }

    #[test]
    fn unknown_text_names_the_command_and_suggests_help() {
        let text = unknown_text("bogus", '/');
        assert!(text.contains("/bogus"));
        assert!(text.contains("/help"));
    }
}
