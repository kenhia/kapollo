# Feature Specification: LAAT Mode, `/save`, `/filter`, and `/load`

**Feature Branch**: `007-laat-mode`  
**Created**: 2026-06-07  
**Status**: Draft  
**Input**: User description: "Sprint 007 — LAAT (Line-At-A-Time) mode, the `Mult` multi-line editing mode, a one-item push/pop input stack, and the `/save`, `/filter`, and `/load` slash commands. One mental model: working with the output of prior commands and stepping through a sequence of commands one at a time."

## Overview

Sprint 005 shipped a multi-line input buffer and a fixed status bar with a
reserved 4-character **mode field**; sprint 006 turned every key binding into a
**named, rebindable action** resolved by a keymap engine. This feature is the
first real exercise of kapollo's "modes" concept and groups four themes under one
mental model — **working with prior command output and stepping through commands
one at a time**:

1. **LAAT (Line-At-A-Time) mode** — a modal multi-line buffer where each line is
   a command. A highlight steps line-by-line; `Enter` submits the highlighted
   line(s); execution **gates on the exit code** — exit `0` advances the
   highlight to the next line, while a non-zero exit keeps the highlight in place
   and flags that line's highlight background as a **probable failure**. Mode
   label `LaaT`, short form `1T`.
2. **`Mult` mode** — the multi-line editing mode entered by adding a second line.
   Here `Up`/`Down` move the caret **between lines** instead of recalling history,
   with **chat-style history recall at the edges**: `Up` while already on the
   first line stashes the current draft and recalls the previous history entry;
   `Down` from the top restores the stashed draft. LAAT is conceptually
   "`Mult` + highlight + step + exit-code gating" — the same keys, plus stepping
   and gating. Mode label `Mult`.
3. **Push/pop input stack** — a one-item stack that **pushes** the current input
   buffer **and** its mode, drops to `norm` until the next submit, then **pops**
   to restore the buffer and mode. Lets a user duck out to run an ad-hoc command
   in the middle of a sequence and come back to exactly where they were.
4. **Slash commands** — `/save <file>` writes the previous block's exact stored
   output to a file; `/filter <cmd>` pipes the previous output through `<cmd>` via
   the shell and the result **becomes the new previous output** (so it chains);
   `/load <file>` reads a script's lines into the LAAT buffer and enters LAAT.

All new keys (the `Ctrl+1` mode toggle and the `Ctrl+Alt+Enter` push/pop) and
the three slash commands are registered as **named, rebindable actions** in the
sprint-006 keymap engine — discoverable via `/keys`, with `Ctrl+1` and
`Ctrl+Alt+Enter` shipping as the **defaults**. Per-mode keymap config sections
(`[keymap.laat]` / `[keymap.mult]`) are explicitly **out of scope** this sprint;
the bindings are global named actions, not mode-scoped config tables.

This realizes the resolved decisions recorded in
[pre-plan-007-laat-mode.md](../planning/pre-plan-007-laat-mode.md): execution
gates on completion with exit-code-driven advancement; `LaaT`/`1T` mode labels;
`Ctrl+1` enters `Mult` from `norm` and toggles `Mult ↔ LAAT` once multi-line;
`/filter` runs via the shell and chains; selection and submission stay separate in
all modes; `/save` and `/load` resolve paths relative to kapollo's cwd with `~`
expansion; the stashed draft survives until popped.

> **Naming note**: the Line-At-A-Time mode uses three deliberate casings — the
> status label is `LaaT` (full) / `1T` (short), running prose says "LAAT", and the
> code's mode enum variant is `Laat`. These are intentional and should not be
> "normalized" to a single spelling.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Step through a sequence of commands one at a time (Priority: P1)

A user pastes or types several commands into the input buffer, enters LAAT mode,
and walks through the lines one by one — submitting the highlighted line, watching
it run to completion, and advancing only when it succeeds. When a line fails, the
highlight stays put and is flagged so the user can fix it before continuing.

**Why this priority**: This is the headline feature of the sprint and the reason
LAAT exists — running a script-like sequence interactively with a safety gate
between each step. It is independently demonstrable with nothing but a multi-line
buffer and the mode toggle.

**Independent Test**: Type three commands across three lines, toggle into LAAT,
and confirm the first line is highlighted. Submit it with `Enter`; on exit `0`
confirm the highlight advances to line two. Submit a line that exits non-zero and
confirm the highlight stays put and that line's highlight background changes to
the probable-failure flag.

**Acceptance Scenarios**:

1. **Given** a multi-line buffer in LAAT mode, **When** the user enters the mode,
   **Then** the first line is highlighted and the status mode field shows `1T`.
2. **Given** a highlighted line in LAAT, **When** the user presses `Enter`,
   **Then** that line is submitted, runs to completion, and its output appears in
   the transcript exactly as a normal single-line submission would.
3. **Given** a submitted LAAT line that exits `0`, **When** the command finishes,
   **Then** the highlight advances to the next line.
4. **Given** a submitted LAAT line that exits non-zero, **When** the command
   finishes, **Then** the highlight stays on that line and its highlight
   background changes to flag a probable failure.
5. **Given** the last line of the buffer has been submitted and succeeded,
   **When** the command finishes, **Then** there is no next line to advance to and
   the buffer is complete.

---

### User Story 2 - Edit multi-line input naturally in `Mult` mode (Priority: P1)

A user starts typing, adds a second line, and expects arrow keys to move the
caret between lines so they can fix a typo on an earlier line — without throwing
the buffer away. When the caret is already at the top edge, `Up` recalls history
chat-style and stashes the draft so `Down` brings it back.

**Why this priority**: Sprint 005's multi-line buffer recalls history on `Up`/
`Down`, which discards an in-progress multi-line draft — a sharp edge found in the
005 walkthrough. `Mult` mode fixes that and is the foundation LAAT builds on
(LAAT = `Mult` + highlight + step + gating), so it is co-critical with Story 1.

**Independent Test**: Type a line, add a second line (entering `Mult`), type a
second line, then press `Up` and confirm the caret moves up a line rather than
recalling history. With the caret on the first line, press `Up` again and confirm
the draft is stashed and the previous history entry is recalled; press `Down` and
confirm the stashed draft is restored.

**Acceptance Scenarios**:

1. **Given** a single-line buffer, **When** the user adds a second line via the
   newline action (`Alt+Enter`), **Then** the buffer enters `Mult` mode and the
   status mode field shows `Mult`.
2. **Given** a multi-line buffer in `Mult` mode with the caret below the first
   line, **When** the user presses `Up`, **Then** the caret moves up one line and
   no history recall occurs.
3. **Given** a multi-line buffer in `Mult` mode with the caret on the first line,
   **When** the user presses `Up`, **Then** the current draft is stashed and the
   previous history entry is recalled.
4. **Given** a recalled history entry after a stash, **When** the user presses
   `Down` from the top, **Then** the stashed draft is restored unchanged.
5. **Given** a multi-line buffer in `Mult` mode, **When** the user deletes content
   until only one line remains, **Then** the buffer transitions back to `norm`.
6. **Given** a `Mult` buffer, **When** the user presses plain `Enter`, **Then**
   the whole buffer is submitted as a single combined submission.

---

### User Story 3 - `/save` and `/filter` the previous command's output (Priority: P2)

A user runs a command, then saves its exact output to a file with `/save <file>`,
or pipes it through another command with `/filter <cmd>` — and because the filter
result becomes the new previous output, they can chain filters.

**Why this priority**: These were deferred from sprint 004 and complete the
"work with prior output" half of the mental model. They depend on the existing
block store but not on LAAT, so they are independently valuable but ranked below
the modal-input core.

**Independent Test**: Run a command, then `/save out.txt` and confirm the file
contains the previous block's exact stored output. Run a command producing many
lines, then `/filter rg <pattern>` and confirm a new transcript block titled
`/filter rg <pattern>` shows only matching lines; run a second `/filter` and
confirm it operates on the first filter's output.

**Acceptance Scenarios**:

1. **Given** a completed command with stored output, **When** the user runs
   `/save <file>`, **Then** the file is written with the previous block's exact
   stored output, with the path resolved relative to kapollo's cwd and `~`
   expanded.
2. **Given** `/save` with no path, **When** the user submits it, **Then** the
   status shows `'/save' requires path` and the input buffer is **not** cleared so
   the user can add a path and resubmit (or `Esc`).
3. **Given** `/save <file>` where the file already exists, **When** the user
   submits it, **Then** an interactive prompt offers
   `File exists, [O]verwrite, [A]ppend, [C]ancel?` and acts on the chosen key.
4. **Given** `/save` when the previous block is unavailable or evicted, **When**
   the user submits it, **Then** the status shows
   `Save failed, previous buffer not found`.
5. **Given** a completed command, **When** the user runs `/filter <cmd>`, **Then**
   a new transcript block titled `/filter <cmd>` is created by piping the previous
   output through `<cmd>` via the shell, and that block becomes the new previous
   output.
6. **Given** a `/filter` whose `<cmd>` exits non-zero, **When** it completes,
   **Then** the status exit code reflects the non-zero exit **and** the status
   message `filter non-zero exit` is shown (non-zero is often fine, e.g. a
   no-match search).
7. **Given** a `/filter` result, **When** the user runs another `/filter <cmd2>`,
   **Then** the second filter operates on the first filter's output (chaining).

---

### User Story 4 - Push/pop the input buffer to run an ad-hoc command (Priority: P2)

Mid-sequence, a user needs to run a one-off command without losing their composed
buffer and mode. They push (saving buffer + mode and dropping to `norm`), run an
ad-hoc command, and the buffer and mode are restored on the next submit.

**Why this priority**: The push/pop stack is the primitive that makes LAAT's
failure-recovery story complete (push → fix → pop → continue) and is reusable by
future modes, but the core stepping and editing stories deliver value without it.

**Independent Test**: Compose a multi-line `Mult`/LAAT buffer, push it, confirm
the mode drops to `norm` and the buffer is empty for ad-hoc input. Run a command,
then confirm the next submit pops the saved buffer and mode back exactly as they
were.

**Acceptance Scenarios**:

1. **Given** a `Mult` or LAAT buffer, **When** the user invokes the push action
   (`Ctrl+Alt+Enter`), **Then** the current buffer and mode are saved, the mode
   drops to `norm`, and the input buffer is cleared for ad-hoc input.
2. **Given** a pushed state, **When** the user submits an ad-hoc command, **Then**
   the command runs and the saved buffer and mode are popped and restored.
3. **Given** a pushed `Mult` buffer that included a stashed history draft, **When**
   it is popped, **Then** the buffer, its mode, and the stashed draft are all
   restored (the stash survives until popped).
4. **Given** an already-pushed state, **When** the user pushes again, **Then** the
   one-item stack semantics are preserved (no second slot is created).

---

### User Story 5 - Load a script into LAAT mode (Priority: P3)

A user runs `/load <file>` to read a script file's lines into the LAAT buffer —
one command per line — landing directly in LAAT ready to step through the script.

**Why this priority**: `/load` is a convenience entry path that makes LAAT useful
for real scripts, but typing or pasting lines already covers the core flow, so it
ranks last.

**Independent Test**: Create a file with several command lines, run
`/load <file>`, and confirm each line becomes a separate line in the LAAT buffer,
the mode is LAAT (`1T`), and the first line is highlighted.

**Acceptance Scenarios**:

1. **Given** a script file with one command per line, **When** the user runs
   `/load <file>`, **Then** each line is loaded as a separate LAAT buffer line and
   the mode becomes LAAT with the first line highlighted.
2. **Given** `/load <file>`, **When** the path is resolved, **Then** it is
   relative to kapollo's cwd (following `cd`) with `~` expanded.

---

### Edge Cases

- **Toggle from `norm`**: `Ctrl+1` in `norm` enters `Mult` even on a single or
  empty line; once the buffer is multi-line, `Ctrl+1` toggles `Mult ↔ LAAT`.
- **Selecting then submitting in any mode**: selection and submission stay
  separate; selecting multiple lines (e.g. via `Shift+Arrow`) and pressing `Enter`
  submits them as **one combined submission**, not sequential gated steps — in
  `norm`, `Mult`, and LAAT alike.
- **LAAT failure recovery**: on a probable failure the user may rerun with `Enter`
  (if the issue is now fixed), treat the non-zero exit as success by pressing
  `Down` then `Enter` to advance, abort the whole buffer with `Esc Esc`, or push
  the buffer, fix the issue, pop, and continue.
- **Leaving `Mult`**: a user leaves `Mult` via `Esc Esc`, by submitting the
  buffer, by pushing the buffer, or by deleting back to a single line (which
  returns to `norm`).
- **LAAT line output association**: a submitted LAAT line behaves exactly like a
  normal single-line submission — its output goes to the transcript and the last
  exit code is shown in the status bar.
- **`/save` to an existing file, cancelled**: choosing `[C]ancel` at the overwrite
  prompt leaves the file untouched and the buffer state intact.
- **`/filter` with no previous output**: when there is no previous block to pipe,
  the filter cannot run; the status reflects that the previous buffer was not
  found (consistent with `/save`).
- **`/load` of a missing or unreadable file**: the load fails with a status
  message and does not enter LAAT with a partial buffer.
- **Rebinding the new actions**: `Ctrl+1` and `Ctrl+Alt+Enter` are named
  actions; a user may rebind them via the keymap config, and `/keys` lists them.

## Requirements *(mandatory)*

### Functional Requirements

#### LAAT mode

- **FR-001**: The system MUST provide a Line-At-A-Time (LAAT) mode in which a
  multi-line input buffer is treated as a sequence of commands, one command per
  line, with the status mode field showing `1T` and the full label `LaaT`.
- **FR-002**: In LAAT mode the system MUST highlight exactly one current line and
  MUST move that highlight between lines via the arrow keys (the highlight tracks
  the caret line).
- **FR-003**: In LAAT mode `Enter` MUST submit the highlighted line(s), and the
  submission MUST wait for the command to complete before deciding whether to
  advance. When a multi-line selection is active, `Enter` MUST instead submit the
  selection as one combined submission per FR-017 (selection overrides the
  highlight).
- **FR-004**: When a submitted LAAT line exits `0`, the system MUST advance the
  highlight to the next line; when it exits non-zero, the system MUST keep the
  highlight on the current line and change that line's highlight background to
  flag a **probable** failure.
- **FR-005**: A submitted LAAT line MUST behave like a normal single-line
  submission for output association — its output is written to the transcript and
  the last exit code is shown in the status bar.
- **FR-006**: The system MUST let the user recover from a probable failure by
  re-running the line with `Enter`, by advancing past it with `Down` then `Enter`
  (treating the non-zero exit as success), or by aborting the whole buffer with
  `Esc Esc`.
- **FR-007**: Leaving LAAT mode MUST clear the LAAT buffer.

#### `Mult` mode

- **FR-008**: The system MUST provide a `Mult` multi-line editing mode entered
  when the buffer gains a second line via the newline action (`Alt+Enter`), with
  the status mode field showing `Mult`.
- **FR-009**: In `Mult` mode `Up`/`Down` MUST move the caret between lines of the
  buffer rather than recalling input history.
- **FR-010**: In `Mult` mode, when the caret is already on the first line, `Up`
  MUST stash the current draft and recall the previous history entry; continued
  `Up` walks older entries. `Down` walks newer entries and, when stepping past the
  newest entry, MUST restore the stashed draft (chat-style edge recall). `Down`
  never recalls older entries.
- **FR-011**: The stashed draft MUST survive mode switches and the push/pop stack
  and MUST persist until popped (it is not cleared by an intervening recall).
- **FR-012**: Deleting a `Mult` buffer back to a single line MUST transition the
  mode back to `norm`.
- **FR-013**: Plain `Enter` in `Mult` mode MUST submit the entire buffer as a
  single combined submission.
- **FR-014**: The user MUST be able to leave `Mult` mode via `Esc Esc`, by
  submitting the buffer, by pushing the buffer, or by deleting back to one line
  (the delete-to-one-line transition is the normative rule in FR-012).

#### Mode entry and toggling

- **FR-015**: A named, rebindable action (default `Ctrl+1`) MUST, when invoked
  in `norm`, enter `Mult` mode even when the buffer is a single or empty line.
- **FR-016**: Once the buffer is multi-line, the same toggle action
  (default `Ctrl+1`) MUST toggle between `Mult` and LAAT modes.
- **FR-017**: Selection and submission MUST remain separate concerns in `norm`,
  `Mult`, and LAAT: selecting multiple lines and pressing `Enter` MUST submit them
  as one combined submission, not as sequentially gated steps.

#### Push/pop input stack

- **FR-018**: A named, rebindable action (default `Ctrl+Alt+Enter`) MUST push the
  current input buffer **and** its mode, drop the mode to `norm`, and clear the
  input buffer for ad-hoc entry.
- **FR-019**: After a push, the **next** submit MUST pop and restore the saved
  buffer and mode (including any stashed draft). **Any** submitted line pops the
  stack — a shell command or a slash command alike; if the user wants to keep the
  pushed state longer, they re-push (and re-stash) after the ad-hoc submission.
- **FR-020**: The push/pop stack MUST be a single-item stack; a second push while
  already pushed MUST NOT create a second slot.

#### Slash commands

- **FR-021**: `/save <file>` MUST write the previous block's exact stored output
  to `<file>`, resolving the path relative to kapollo's current working directory
  (which follows `cd`) with `~` tilde expansion.
- **FR-022**: `/save` with no path MUST show the status message
  `'/save' requires path` and MUST NOT clear the input buffer, so the user can add
  a path and resubmit or cancel.
- **FR-023**: `/save <file>` to a file that already exists MUST present an
  interactive prompt `File exists, [O]verwrite, [A]ppend, [C]ancel?` and act
  according to the chosen option.
- **FR-024**: `/save` when the previous block is unavailable or evicted MUST show
  the status message `Save failed, previous buffer not found`.
- **FR-025**: `/filter <cmd>` MUST run `<cmd>` via the shell (so pipes, globs, and
  aliases work), piping the previous block's output into it, and MUST create a new
  transcript block titled `{leader}filter <cmd>` (using the effective configured
  leader character, consistent with the `/help` and `/keys` synthetic blocks).
- **FR-026**: The output of a `/filter` block MUST become the new previous output
  so that a subsequent `/filter` operates on it (chaining).
- **FR-027**: When a `/filter` command exits non-zero, the system MUST update the
  status exit code **and** show the status message `filter non-zero exit`.
- **FR-028**: `/load <file>` MUST read the file's lines into the LAAT buffer (one
  command per line) and enter LAAT mode with the first line highlighted, resolving
  the path relative to kapollo's cwd with `~` tilde expansion.

#### Keymap integration and documentation

- **FR-029**: The new mode-toggle and push/pop bindings MUST be registered as
  named, rebindable actions in the sprint-006 keymap engine, listed by `/keys`,
  with `Ctrl+1` and `Ctrl+Alt+Enter` as defaults.
- **FR-030**: The `/save`, `/filter`, and `/load` slash commands MUST be
  registered in the slash-command registry alongside the existing commands.
- **FR-031**: LAAT mode, `Mult` mode, the push/pop stack, and the three new slash
  commands MUST be documented in user-facing docs, including the mode labels and
  default bindings.

### Key Entities *(include if feature involves data)*

- **Input mode**: the current editing mode of the input buffer — `norm`, `Mult`,
  or `LaaT` — surfaced in the status bar's 4-character mode field (`Mult`, `1T`).
- **LAAT buffer**: the multi-line sequence of commands in LAAT mode, with a single
  highlighted current line and a per-line probable-failure flag.
- **Highlight**: the marker for the current LAAT line; advances on exit `0`, stays
  and is flagged on non-zero exit.
- **Stashed draft**: the temporary buffer saved when chat-style history recall
  begins at an edge; restored on `Down` and persisting until popped.
- **Input stack entry**: the single saved `(buffer, mode, stashed draft)` snapshot
  held by the one-item push/pop stack.
- **Previous block**: the most recent completed command's stored output in the
  block store, the source for `/save` and `/filter` and the target a `/filter`
  result replaces.
- **Named action**: a stable, rebindable behavior (the mode toggle and the
  push/pop action) registered in the keymap engine and listed by `/keys`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can load or type a sequence of commands, step through them
  one at a time in LAAT, and the highlight advances only on exit `0` while a
  non-zero exit visibly flags the failing line — 100% of the time.
- **SC-002**: In `Mult` mode, editing a typo on an earlier line by pressing `Up`
  never discards the multi-line draft, eliminating the sprint-005 buffer-loss
  sharp edge.
- **SC-003**: Chat-style edge recall is reversible: a stashed draft is restored
  byte-for-byte on `Down`, and survives a push/pop round-trip.
- **SC-004**: `/save` writes the previous block's exact stored output, and every
  error path (no path, existing file, missing previous block) produces a clear,
  documented status message or prompt rather than silent failure.
- **SC-005**: `/filter` reproduces a shell pipe of the previous output and can be
  chained, with non-zero exits surfaced (exit code plus message) rather than
  hidden.
- **SC-006**: A user can push the input buffer, run an ad-hoc command, and have
  their buffer and mode restored exactly on the next submit, with the one-item
  stack never silently dropping their saved state.
- **SC-007**: Both new default bindings (`Ctrl+1`, `Ctrl+Alt+Enter`) and the
  three new slash commands are discoverable via `/keys` and the slash registry and
  are rebindable through config.

## Assumptions

- The sprint-005 multi-line input buffer and the reserved 4-character status mode
  field are the foundation this feature builds on; `Mult` and `LaaT` populate that
  field with `Mult` and `1T`.
- The sprint-006 named-action keymap engine is the authoritative binding
  mechanism; the new toggle and push/pop bindings are added as global named
  actions (not mode-scoped config tables) and `Ctrl+1` / `Ctrl+Alt+Enter` are
  their defaults.
- `InputHistory` already tracks a recall cursor (`None` = live draft); it is
  extended to also hold the stashed draft buffer so the temporary content can be
  restored on `Down`.
- The sprint-004 block store and `/save` / `/filter` seam already exist (kwi
  WI #43, #44); this feature wires the slash commands to them.
- "Probable failure" is deliberate wording: some commands use non-zero exit codes
  for success (e.g. a no-match search, `robocopy`), so the flag invites review
  rather than asserting failure.
- Path resolution for `/save` and `/load` follows kapollo's current working
  directory (which tracks `cd`) with `~` expansion, consistent with the shell.
- Platform is Linux-first, consistent with prior sprints.

## Out of Scope (Non-Goals)

- **Per-mode keymap config sections** (`[keymap.laat]` / `[keymap.mult]`) — the
  new bindings are named, rebindable actions, but mode-scoped config tables wait
  for a later sprint.
- **Persisting LAAT buffers between sessions** — LAAT buffers live only for the
  current session.
- **Sequential gated submission of a multi-line selection** — selecting several
  lines and pressing `Enter` submits them as one combined submission, not as
  per-line gated steps.
- **A multi-item push/pop stack** — exactly one saved slot this sprint.

## Dependencies

- **Sprint 005** — the multi-line input buffer and the reserved 4-character status
  mode field this feature populates and extends.
- **Sprint 006** — the named-action keymap engine the new bindings register with
  and that `/keys` lists.
- **Sprint 004** — the block store and the `/save` / `/filter` seam (kwi WI #43,
  #44) the new slash commands build on.
- Resolved pre-planning decisions in
  [pre-plan-007-laat-mode.md](../planning/pre-plan-007-laat-mode.md) (exit-code
  gating, `LaaT`/`1T` labels, `Ctrl+1` entry/toggle, `Ctrl+Alt+Enter`
  push/pop, shell-run chaining `/filter`, cwd-relative `/save` and `/load`,
  stashed draft surviving until popped, selection/submission separation).
