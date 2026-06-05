//! Input router: decides whether submitted input is a kapollo slash command or
//! shell input (FR-021, FR-022). The leader char (default `/`) marks a slash
//! command; a doubled leader escapes to literal shell input.

/// The routing decision for a submitted line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Routed {
    /// A slash command, with the leader char stripped (e.g. `/help` → `help`).
    Slash(String),
    /// Literal input to send to the wrapped shell.
    Shell(String),
}

/// Route `line` according to `leader`.
///
/// - Leading `leader` → [`Routed::Slash`] (one leader stripped).
/// - Leading doubled `leader` (e.g. `//`) → [`Routed::Shell`] with one leader
///   stripped, passing the rest through literally (FR-022).
/// - Anything else → [`Routed::Shell`] unchanged.
pub fn route(line: &str, leader: char) -> Routed {
    let mut chars = line.chars();
    if chars.next() != Some(leader) {
        return Routed::Shell(line.to_string());
    }
    // Starts with the leader. Check for a doubled-leader escape.
    if chars.clone().next() == Some(leader) {
        // Strip exactly one leader; the remainder is literal shell input.
        return Routed::Shell(chars.as_str().to_string());
    }
    Routed::Slash(chars.as_str().to_string())
}

/// Where a mouse event should go, decided from the routing context (FR-015/016).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseRoute {
    /// Forward to the wrapped child program (it owns the mouse).
    ToChild,
    /// kapollo consumes it: selection FSM (click/drag) or scrollback (wheel).
    Consumed,
    /// Let the host terminal handle it natively (Shift-held selection).
    Bypass,
}

/// Decide where a mouse event goes given the current context. Shift always wins
/// (host-native selection, FR-016); otherwise a full-screen program or a child
/// that has enabled mouse tracking owns the mouse (FR-015); otherwise kapollo
/// consumes the event for its own selection / scrollback (FR-007/009).
pub fn route_mouse(shift: bool, alt_screen: bool, child_mouse: bool) -> MouseRoute {
    if shift {
        MouseRoute::Bypass
    } else if alt_screen || child_mouse {
        MouseRoute::ToChild
    } else {
        MouseRoute::Consumed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leader_prefixed_is_a_slash_command() {
        assert_eq!(route("/help", '/'), Routed::Slash("help".to_string()));
        assert_eq!(route("/clear", '/'), Routed::Slash("clear".to_string()));
    }

    #[test]
    fn doubled_leader_escapes_to_literal_shell_input() {
        assert_eq!(route("//ls", '/'), Routed::Shell("/ls".to_string()));
        assert_eq!(route("//", '/'), Routed::Shell("/".to_string()));
    }

    #[test]
    fn non_slash_input_passes_through() {
        assert_eq!(route("ls -la", '/'), Routed::Shell("ls -la".to_string()));
        assert_eq!(route("", '/'), Routed::Shell("".to_string()));
    }

    #[test]
    fn honors_a_custom_leader_char() {
        assert_eq!(route(":quit", ':'), Routed::Slash("quit".to_string()));
        assert_eq!(route("/path", ':'), Routed::Shell("/path".to_string()));
        assert_eq!(route("::echo", ':'), Routed::Shell(":echo".to_string()));
    }

    #[test]
    fn shift_always_bypasses_to_host() {
        assert_eq!(route_mouse(true, false, false), MouseRoute::Bypass);
        assert_eq!(route_mouse(true, true, false), MouseRoute::Bypass);
        assert_eq!(route_mouse(true, false, true), MouseRoute::Bypass);
    }

    #[test]
    fn alt_screen_or_child_mouse_routes_to_child() {
        assert_eq!(route_mouse(false, true, false), MouseRoute::ToChild);
        assert_eq!(route_mouse(false, false, true), MouseRoute::ToChild);
        assert_eq!(route_mouse(false, true, true), MouseRoute::ToChild);
    }

    #[test]
    fn otherwise_kapollo_consumes() {
        assert_eq!(route_mouse(false, false, false), MouseRoute::Consumed);
    }
}
