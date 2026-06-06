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

Run `/keys` at any time for the live, authoritative list of bindings.

### Submitting & composing

| Key | Action |
|-----|--------|
| **Enter** | Submit the input pad contents as one command |
| **Shift+Enter** / **Alt+Enter** | Insert a newline without submitting (compose multiline input) |
| **Backspace** | Delete the character before the cursor |
| **Up** / **Down** | Recall previous / next entries from kapollo's input history |

On submit, trailing blank (whitespace-only) lines of a multiline buffer are
dropped so a stray empty last line does not run an extra command; interior blank
lines are preserved and single-line input is never altered.

### Cursor motion & line editing (input pad)

| Key | Action |
|-----|--------|
| **Left** / **Right** | Move the cursor one character |
| **Ctrl+Left** / **Ctrl+Right** | Move the cursor one word (within the current line) |
| **Home** / **End** | Jump to the start / end of the current line |
| **Shift+Left** / **Shift+Right** | Extend the selection one character |
| **Shift+Ctrl+Left** / **Shift+Ctrl+Right** | Extend the selection one word |
| **Ctrl+U** / **Ctrl+K** | Delete to the start / end of the current line |
| **Ctrl+W** | Delete the word before the cursor |

Motion and kills are scoped to the caret's **current line** and never cross a
newline.

### Selection, copy & cancel

| Key | Action |
|-----|--------|
| **Ctrl-C** | Copy the active selection if there is one; otherwise send SIGINT to the running command |
| **Esc** | Cancel an active selection; with no selection, clear the current line |
| **Esc Esc** | On a multiline buffer, clear the whole buffer; also clears the status message |

At most **one** selection is active at a time across the input pad and the
transcript: starting a selection in one clears the other.

### Scrolling the transcript

| Key | Action |
|-----|--------|
| **PageUp** / **PageDown** | Scroll a page at a time (keeping `scroll.context_lines` of overlap) |
| **Shift+PageUp** / **Shift+PageDown** | Scroll one line at a time |
| **Shift+Home** / **Shift+End** | Jump to the oldest / newest output |

`Shift+Enter` requires a terminal that supports the Kitty keyboard protocol;
`Alt+Enter` is the universal fallback for inserting a newline.

kapollo keeps its **own** input history, separate from your shell's native
history.

## Mouse, selection, and copy

kapollo captures the mouse for selection over the transcript. The grid it
renders is a real terminal emulator, so progress bars, in-place redraws, and
inline color appear exactly as the program intended.

| Mouse action | Result |
|--------------|--------|
| **Left-drag** | Select a range of text; auto-scrolls when you drag past the top or bottom edge |
| **Right-click** on an active selection | Copy the selection |
| **Right-click** with no selection | Copy the block under the cursor, including its command line |
| **Scroll wheel** | Scroll the transcript (see `scroll.wheel_lines`) |
| **Shift + any mouse action** | Bypass kapollo and use the host terminal's native selection |
| **Ctrl-C** with an active selection | Copy the selection (does not interrupt the command) |

Copying prefers **OSC 52** (terminal-mediated, so it works over SSH) and falls
back to the local OS clipboard; if neither is available, a notice appears on the
status rule. Selections and block copies are taken from kapollo's retained
**block store**, so the text is faithful to what the command produced.

When a full-screen program grabs the mouse (e.g. `vim` with mouse mode, `htop`),
mouse events are forwarded to it instead.

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
| `/status` | Toggle the fixed status bar on or off |
| `/keys` | List the active key bindings |
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

[mouse]
# Capture the mouse for selection. When false, the host terminal handles the
# mouse and kapollo never grabs it (default true).
enabled = true
# Copy the selection automatically when you release the mouse (default false).
copy_on_select = false

[clipboard]
# Use terminal-mediated OSC 52 copy first (works over SSH; default true).
osc52 = true
# Fall back to the local OS clipboard when OSC 52 is off or unavailable
# (default true).
local_fallback = true

[scroll]
# Transcript lines advanced per mouse-wheel notch (default 3).
wheel_lines = 3
# Number of scrollback lines the grid retains (default 10000).
scrollback_lines = 10000
# Lines of overlap kept when paging the transcript with PageUp/PageDown
# (default 3).
context_lines = 3

[status]
# Show the fixed status bar beneath the input pad (default true). Toggle live
# with /status. Auto-hides on terminals shorter than 10 rows.
enabled = true

[divider]
# Draw a horizontal rule directly above the input pad (default true).
enabled = true
```

When a block exceeds its cap, the oldest output is dropped and a
`… output truncated …` marker is shown.

## Chrome

The transcript renders kapollo's emulated terminal grid directly, so command
output looks exactly as it would in a normal terminal. Directly above the input
pad a horizontal **divider** rule separates the transcript from your input
(toggle with `[divider] enabled`).

Beneath the input pad a single-line **status bar** shows, left to right: a
4-column **mode** field (`norm` by default), the current working directory
(which follows `cd`), an optional transient **message**, and the last command's
**exit code** hugging the right edge. The bar never wraps: when space runs short
the message is shortened first, then the cwd is middle-ellipsized (e.g.
`/home/…/kapollo`), while the mode and exit code are always preserved. Toggle the
bar with `/status`; it auto-hides on terminals shorter than 10 rows.

**Status messages** (such as a copy result or failure) persist until you submit
the next command (**Enter**) or clear them with **Esc Esc** — they never time
out.


## Color

kapollo honors the `NO_COLOR` convention: set `NO_COLOR` to a non-empty value
to disable color in kapollo's own chrome (the `λ` prompt, the divider, and the
status bar).
