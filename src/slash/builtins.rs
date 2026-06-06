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
         \x20 {leader}status   Toggle the fixed status bar.\n\
         \x20 {leader}keys     List the active key bindings.\n\
         \x20 {leader}reload-config  Re-read the config file (keymap included).\n\
         \x20 {leader}quit     Exit kapollo and restore the terminal.\n\
         \x20 {leader}exit     Alias for {leader}quit.\n\
         \n\
         Keys (run '{leader}keys' for the full list):\n\
         \x20 Enter            Submit the current input.\n\
         \x20 Ctrl-C           Interrupt the running command (not kapollo).\n\
         \x20 PageUp/PageDown  Scroll the transcript a page at a time.\n\
         \x20 Shift+Home/End   Jump to the oldest / newest output.\n\
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
    fn help_text_mentions_reload_config() {
        assert!(help_text('/').contains("/reload-config"));
    }

    #[test]
    fn unknown_text_names_the_command_and_suggests_help() {
        let text = unknown_text("bogus", '/');
        assert!(text.contains("/bogus"));
        assert!(text.contains("/help"));
    }
}
