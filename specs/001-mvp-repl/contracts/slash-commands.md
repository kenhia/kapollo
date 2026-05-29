# Contract: Built-in Slash Commands

**Feature**: 001-mvp-repl

Slash commands are intercepted by the input router before reaching the shell
(FR-021). They begin with the leader char (default `/`).

## Routing rules

- Input whose first character is the leader char is parsed as a slash
  command (FR-021).
- Input beginning with a **doubled** leader char (e.g. `//`) is an escape:
  one leader char is stripped and the remainder is passed to the shell as
  literal input (FR-022).
- All other input is passed through to the wrapped shell (FR-021).
- Unknown slash command: kapollo shows an error block naming the command and
  suggests `/help`; nothing is sent to the shell.

## MVP commands

| Command | Effect | Maps to |
|---------|--------|---------|
| `/help` | Display the list of available slash commands and basic usage (keys, leader char, how to quit). | FR-023 |
| `/clear` | Clear the visible transcript. Does not affect the wrapped shell. | FR-023 |
| `/quit` | Exit kapollo cleanly and restore the terminal. | FR-023, FR-025 |

## Behavior contract

- Slash commands act on kapollo state, not the shell; they do not create a
  shell command block (though `/help` output and errors render as blocks).
- `/quit` triggers the same clean-teardown path as wrapped-shell exit
  (FR-025).
- Command names are case-sensitive and matched exactly for MVP (fuzzy
  matching is post-MVP, D6).

## Out of MVP scope

`/save`, `/filter`, `/view`, `/cd`, `/history`, slots, AI commands, and rich
slash-mode (fuzzy + inline descriptions) are post-MVP. The registry is
designed so these slot in without changes to routing (architecture §2).

## Acceptance mapping

- FR-021, FR-022, FR-023.
