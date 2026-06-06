# Pre-plan 005 — Input editing & fixed status bar

> Source: `.scratch/005-pre-planning.md` (the "daily-driver input" cut).
> Theme: the highest "I'll actually use it daily" value per unit of work —
> input-line editing, scrollback polish, and a fixed-format status bar.
> Keys ship **hardcoded** this sprint; configurability is sprint 006.

## Goal

Make kapollo's input pad and scrollback feel like a real shell line editor, and
surface a small always-on status bar — without taking on the keymap engine,
status template language, or LAAT mode.

## In scope

### Input-line editing (hardcoded keys)

| Key | Function |
|-----|----------|
| `Home` | Move cursor to start of current line |
| `End` | Move cursor to end of current line |
| `Ctrl+Left` | Move left to start of word |
| `Ctrl+Right` | Move right to end of word |
| `Shift+Left` | Start/extend selection one char left |
| `Shift+Right` | Start/extend selection one char right |
| `Shift+Ctrl+Left` | Start/extend selection to start of word |
| `Shift+Ctrl+Right` | Start/extend selection to end of word |
| `Ctrl+U` | Kill to start of line |
| `Ctrl+K` | Kill to end of line |
| `Ctrl+W` | Delete word before cursor |
| `Esc` | If a selection is active, cancel it; else clear the current line. In a multi-line buffer, `Esc Esc` clears the whole buffer (see Decisions / Q3) |

### Bracketed-paste rework (NOTE_1)

- Today a multi-line paste submits every line except the last (each `\n`
  triggers a submit). Change: a paste lands as **one multi-line input buffer**
  (one line per pasted line); nothing is submitted until the user presses
  `Enter`.
- This is a **bracketed-paste handling change** (crossterm `Event::Paste`), not
  a `Ctrl+V` keybinding — the original plan's `Ctrl-V` row is dropped.

### Scrollback polish

| Key | Function |
|-----|----------|
| `PageUp` | Scroll transcript up one page minus *context lines* |
| `PageDown` | Scroll transcript down one page minus *context lines* |
| `Shift+PageUp` | Scroll transcript up one line |
| `Shift+PageDown` | Scroll transcript down one line |
| `Shift+Home` | Jump to oldest output (top of scrollback) |
| `Shift+End` | Jump to newest output (bottom) |

- *Context lines* default **3**, with edge-case handling when the output pad is
  too small (clamp so a page scroll always advances at least 1 line).
- `Home`/`End` are freed from scrollback (now line-editing); their old jobs move
  to `Shift+Home`/`Shift+End`.

### Mouse click-vs-drag threshold

- A plain left-click that places focus should **not** create a one-cell
  selection (the current annoyance). **The exact rule is a research item**
  (kwi WI #45) and is *not* a committed 005 deliverable; crossterm reports mouse
  position at cell resolution only, so the Windows-Terminal-style "tiny drag"
  may not be directly detectable. If the research lands a viable rule during the
  sprint it may be folded in, otherwise it rolls forward.

### Fixed-format status bar

- Single line under the input pad; **on by default**, configurable on/off.
- Auto-hidden when the terminal has **< 10 rows**.
- **Fixed format** this sprint (no template engine — that's sprint 007). Reserve
  a 4-char mixed-case **mode field** (e.g. `LaaT`) so future modes don't reflow
  the layout. In this sprint the only mode is the default shell mode.
- Layout: `mode | cwd<greedypad>| message | exit` — a greedy pad sits between
  `cwd` and `message` (no `|` separator after `cwd`); `message` is
  right-justified into the remaining width.
- New `/status` slash command toggles the status bar on/off.

### `/keys` discoverability

- New `/keys` slash command lists the (hardcoded) key map.
- `/help` gains a one-line pointer to `/keys`.

## Decisions (resolved in pre-planning)

- **Single selection across both pads.** There is at most one active selection
  at a time. Starting a selection in the input pad clears any output-pad
  selection and vice-versa — which collapses the `Ctrl+C`/`Esc` ambiguity.
- **`Esc` semantics:** active selection → cancel selection; otherwise → clear
  input buffer. A *double* `Esc` clears the status message. In a **multi-line**
  input buffer, clearing the buffer requires `Esc Esc` (single `Esc` clears the
  current line / cancels selection only) — see Q3.
- **`Home`/`End` act on the current line** in a multi-line buffer. Reserve
  action names `multiline_move_start_buffer` / `multiline_move_end_buffer` for
  whole-buffer motion, **left unmapped** for now.
- **Status message lifetime:** message persists until the next submitted command
  (`Enter`) — *not* a timeout. Add an explicit clear via double `Esc`.
- **Keys are hardcoded** this sprint; configurability is sprint 006. Each action
  should be named now (so 006 can bind default + alternate per action) even
  though the binding is hardcoded here.

## Out of scope (explicitly deferred)

- Configurable keymap engine → **pre-plan-006**.
- Status template language → **pre-plan-007**.
- LAAT mode + `Ctrl+Alt+Enter` push/pop input stack → **pre-plan-008**.
- `/save` and `/filter` slash commands → **pre-plan-008** (grouped with LAAT by
  mental model, even though they touch different code).
- Binding `copy_block_without_command` / `copy_current_line` to keys — no good
  default chosen; wait for keymap config (006).

## Open questions

- **Q2 — Word-motion boundary rule.** *Resolved:* `Ctrl+W` kill uses the
  readline whitespace rule; `Ctrl+Left`/`Ctrl+Right` motion is punctuation-aware.
- **Q3 — `Esc` in a multi-line input.** *Resolved:* single `Esc` cancels a
  selection / clears the current line; `Esc Esc` clears the whole buffer.
- **Q4 — Status bar contents & layout.** *Resolved:* fixed format is
  `mode | cwd<greedypad>| message | exit` — i.e. the greedy pad sits between
  `cwd` and `message` with **no `|` separator after `cwd`**; the message is
  right-justified into the remaining width.
- **Q5 — Mouse click-vs-drag (technical).** *Cut from 005 scope; tracked as kwi
  WI #45.* crossterm reports mouse position at cell resolution only, so the
  "ignore sub-cell drags" approach isn't directly expressible. Research whether
  SGR-pixel mouse reporting (or another mechanism) makes a click-vs-drag
  threshold viable.

## Dependencies / sequencing

- Foundation for **006** (keymap engine binds these named actions) and **008**
  (LAAT needs the multi-line input buffer landed here).
- No dependency on 007.

## Suggested success criteria (for the eventual spec)

- Word motion, line motion, selection-by-keyboard, and the kill commands all
  operate correctly in single- and multi-line buffers.
- A multi-line paste never auto-submits; `Enter` submits the whole buffer.
- Only one selection is ever active across the two pads.
- Status bar renders, toggles via `/status`, hides under 10 rows, and clears its
  message on the next `Enter` (or double `Esc`).
- `/keys` lists the active bindings.
