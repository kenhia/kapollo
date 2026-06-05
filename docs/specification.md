# kapollo Specification (Combined)

> Authoritative combined specification per Constitution Principle I. This
> document consolidates the MVP requirements for quick reference. The canonical
> per-feature source is [specs/001-mvp-repl/spec.md](../specs/001-mvp-repl/spec.md);
> the technical reference is [architecture.md](architecture.md).

Last updated: 2026-06-04 (grid rework 004: native terminal grid via
`wezterm-term`, mouse selection/copy with app hand-over, and the canonical
block store)

## 1. Overview

kapollo (`kap`) is a Linux terminal application that wraps the user's real
shell (fish or bash for the MVP) and presents an Apollo-DM-style split UI: an
**input pad** at the bottom for composing commands and a **transcript pad**
above where each command and its output appear as a discrete **block**. The
transcript pad renders a **native terminal grid** (an embedded `wezterm-term`
emulator), so progress bars, in-place redraws, and inline color display exactly
as the program intended. A **slash-command** layer (invoked by a configurable
leader char) adds features beyond a plain shell wrapper. Full-screen
(alt-screen) programs are rendered through the same grid and receive mouse and
key input directly.

## 2. Functional Requirements

### CLI & shell wrapping
- **FR-001** Launchable as `kap` (and `kapollo`).
- **FR-002** Wrap the user's real shell in a PTY, defaulting to `$SHELL`.
- **FR-003** Send submitted input to the wrapped shell and read its output.

### Blocks & capture
- **FR-004** Capture each command's output and present it as a discrete block.
- **FR-005** Detect the start/end of each command's output.
- **FR-006** Capture each command's exit code and associate it with the block.
- **FR-007** Auto-inject the shell-integration hook (fish/bash); other shells
  fall back to sentinel boundary detection.
- **FR-008** Export `KAPOLLO_ACTIVE=1` and a version variable to the shell.
- **FR-034** Normalize captured output to clean printable text: drop bare `\r`
  and other C0 controls, map `\r\n` → `\n`, and never leak OSC/CSI/DCS escape
  sequences or terminal query/responses as visible text.

### Input pad & history
- **FR-009** Submit the input pad contents on Enter.
- **FR-010** Insert a literal newline on Shift+Enter / Alt+Enter (no submit).
- **FR-011** Support multiline input as a single submitted command.
- **FR-012** Auto-grow the input pad up to a height cap, then scroll internally.
- **FR-013** Maintain kapollo's own input history (separate from the shell's),
  recalled with Up/Down.
- **FR-014** Scroll the transcript independently of the input pad: PageUp/
  PageDown by a page and Home/End to the oldest/newest output (clamped);
  submitting a command re-pins the view to the newest output.

### Output retention
- **FR-015** Retain captured output bytes per block.
- **FR-016** Enforce configurable per-block and whole-transcript caps with a
  visible truncation marker; oldest output/blocks evicted first.
- **FR-035** Stay responsive and interruptible under huge output: a multi-
  million-line flood completes near shell-native time (amortized O(1) cap
  enforcement, bounded per-pass draining) and Ctrl-C interrupts promptly.

### Resize & passthrough
- **FR-017** Reflow both pads on terminal resize without losing transcript.
- **FR-018** Detect alt-screen entry and hand the full terminal to the program.
- **FR-019** Forward resize to the wrapped program during passthrough.
- **FR-020** Restore the split-pad UI with the transcript intact on exit.
- **FR-036** Forward stdin to the program verbatim during passthrough (no
  `KeyEvent` re-encoding) so terminal query/responses (OSC 11/10/4, DA, cursor
  position) reach it intact; emit an explicit SGR/cursor reset on exit so no
  residual style or hidden cursor bleeds into the restored UI.

### Slash commands
- **FR-021** Treat leader-char-prefixed input as a slash command; pass the rest
  through to the shell.
- **FR-022** Doubled leader escapes to a literal leader char passed to the shell.
- **FR-023** Provide at least `/quit`, `/clear`, and `/help` (with `/exit` as an
  alias of `/quit`).

### Signals, safety & teardown
- **FR-024** Forward Ctrl-C (SIGINT) to the running command, not kapollo.
- **FR-025** Always restore the terminal on exit, error, and panic.
- **FR-026** Catch panics at the event-loop boundary, restore, and log.
- **FR-027** Terminate cleanly when the wrapped shell exits on its own.

### Configuration, logging & environment
- **FR-028** Read `~/.config/kapollo/config.toml` if present; sensible defaults
  otherwise.
- **FR-029** Make output caps, the leader char, the wrapped shell, and the
  prompt glyph/color (`prompt_char`/`prompt_color`) configurable.
- **FR-030** Write logs to a file sink, never to the TUI; quiet by default,
  opt-in verbose.
- **FR-031** Honor `NO_COLOR` for kapollo's own chrome.
- **FR-032** Behave sanely (no TUI) when stdout is not a TTY.

### Status chrome
- **FR-033** Show at least the current working directory and last exit code.
- **FR-037** Render a borderless transcript with a colorized prompt glyph (`λ`)
  echoing each command and a blank line between blocks; carry the cwd (always,
  following `cd` via OSC 7) and the last exit code (only when non-zero) on a
  single status rule directly above the input pad. Color is suppressed under
  `NO_COLOR`.

### Grid, selection & block store (sprint 004)
- **FR-G01** Render the transcript through an embedded terminal emulator
  (`wezterm-term`), which owns escape parsing, in-place CR updates, SGR color,
  and alt-screen state; kapollo never re-implements VT parsing as text
  heuristics. The emulator's scrollback is the authoritative scrolled history.
- **FR-G02** Support mouse selection over the transcript: left-drag selects
  (auto-scrolling past the edges), right-click or Ctrl-C copies a selection, and
  Shift bypasses to the host terminal's native selection. Ctrl-C with no
  selection still sends SIGINT.
- **FR-G03** Hand the mouse and keys to a full-screen / mouse-grabbing child;
  otherwise kapollo consumes them for selection and scrollback.
- **FR-G04** Copy via OSC 52 (terminal-mediated, SSH-friendly) with a local
  clipboard fallback, surfacing a visible notice when copying fails.
- **FR-G05** Retain each block's output in a bounded, canonical **block store**
  whose text is faithful and survives grid scrollback eviction; access is
  through a stable accessor seam so a persistent backing can be added without
  changing callers. Requests for an evicted block's text return an explicit
  "unavailable" result.
- **FR-G06** Offer block-aware copy affordances: a block's output with its
  command line, without its command line, and the current line.
- **FR-G07** Reflect each block's exit status and elapsed runtime in the chrome.
- **FR-G08** Drive block boundaries (begin / output-start / end) from OSC 133
  marks with a sentinel fallback, anchoring each block's grid rows to stable row
  indices so they never drift as new output scrolls in.

## 3. Key Entities

- **Block** — one command + retained output + exit code, plus its grid
  `row_range` (stable-row anchored), `cwd`, start/end timestamps (and derived
  `duration`), an `available` flag, and reserved `private`/`save_output` flags.
- **Block Store** — the canonical, bounded, in-memory collection of blocks and
  the source of truth for copy (and future `/save`/`/filter`); text is reached
  only through the `BlockText` accessor seam and survives grid eviction.
- **Grid** — the embedded `wezterm-term` emulator: the authoritative screen +
  scrollback the transcript pad renders from.
- **Session / Transcript** — ordered list of blocks; drives caps and chrome.
- **Input History** — kapollo's own ordered list of submitted inputs.
- **Configuration** — shell, leader char, output caps, and the `mouse`,
  `clipboard`, and `scroll` settings; defaults when absent.
- **Shell Session** — the wrapped shell process in a PTY with the injected hook.

## 4. Success Criteria

- **SC-001** Launch and run a command, seeing output as a block, with no setup.
- **SC-002** Shell state persists across 100+ consecutive commands.
- **SC-003** `vim`/`less`/`top` launch, operate, and exit with the UI restored.
- **SC-004** Ctrl-C interrupts the command and leaves kapollo running.
- **SC-005** Terminal restored cleanly on exit/error/panic.
- **SC-006** Very large output respects caps and shows a truncation marker.
- **SC-007** Multiline compose + submit as a unit; arrow-key history recall.
- **SC-008** Resize during use keeps the transcript and resizes the shell.
- **SC-009** Identical core-run-loop behavior under fish and bash.
- **SC-010** Progress bars / in-place redraws / inline color render correctly
  through the grid; mouse selection and copy place exact text on the clipboard
  with correct hand-over to full-screen programs; a block's retained text stays
  queryable after its grid rows scroll past the scrollback cap.

## 5. Scope

- **In scope (MVP)**: Linux; fish + bash; US1 (run loop), US2 (multiline +
  history), US3 (passthrough), US4 (interrupt/control/exit). Sprint 004 adds the
  native terminal grid, mouse selection/copy, and the canonical block store.
- **Out of scope**: macOS/Windows, history DB persistence, AI layer, `/save`,
  `/filter` (deferred; tracked separately), fuzzy search, markdown rendering,
  newline-key remapping.
