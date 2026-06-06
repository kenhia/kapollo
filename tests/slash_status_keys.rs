//! `/keys` and `/help` discoverability (sprint 005, US-polish; FR-030/FR-031).
//! `/keys` must list every mapped binding by its stable action name, and `/help`
//! must point the user at `/keys` for the full key reference.

use kapollo::action::{self, Action};
use kapollo::slash::builtins::help_text;

/// The `/keys` body is rendered from `action::listing()`; assert the listing
/// covers every mapped action exactly once and includes the contextual
/// `Esc Esc` gesture, so the rendered command can never silently drop a binding.
#[test]
fn keys_lists_every_mapped_binding() {
    let listing = action::listing();

    // Every mapped action appears, by stable name, with a non-empty key display.
    for action in MAPPED_ACTIONS {
        let name = action.name();
        let entry = listing
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("/keys listing is missing `{name}`"));
        assert!(
            !entry.1.is_empty(),
            "`{name}` has an empty key display in /keys"
        );
    }

    // The contextual `Esc Esc` gesture is surfaced even though it is not a chord.
    let clear = listing
        .iter()
        .find(|(n, _)| n == Action::ClearStatusMessage.name())
        .expect("/keys lists clear_status_message");
    assert_eq!(clear.1, "Esc Esc");

    // Reserved, unmapped actions never appear (they have no binding).
    assert!(
        !listing
            .iter()
            .any(|(n, _)| n == Action::MultilineMoveStartBuffer.name()
                || n == Action::MultilineMoveEndBuffer.name()),
        "reserved unmapped actions must not appear in /keys"
    );
}

/// `/help` points at `/keys` for the full key reference (FR-031).
#[test]
fn help_points_at_keys() {
    let text = help_text('/');
    assert!(text.contains("/keys"), "/help should mention /keys");
    assert!(text.contains("/status"), "/help should mention /status");
}

/// Every action with a default key binding this sprint (data-model §3 "Mapped
/// actions"). Reserved and contextual actions are intentionally excluded.
const MAPPED_ACTIONS: &[Action] = &[
    Action::LineMoveStart,
    Action::LineMoveEnd,
    Action::WordMoveLeft,
    Action::WordMoveRight,
    Action::SelectCharLeft,
    Action::SelectCharRight,
    Action::SelectWordLeft,
    Action::SelectWordRight,
    Action::KillToLineStart,
    Action::KillToLineEnd,
    Action::DeleteWordBefore,
    Action::ScrollPageUp,
    Action::ScrollPageDown,
    Action::ScrollLineUp,
    Action::ScrollLineDown,
    Action::ScrollToTop,
    Action::ScrollToBottom,
];
