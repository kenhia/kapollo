//! Slash-command layer: exact-match dispatch of leader-prefixed commands
//! intercepted by the input router (FR-023, contracts/slash-commands.md). The
//! registry is intentionally tiny for the MVP; post-MVP commands slot in
//! without changes to routing.

pub mod builtins;

/// A recognized built-in slash command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommand {
    /// `/help` — show available commands and basic usage.
    Help,
    /// `/clear` — clear the visible transcript.
    Clear,
    /// `/quit` — exit kapollo cleanly.
    Quit,
    /// `/status` — toggle the fixed status bar (sprint 005, FR-026).
    Status,
    /// `/keys` — list the active key bindings (sprint 005, FR-030).
    Keys,
    /// `/reload-config` — re-read configuration on demand (sprint 006, FR-015).
    ReloadConfig,
    /// `/save <path>` — write the previous block's output to a file (sprint 007,
    /// FR-021). The payload is the trimmed path (empty when omitted).
    Save(String),
    /// `/filter <cmd>` — pipe the previous block's output through `<cmd>` via the
    /// shell (sprint 007, FR-025). The payload is the raw remainder (pipes kept).
    Filter(String),
    /// `/load <path>` — load a file's lines into the input buffer and enter
    /// `Laat` (sprint 007, FR-028). The payload is the trimmed path.
    Load(String),
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
/// exact and case-sensitive for the MVP (D6). The argument-bearing commands
/// (`save`/`filter`/`load`, sprint 007) take the trimmed remainder as payload;
/// argument-less commands still require an exact match. Unknown commands yield
/// [`Dispatch::Unknown`] so the caller can render an error block suggesting
/// `/help`.
pub fn dispatch(command: &str) -> Dispatch {
    let trimmed = command.trim();
    // The verb is the first whitespace-delimited token; the payload is the
    // trimmed remainder (empty payloads still dispatch, sprint 007).
    let (verb, rest) = match trimmed.split_once(char::is_whitespace) {
        Some((v, r)) => (v, r.trim()),
        None => (trimmed, ""),
    };
    match verb {
        "save" => return Dispatch::Command(SlashCommand::Save(rest.to_string())),
        "filter" => return Dispatch::Command(SlashCommand::Filter(rest.to_string())),
        "load" => return Dispatch::Command(SlashCommand::Load(rest.to_string())),
        _ => {}
    }
    // Argument-less commands require an exact match (preserve sprint 005/006
    // behavior: a trailing argument is an unknown command, not a bare verb).
    match trimmed {
        "help" => Dispatch::Command(SlashCommand::Help),
        "clear" => Dispatch::Command(SlashCommand::Clear),
        "quit" | "exit" => Dispatch::Command(SlashCommand::Quit),
        "status" => Dispatch::Command(SlashCommand::Status),
        "keys" => Dispatch::Command(SlashCommand::Keys),
        "reload-config" => Dispatch::Command(SlashCommand::ReloadConfig),
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
        assert_eq!(dispatch("status"), Dispatch::Command(SlashCommand::Status));
        assert_eq!(dispatch("keys"), Dispatch::Command(SlashCommand::Keys));
    }

    #[test]
    fn dispatches_reload_config() {
        assert_eq!(
            dispatch("reload-config"),
            Dispatch::Command(SlashCommand::ReloadConfig)
        );
    }

    #[test]
    fn unknown_command_reports_its_name() {
        assert_eq!(dispatch("bogus"), Dispatch::Unknown("bogus".to_string()));
        assert_eq!(dispatch("Help"), Dispatch::Unknown("Help".to_string()));
    }
}
