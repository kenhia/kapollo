# Contract Delta: Slash commands — `/exit` alias & `/help` content

Extends [specs/001-mvp-repl/contracts/slash-commands.md](../../001-mvp-repl/contracts/slash-commands.md)
(FR-020, FR-022).

## `/exit` — alias for `/quit`

- `/exit` MUST behave **identically** to `/quit`: it triggers the same clean
  teardown path (sets `should_quit`, the event loop ends, the RAII terminal guard
  restores the terminal).
- Dispatch: the slash registry maps both `quit` and `exit` to the same
  `SlashCommand::Quit`. No new command variant is required.
- `/exit` is a normal slash command: typing the literal leader twice
  (`//exit`) still escapes to literal text (unchanged leader/escape semantics).

## `/help` — must list transcript scrolling keys

The `/help` body MUST include the transcript scrolling key bindings (FR-022). The
keys section is extended to read (leader-parameterized as today):

```text
kapollo slash commands (leader '/'):
  /help     Show this help.
  /clear    Clear the visible transcript.
  /quit     Exit kapollo and restore the terminal.
  /exit     Alias for /quit.

Keys:
  Enter            Submit the current input.
  Shift/Alt+Enter  Insert a newline (compose multiline input).
  Ctrl-C           Interrupt the running command (not kapollo).
  PageUp/PageDown  Scroll the transcript up/down.
  Home/End         Jump to the top/bottom of the transcript.

To send literal input beginning with '/', double it (e.g. '//path').
```

- `/help` MUST mention `/exit` alongside the other commands.
- `/help` MUST list `PageUp`/`PageDown` and `Home`/`End` (the scrolling keys from
  the keybindings contract).
- The exact wording may differ; the **required content** is: the four commands
  (`/help`, `/clear`, `/quit`, `/exit`) and the scrolling keys.
