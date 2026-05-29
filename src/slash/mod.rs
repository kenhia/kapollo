//! Slash-command layer: exact-match dispatch of leader-prefixed commands
//! intercepted by the input router (FR-023, contracts/slash-commands.md). The
//! registry is intentionally tiny for the MVP; post-MVP commands slot in
//! without changes to routing.

pub mod builtins;

/// A recognized built-in slash command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashCommand {
    /// `/help` — show available commands and basic usage.
    Help,
    /// `/clear` — clear the visible transcript.
    Clear,
    /// `/quit` — exit kapollo cleanly.
    Quit,
}

/// The result of dispatching a slash-command string (leader already stripped).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dispatch {
    /// A recognized command.
    Command(SlashCommand),
    /// An unknown command name (the first whitespace-delimited token).
    Unknown(String),
}

/// Dispatch a slash-command string (without the leader char). Matching is
/// exact and case-sensitive for the MVP (D6). Unknown commands yield
/// [`Dispatch::Unknown`] so the caller can render an error block suggesting
/// `/help`.
pub fn dispatch(command: &str) -> Dispatch {
    let name = command.trim();
    match name {
        "help" => Dispatch::Command(SlashCommand::Help),
        "clear" => Dispatch::Command(SlashCommand::Clear),
        "quit" => Dispatch::Command(SlashCommand::Quit),
        other => {
            let token = other.split_whitespace().next().unwrap_or("").to_string();
            Dispatch::Unknown(token)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatches_known_commands() {
        assert_eq!(dispatch("help"), Dispatch::Command(SlashCommand::Help));
        assert_eq!(dispatch("clear"), Dispatch::Command(SlashCommand::Clear));
        assert_eq!(dispatch("quit"), Dispatch::Command(SlashCommand::Quit));
    }

    #[test]
    fn unknown_command_reports_its_name() {
        assert_eq!(dispatch("bogus"), Dispatch::Unknown("bogus".to_string()));
        assert_eq!(dispatch("Help"), Dispatch::Unknown("Help".to_string()));
    }
}
