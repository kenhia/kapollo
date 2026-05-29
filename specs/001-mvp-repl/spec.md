# Feature Specification: kapollo MVP — Split-Pad Shell REPL

**Feature Branch**: `001-mvp-repl`  
**Created**: 2026-05-29  
**Status**: Draft  
**Input**: Brainstorm ([specs/planning/brainstorm.md](../planning/brainstorm.md)), Architecture ([docs/architecture.md](../../docs/architecture.md), decisions D1–D21), and the planning chat. Scoped to the MVP boundary defined in brainstorm §9.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Run commands in a split-pad shell (Priority: P1)

A user launches kapollo (`kap`) in their terminal and is presented with a
full-screen split layout: an **input pad** at the bottom where they type
commands and an **output (transcript) pad** above where each command and its
output appear. They type a command, press Enter, and see the command echoed
into the transcript followed by its output, with the input pad cleared and
ready for the next command. Their working directory, environment, aliases,
and shell features behave exactly as in their normal shell, because kapollo
wraps their real shell (fish or bash).

**Why this priority**: This is the core value proposition and the minimum
viable product. Without it, kapollo is not usable as a shell. Every other
story builds on this loop.

**Independent Test**: Launch `kap`, type `echo hello`, press Enter, and
confirm `hello` appears in the transcript pad as a discrete block; type
`pwd` and confirm it reflects the current directory; type `cd ..` then `pwd`
and confirm directory state persisted across commands.

**Acceptance Scenarios**:

1. **Given** kapollo is launched in a terminal, **When** the UI appears,
   **Then** an input pad is shown at the bottom and an empty transcript pad
   above it, with the input pad focused.
2. **Given** the input pad is focused, **When** the user types `echo hello`
   and presses Enter, **Then** the command and its output (`hello`) appear
   as a block in the transcript and the input pad is cleared.
3. **Given** a command has run, **When** the user runs `cd /tmp` then `pwd`,
   **Then** `pwd` reports `/tmp`, demonstrating shell state persists across
   commands.
4. **Given** a command exits non-zero (e.g. `false`), **When** it completes,
   **Then** the status chrome reflects the non-zero exit code.
5. **Given** the user's default shell is fish or bash, **When** kapollo
   launches, **Then** it wraps that shell and the user's aliases and
   functions are available.

---

### User Story 2 - Compose multiline commands and recall history (Priority: P2)

A user needs to enter a multiline command (e.g. a `for` loop or a long
pipeline broken for readability) without it being submitted prematurely.
They press Shift+Enter (or Alt+Enter) to insert a newline within the input
pad, building up multiple lines, then press Enter once to submit the whole
thing. They also press the Up arrow to recall a previously submitted input
to re-run or edit it.

**Why this priority**: Multiline editing is a first-class part of the Apollo
metaphor and a stated headline behavior; history recall is a baseline
expectation for any interactive shell front-end. Both materially improve
usability but the basic run loop (P1) is viable without them.

**Independent Test**: In the input pad, press Shift+Enter twice to create a
three-line input, type content on each line, press Enter, and confirm the
full multiline command runs as one unit; then press Up and confirm the
previous input is recalled into the input pad.

**Acceptance Scenarios**:

1. **Given** the input pad is focused, **When** the user presses
   Shift+Enter, **Then** a newline is inserted in the input pad and the
   command is NOT submitted.
2. **Given** the terminal cannot distinguish Shift+Enter, **When** the user
   presses Alt+Enter, **Then** a newline is inserted (fallback behavior).
3. **Given** a multiline command in the input pad, **When** the user presses
   Enter, **Then** the entire multiline input is submitted as one command.
4. **Given** the input pad auto-grows with multiline content, **When** the
   content exceeds a height cap, **Then** the input pad scrolls internally
   rather than consuming the whole screen.
5. **Given** the user has submitted commands earlier in the session,
   **When** they press the Up arrow in an empty input pad, **Then** the most
   recent submitted input is recalled; pressing Up again recalls the one
   before it, and Down moves back toward the newest.

---

### User Story 3 - Run interactive full-screen programs (Priority: P2)

A user runs a full-screen, interactive program such as `vim`, `less`, or
`top` from the input pad. kapollo recognizes that the program has taken over
the screen (alt-screen) and hands the terminal to it (passthrough) so it
works exactly as it would in a normal terminal. When the program exits, the
split-pad UI returns and the user continues entering commands.

**Why this priority**: kapollo is intended to be a daily driver; if common
interactive tools break, it cannot replace a normal shell. It is P2 rather
than P1 because the basic run loop demonstrates value first, but this is
required before kapollo is genuinely usable day-to-day.

**Independent Test**: From the input pad, run `vim`, confirm vim opens and is
fully usable (edit, `:q`), and confirm that on exit the split-pad UI is
restored intact with the transcript preserved.

**Acceptance Scenarios**:

1. **Given** the split-pad UI is active, **When** the user runs a program
   that switches to the alt-screen (`vim`, `less`, `top`), **Then** kapollo
   enters passthrough and the program receives the full terminal and all
   keystrokes.
2. **Given** an interactive program is running in passthrough, **When** the
   user resizes the terminal, **Then** the program sees the correct new
   dimensions.
3. **Given** an interactive program exits, **When** control returns to
   kapollo, **Then** the split-pad UI is restored and the transcript from
   before is intact.

---

### User Story 4 - Interrupt, control, and exit safely (Priority: P1)

A user runs a long or runaway command and presses Ctrl-C to interrupt it,
expecting only the running command to be interrupted — not kapollo itself.
They use slash commands (`/help`, `/clear`, `/quit`) to get help, clear the
transcript, and exit. On exit (or any crash), the terminal is always
restored to a clean, usable state.

**Why this priority**: Signal handling and clean teardown are
non-negotiable for a terminal application — a tool that can leave the
terminal corrupted or cannot be interrupted is unsafe to adopt. It is P1
because it is part of the basic trustworthy run loop.

**Independent Test**: Run `sleep 60`, press Ctrl-C, confirm the command is
interrupted and kapollo remains running; type `/quit` and confirm kapollo
exits and the terminal prompt is restored cleanly (cursor visible, normal
mode, no leftover alt-screen).

**Acceptance Scenarios**:

1. **Given** a command is running, **When** the user presses Ctrl-C, **Then**
   the running command receives the interrupt and kapollo continues running.
2. **Given** the input pad is focused, **When** the user types `/help`,
   **Then** kapollo displays available slash commands and basic usage.
3. **Given** a transcript with content, **When** the user types `/clear`,
   **Then** the visible transcript is cleared.
4. **Given** kapollo is running, **When** the user types `/quit`, **Then**
   kapollo exits and the terminal is restored to a clean state.
5. **Given** kapollo encounters a fatal error or panic, **When** it
   terminates, **Then** the terminal is restored (cursor shown, raw mode
   disabled, alt-screen left) and an error is reported.
6. **Given** the user types a literal leading leader character (e.g. wants a
   literal `/`), **When** they type the leader char twice, **Then** the
   input is treated as literal text passed to the shell, not a slash command.

---

### Edge Cases

- **Terminal too small**: When the terminal is too small to render both
  pads meaningfully, kapollo MUST degrade gracefully (e.g. show a minimal
  usable layout or a clear message) rather than crash or corrupt output.
- **Massive output**: When a command produces output exceeding the
  per-block cap, kapollo MUST retain the tail, drop the head, and show a
  visible truncation marker — without exhausting memory.
- **Transcript growth**: When the session accumulates output exceeding the
  whole-transcript cap, kapollo MUST evict oldest blocks first.
- **No TTY / piped output**: When kapollo's stdout is not a TTY, it MUST NOT
  attempt to draw the TUI and MUST behave sanely.
- **Shell exits on its own**: When the wrapped shell process exits (e.g. the
  user runs `exit`), kapollo MUST terminate cleanly and restore the
  terminal.
- **Unsupported shell**: When the user's shell is neither fish nor bash,
  kapollo MUST still attempt to wrap it (best-effort) and fall back to
  sentinel-based block detection if prompt marks are unavailable.
- **Block boundary marks unavailable**: When OSC 133 marks cannot be
  installed/observed, kapollo MUST fall back to sentinel injection so blocks
  and exit codes are still delimited.
- **`NO_COLOR` set**: When `NO_COLOR` is present in the environment, kapollo
  MUST suppress color in its own chrome.
- **Resize during normal use**: When the terminal is resized while in the
  split-pad UI, kapollo MUST reflow both pads without losing transcript
  content and forward the new size to the shell.

## Requirements *(mandatory)*

### Functional Requirements

**Core run loop & shell wrapping**

- **FR-001**: kapollo MUST run as a CLI binary launchable as `kap` (and
  `kapollo`) in a Linux terminal and present a full-screen split UI.
- **FR-002**: kapollo MUST wrap the user's real shell in a PTY, defaulting to
  `$SHELL`, and MUST support fish and bash as validated shells.
- **FR-003**: kapollo MUST send submitted input to the wrapped shell and
  preserve shell state across commands (working directory, environment,
  aliases, functions, pipes, and operators like `&&`).
- **FR-004**: kapollo MUST capture each command's output and present the
  command together with its output as a discrete **block** in the transcript
  pad.
- **FR-005**: kapollo MUST detect the start and end of each command's output
  using shell semantic prompt marks (OSC 133) where available, and MUST fall
  back to sentinel injection where marks are unavailable.
- **FR-006**: kapollo MUST capture each command's exit code and associate it
  with its block.
- **FR-007**: kapollo MUST auto-inject its shell integration hook into the
  wrapped shell for fish and bash so block boundaries and exit codes are
  captured without manual user setup.
- **FR-008**: kapollo MUST export `KAPOLLO_ACTIVE=1` (and a version variable,
  e.g. `KAPOLLO_VERSION`) into the wrapped shell's environment.

**Input pad behavior**

- **FR-009**: kapollo MUST submit the input pad contents to the shell when
  the user presses Enter.
- **FR-010**: kapollo MUST insert a literal newline (without submitting)
  when the user presses Shift+Enter or Alt+Enter.
- **FR-011**: kapollo MUST support multiline input as a single submitted
  command.
- **FR-012**: The input pad MUST auto-grow with multiline content up to a
  height cap and then scroll internally.
- **FR-013**: kapollo MUST maintain its own input history, separate from the
  wrapped shell's native history, and MUST allow recalling previous
  submitted inputs via the Up and Down arrow keys.

**Transcript / output pad behavior**

- **FR-014**: The transcript pad MUST be scrollable independently of the
  input pad.
- **FR-015**: kapollo MUST retain captured output bytes per block (not just
  rendered lines) to support later features.
- **FR-016**: kapollo MUST enforce a configurable per-block output cap and a
  configurable whole-transcript cap, using ring-buffer semantics (retain the
  tail; drop oldest), and MUST display a visible truncation marker when a
  block's output is truncated.
- **FR-017**: kapollo MUST reflow the transcript and input pads on terminal
  resize without losing transcript content, and MUST forward the new size to
  the wrapped shell.

**Interactive program passthrough**

- **FR-018**: kapollo MUST detect when a program switches to the alt-screen
  and MUST enter passthrough, handing the full terminal and keystrokes to
  that program.
- **FR-019**: kapollo MUST forward terminal resize events to the wrapped
  program during passthrough.
- **FR-020**: kapollo MUST restore the split-pad UI with the prior
  transcript intact when an interactive program exits passthrough.

**Slash commands**

- **FR-021**: kapollo MUST treat input that begins with the leader char
  (default `/`) as a slash command, and MUST pass all other input through to
  the shell.
- **FR-022**: kapollo MUST treat a doubled leader char as an escape that
  inserts a literal leader char and passes the input through to the shell.
- **FR-023**: kapollo MUST provide at least the slash commands `/quit`
  (exit), `/clear` (clear visible transcript), and `/help` (show available
  commands and basic usage).

**Signals, safety, and teardown**

- **FR-024**: kapollo MUST forward Ctrl-C (SIGINT) to the running foreground
  command rather than terminating kapollo itself.
- **FR-025**: kapollo MUST always restore the terminal to a clean state
  (cursor visible, raw mode disabled, alt-screen left) on normal exit, on
  error, and on panic.
- **FR-026**: kapollo MUST catch panics at the event-loop boundary, restore
  the terminal, log the error, and surface a recoverable error message
  rather than leaving the terminal corrupted.
- **FR-027**: kapollo MUST terminate cleanly and restore the terminal when
  the wrapped shell process exits on its own.

**Configuration, logging, and environment**

- **FR-028**: kapollo MUST read configuration from
  `~/.config/kapollo/config.toml` if present, and MUST operate with sensible
  defaults when no config file exists.
- **FR-029**: Output caps, the leader char, and the wrapped shell MUST be
  configurable via the config file. (Newline-key remapping is out of MVP
  scope.)
- **FR-030**: kapollo MUST write logs to a file sink and MUST NOT write log
  output to the TUI surface; default verbosity MUST be quiet, with opt-in
  verbose logging.
- **FR-031**: kapollo MUST honor `NO_COLOR` for its own chrome.
- **FR-032**: kapollo MUST behave sanely when stdout is not a TTY (no TUI
  rendering).

**Status chrome**

- **FR-033**: kapollo MUST display a status area showing at least the current
  working directory and the last command's exit code.

### Key Entities *(include if feature involves data)*

- **Block**: One command plus its captured output and exit code. Attributes:
  identifier, command text, start/end timestamps, captured output (size-
  capped, ring-buffered bytes), exit code, and flags reserved for later
  features (e.g. `private`, `save_output`). The transcript is an ordered
  collection of blocks and is the source of truth the UI renders from.
- **Session / Transcript**: The ordered list of blocks for the running
  kapollo instance, subject to the whole-transcript output cap.
- **Input History**: kapollo's own ordered list of previously submitted
  inputs, navigable via arrow keys, separate from the wrapped shell's
  history.
- **Configuration**: User settings loaded from
  `~/.config/kapollo/config.toml` — at minimum the wrapped shell, leader
  char, and output caps, with defaults applied when absent.
- **Shell Session**: The wrapped shell process running in a PTY, with its
  environment (including `KAPOLLO_ACTIVE`) and the injected integration hook.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can launch kapollo and run a command, seeing its output
  as a discrete block, within seconds of launch and with no manual setup.
- **SC-002**: Shell state (working directory, environment, aliases) persists
  correctly across at least 100 consecutive commands in a session with no
  divergence from running the same commands in the bare shell.
- **SC-003**: Common interactive programs (`vim`, `less`, `top`) launch,
  operate, and exit with the split-pad UI fully restored afterward, in 100%
  of attempts during validation.
- **SC-004**: Pressing Ctrl-C interrupts the running command and leaves
  kapollo running in 100% of attempts.
- **SC-005**: On exit, error, or induced panic, the terminal is restored to
  a clean, usable state in 100% of attempts (no corrupted terminal, no
  leftover alt-screen, cursor visible).
- **SC-006**: A command producing very large output (e.g. millions of lines)
  does not exhaust memory; the per-block and transcript caps hold and a
  truncation indicator is shown.
- **SC-007**: Multiline commands compose with Shift+Enter/Alt+Enter and
  submit as a single unit; the user can recall prior inputs with the arrow
  keys in 100% of attempts.
- **SC-008**: The terminal can be resized during normal use without losing
  transcript content, and the wrapped shell observes the correct dimensions.
- **SC-009**: kapollo behaves identically for the core run loop whether the
  wrapped shell is fish or bash.

## Assumptions

- **Platform**: MVP targets Linux only. macOS and Windows are out of scope
  for the MVP (planned later; D9).
- **Shells**: fish and bash are the validated shells; other shells are
  best-effort and may rely on the sentinel fallback for block detection
  (D17).
- **Output model**: stdout and stderr are captured as a single best-effort
  interleaved stream (single PTY); true stream separation is not provided
  (documented limitation; D14/D13).
- **Slots / file save**: Saving inputs/outputs to files and named slots are
  out of MVP scope (post-MVP; D10).
- **AI feature layer**: Out of MVP scope; the block model is shaped to
  support it later (D11).
- **Rich history & history DB**: Persistent rich history (SQLite store,
  purge controls, privacy leaders) is out of MVP scope; MVP provides only
  in-session arrow-key recall (D13/D20).
- **Rich slash-mode**: Fuzzy matching, inline descriptions, and additional
  slash commands beyond `/quit`, `/clear`, `/help` are out of MVP scope
  (post-MVP; D6).
- **Markdown rendering / output filters**: `/view`, `/save`, `/filter` are
  out of MVP scope (post-MVP; Tier 2).
- **Newline-key remapping**: Configurable key remapping for newline/submit
  is out of MVP scope (post-MVP; D16).
- **Config format**: Configuration is TOML at the XDG path
  `~/.config/kapollo/config.toml` (D15).
- **Terminal capabilities**: Where supported, the terminal can be put into a
  mode that distinguishes Shift+Enter; Alt+Enter is the reliable fallback
  for terminals that cannot (D16).
- **Shell history**: The wrapped shell continues to manage its own history
  natively; kapollo does not alter it in the MVP (D20).
