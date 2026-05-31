# Usage

kapollo wraps your real shell in a split-pad UI: an **input pad** at the bottom
where you compose commands, and a **transcript pad** above where each command
and its output appear as a discrete **block**. A status line shows the current
working directory and the last exit code.

## Running commands

Type a command and press **Enter**. kapollo sends it to the wrapped shell and
captures the output into a new block. Shell state (cwd, env, aliases) persists
across commands because it is your real shell.

## Key bindings

| Key | Action |
|-----|--------|
| **Enter** | Submit the input pad contents as one command |
| **Shift+Enter** / **Alt+Enter** | Insert a newline without submitting (compose multiline input) |
| **Left** / **Right** | Move the cursor within the input pad |
| **Backspace** | Delete the character before the cursor |
| **Up** / **Down** | Recall previous / next entries from kapollo's input history |
| **PageUp** / **PageDown** | Scroll the transcript a page at a time |
| **Home** / **End** | Jump to the oldest / newest output in the transcript |
| **Ctrl-C** | Send SIGINT to the running command (interrupts the command, not kapollo) |

`Shift+Enter` requires a terminal that supports the Kitty keyboard protocol;
`Alt+Enter` is the universal fallback for inserting a newline.

kapollo keeps its **own** input history, separate from your shell's native
history.

## Full-screen programs

When you run a full-screen (alt-screen) program such as `vim`, `less`, or
`top`, kapollo hands the whole terminal to it (passthrough) and hides its own
chrome. On exit, the split-pad UI is restored with the prior transcript intact.
Terminal resizes are forwarded to the program while it runs.

## Slash commands

The **leader char** (default `/`) at the start of input invokes a slash
command instead of running a shell command. Double the leader (`//`) to send a
literal leading leader char to the shell.

| Command | Action |
|---------|--------|
| `/help` | Show available slash commands |
| `/clear` | Clear the visible transcript |
| `/quit` | Exit kapollo, restoring the terminal cleanly |
| `/exit` | Alias for `/quit` |

## Configuration

kapollo reads `~/.config/kapollo/config.toml` if it exists. Missing keys fall
back to defaults; unknown keys are logged and ignored (not fatal). Out-of-range
caps are clamped to hard maxima.

```toml
# Shell to wrap (defaults to $SHELL when unset).
shell = "/usr/bin/fish"

# Leader character for slash commands (default "/").
leader_char = "/"

# Prompt glyph echoed before each command in the transcript (default "λ").
prompt_char = "λ"

# Color of the prompt glyph (named color; default "red"). Honors NO_COLOR.
prompt_color = "red"

[caps]
# Per-block output retention. Defaults: 1 MiB / 50000 lines.
# Hard maximum for per_block_bytes is 64 MiB.
per_block_bytes = 1048576
per_block_lines = 50000

# Whole-transcript retention. Defaults: 128 MiB / 1000 blocks.
# Oldest blocks are evicted first.
transcript_bytes = 134217728
transcript_blocks = 1000
```

When a block exceeds its cap, the oldest output is dropped and a
`… output truncated …` marker is shown.

## Chrome

The transcript is borderless. Each command is echoed with a colorized prompt
glyph (`λ` by default; see `prompt_char` / `prompt_color`), and consecutive
blocks are separated by a blank line. A single status rule sits directly above
the input pad showing the current working directory (which follows `cd`) and the
last exit code — the exit code is shown only when it is non-zero.

## Color

kapollo honors the `NO_COLOR` convention: set `NO_COLOR` to a non-empty value
to disable color in kapollo's own chrome (the `λ` prompt and the status rule).
