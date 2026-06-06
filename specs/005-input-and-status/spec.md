# Feature Specification: Input Editing & Fixed Status Bar

**Feature Branch**: `005-input-and-status`  
**Created**: 2026-06-05  
**Status**: Draft  
**Input**: User description: "Make kapollo's input pad and scrollback feel like a real shell line editor, plus a small always-on fixed-format status bar. Keys ship hardcoded this sprint (configurability is a later sprint), but each behavior must be a named action so a future keymap engine can bind it."

## Overview

After the grid rework (004), kapollo renders like a native terminal but still
edits like a toy: the input pad lacks word/line motion, keyboard selection, and
the kill commands every shell user reaches for; a multi-line paste auto-submits
each line; and there is no always-on surface for mode, working directory, or
status messages. This feature makes the input pad and scrollback feel like a
real shell **line editor** and adds a small, **fixed-format status bar**.

The work is deliberately scoped to land daily-driver value without taking on the
larger systems it sets up. Keys ship **hardcoded** this sprint, but every
behavior is expressed as a **named action** so the keymap engine (sprint 006)
can later bind a default and an alternate per action with no behavioral rewrite.
The status bar uses a **fixed layout** (no template language — that is sprint
008). LAAT mode, the push/pop input stack, and `/save`/`/filter` keybindings
(sprint 007) are explicitly out of scope.

This realizes the resolved decisions recorded in
[pre-plan-005-input-and-status.md](../planning/pre-plan-005-input-and-status.md):
single selection across both pads, current-line `Home`/`End` with `Esc Esc`
buffer-clear in multi-line buffers, status-message lifetime tied to the next
submit, and the fixed `mode | cwd<greedypad>| message | exit` status layout.

The stable plumbing (PTY, config, slash registry, input router, grid, block
store, chrome) carries over unchanged; this feature adds an input-editing action
layer, reworks bracketed-paste handling, retargets the scrollback keys, and
introduces the status-bar chrome and its `/status` and `/keys` slash commands.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Shell-grade line editing in the input pad (Priority: P1)

A user composes and edits commands in the input pad using the motion, selection,
and kill keys they already know from a real shell — moving by word and line,
extending a selection from the keyboard, and killing to the start or end of the
line — and it all behaves correctly whether the buffer is a single line or
several lines.

**Why this priority**: This is the headline "I'll actually use it daily" win and
the foundation the rest of the sprint builds on. Word/line motion, keyboard
selection, and the kill commands are the difference between a usable composer and
a frustrating one, and they are independently demonstrable without the status bar
or the paste rework.

**Independent Test**: In both a single-line and a multi-line buffer, use `Home`/
`End` to jump within the current line, `Ctrl+Left`/`Ctrl+Right` to move by word
across punctuation, `Shift`-arrow combinations to grow a selection by char and by
word, and `Ctrl+U`/`Ctrl+K`/`Ctrl+W` to kill to start, to end, and the word
before the cursor. Confirm each operates on the **current line** and that the
selection/caret end up where a shell user expects.

**Acceptance Scenarios**:

1. **Given** the caret is mid-line, **When** the user presses `Home` then `End`,
   **Then** the caret moves to the start, then the end, of the **current line**
   (not the whole buffer).
2. **Given** a line containing words separated by spaces and punctuation, **When**
   the user presses `Ctrl+Left`/`Ctrl+Right`, **Then** the caret moves by word
   with punctuation treated as boundaries (punctuation-aware motion).
3. **Given** no active selection, **When** the user presses `Shift+Left`/
   `Shift+Right`, **Then** a selection starts and extends one character per press;
   `Shift+Ctrl+Left`/`Shift+Ctrl+Right` extend the selection by a word.
4. **Given** the caret is mid-line, **When** the user presses `Ctrl+U`, **Then**
   text from the line start to the caret is deleted; **When** the user presses
   `Ctrl+K`, **Then** text from the caret to the line end is deleted.
5. **Given** the caret follows one or more words, **When** the user presses
   `Ctrl+W`, **Then** the word before the caret is deleted using the readline
   whitespace rule (consume preceding whitespace, then the preceding non-whitespace
   run).
6. **Given** a multi-line buffer with the caret on an interior line, **When** the
   user performs any motion, selection, or kill command, **Then** it operates on
   that current line exactly as it would in a single-line buffer.

---

### User Story 2 - Multi-line paste lands as one buffer, only Enter submits (Priority: P1)

A user pastes a multi-line block (e.g. a script snippet) into the input pad and
it lands as a **single multi-line input buffer**, one line per pasted line, with
nothing submitted until they press `Enter` — so they can review and edit before
running it.

**Why this priority**: The current behavior — every embedded newline submits a
line — actively corrupts pasted scripts and is a daily hazard. Fixing it is small
but high-value, and it is independently testable from the editing and status work.

**Independent Test**: Paste a 3-line snippet. Confirm all three lines appear in
the input pad as one buffer with no command submitted, the caret lands at the end
of the pasted content, the buffer is fully editable, and pressing `Enter` submits
the **entire** buffer as one command.

**Acceptance Scenarios**:

1. **Given** the clipboard holds multiple lines, **When** the user pastes,
   **Then** every line appears in the input pad as one multi-line buffer (one line
   per pasted line) and **no** command is submitted.
2. **Given** a freshly pasted multi-line buffer, **When** the user presses
   `Enter`, **Then** the whole buffer is submitted as a single command.
3. **Given** a pasted multi-line buffer, **When** the user edits it before
   submitting, **Then** all line-editing commands from User Story 1 apply normally
   across the pasted lines.
4. **Given** a paste whose final line has no trailing newline, **When** it lands,
   **Then** the caret rests at the end of the pasted content and the buffer is not
   submitted.

---

### User Story 3 - Scrollback polish with retargeted keys (Priority: P2)

A user navigates scrollback with page and line granularity that keeps a few lines
of context across a page jump, and reaches the very top or bottom with a single
keystroke — using keys that no longer collide with line editing.

**Why this priority**: Scrollback navigation already exists from 004; this story
refines it (context-preserving page scroll, line-granular `Shift+PageUp/Down`,
top/bottom jumps) and frees `Home`/`End` for line editing. It depends on User
Story 1 having claimed `Home`/`End` but is otherwise an independent slice.

**Independent Test**: With more output than fits on screen, press `PageUp`/
`PageDown` and confirm each jump moves by one page **minus the context lines**
(default 3) and always advances at least one line even on a short pad; press
`Shift+PageUp`/`Shift+PageDown` and confirm single-line movement; press
`Shift+Home`/`Shift+End` and confirm jumps to the top and bottom of scrollback.

**Acceptance Scenarios**:

1. **Given** scrollback taller than the output pad, **When** the user presses
   `PageUp` or `PageDown`, **Then** the view moves by one page minus the context
   lines (default 3).
2. **Given** an output pad small enough that page-minus-context would be zero or
   negative, **When** the user presses `PageUp`/`PageDown`, **Then** the view
   still advances by at least one line.
3. **Given** any scroll position, **When** the user presses `Shift+PageUp`/
   `Shift+PageDown`, **Then** the view moves exactly one line up/down.
4. **Given** any scroll position, **When** the user presses `Shift+Home`, **Then**
   the view jumps to the oldest output; **When** the user presses `Shift+End`,
   **Then** it jumps to the newest output.
5. **Given** line editing is active, **When** the user presses `Home`/`End`,
   **Then** they act on the input line and do **not** scroll the transcript.

---

### User Story 4 - Fixed-format status bar (Priority: P2)

A user sees a single always-on status line beneath the input pad showing the
current mode, working directory, the latest status message, and the last exit
code — in a stable, fixed layout that does not reflow as content changes — and can
toggle it on or off.

**Why this priority**: The status bar surfaces context (cwd, exit code, messages)
that today has nowhere to live, and it reserves the mode field that later sprints
(LAAT) depend on. It is demonstrable on its own and only loosely couples to the
other stories via the status-message lifetime rule.

**Independent Test**: With the terminal at least 10 rows tall, confirm a single
status line renders beneath the input pad in the layout
`mode | cwd<greedypad>| message | exit`, with the message right-justified into the
remaining width and no `|` separator after `cwd`. Run `/status` and confirm it
toggles the bar off and on. Shrink the terminal below 10 rows and confirm the bar
auto-hides; grow it back and confirm it returns.

**Acceptance Scenarios**:

1. **Given** the terminal has at least 10 rows and the status bar is enabled,
   **When** the UI renders, **Then** a single status line appears beneath the
   input pad laid out as `mode | cwd<greedypad>| message | exit`.
2. **Given** the status line renders, **When** the available width changes,
   **Then** the greedy pad between `cwd` and `message` absorbs the slack (no `|`
   after `cwd`) and `message` stays right-justified into the remaining width.
3. **Given** the default shell mode, **When** the mode field renders, **Then** it
   occupies a fixed 4-character mixed-case field (e.g. `LaaT`-width) so future
   modes do not reflow the layout.
4. **Given** the status bar is enabled, **When** the user runs `/status`, **Then**
   the bar is hidden; running `/status` again re-enables it.
5. **Given** the terminal has fewer than 10 rows, **When** the UI renders,
   **Then** the status bar is auto-hidden regardless of the enabled setting, and
   it reappears when the terminal grows back to 10 or more rows.
6. **Given** a completed command with an exit code, **When** the status line
   renders, **Then** the `exit` field reflects that exit code.

---

### User Story 5 - Status message lifetime & single-selection arbitration (Priority: P3)

A user sees a status message persist until their next command, and experiences a
single, unambiguous selection across both the input pad and the transcript — which
in turn makes `Ctrl+C` and `Esc` behave predictably.

**Why this priority**: This story ties the surfaces together: it defines when a
status message clears and enforces the one-selection-at-a-time rule that
disambiguates `Ctrl+C` (copy vs. interrupt) and `Esc` (cancel selection vs. clear
line vs. clear buffer). It depends on the status bar (US4) and selection (US1)
existing, so it is sequenced last.

**Independent Test**: Trigger a status message, run several non-submitting
actions, and confirm it persists; press `Enter` and confirm it clears; trigger it
again and press `Esc Esc` and confirm it clears. Start a selection in the input
pad, then start one in the transcript, and confirm the input-pad selection is
cleared (and vice-versa). With an active selection press `Ctrl+C` and confirm a
copy; with no selection press `Ctrl+C` and confirm an interrupt.

**Acceptance Scenarios**:

1. **Given** a status message is shown, **When** the user performs any number of
   non-submitting actions, **Then** the message persists (it is not on a timeout).
2. **Given** a status message is shown, **When** the user submits a command with
   `Enter`, **Then** the message clears.
3. **Given** a status message is shown, **When** the user presses `Esc` twice
   (double `Esc`), **Then** the message clears.
4. **Given** a selection is active in one pad, **When** the user starts a selection
   in the other pad, **Then** the first selection is cleared so at most one
   selection is active across both pads.
5. **Given** an active selection (in either pad), **When** the user presses
   `Ctrl+C`, **Then** the selection is copied; **Given** no active selection,
   **When** the user presses `Ctrl+C`, **Then** SIGINT is sent to the child.
6. **Given** an active selection, **When** the user presses `Esc`, **Then** the
   selection is cancelled; **Given** no selection in a single-line buffer, **When**
   the user presses `Esc`, **Then** the current line is cleared; **Given** a
   multi-line buffer with no selection, **When** the user presses `Esc`, **Then**
   only the current line clears, and `Esc Esc` clears the whole buffer.

---

### Edge Cases

- **Word motion at buffer edges**: `Ctrl+Left` at the start of a line and
  `Ctrl+Right` at the end of a line resolve sensibly (stop at the line boundary)
  rather than silently crossing into adjacent lines or no-op'ing ambiguously.
- **Kill on an empty or whitespace-only region**: `Ctrl+U`/`Ctrl+K`/`Ctrl+W` with
  nothing to delete (caret at line start/end, or only whitespace before the caret)
  are well-defined no-ops or consume exactly the whitespace per the readline rule.
- **Paste containing only a single line or only newlines**: a single-line paste
  behaves as an ordinary insert; a paste that is purely newlines yields the
  corresponding empty lines in one buffer without submitting.
- **Page scroll on a very short pad**: when page-minus-context underflows, the
  at-least-one-line clamp guarantees progress in both directions.
- **Status overflow**: a `cwd` or `message` longer than the available width is
  truncated so the fixed layout (and the `mode`/`exit` fields) never breaks across
  lines or pushes the bar to a second row.
- **Terminal at exactly 10 rows / crossing the threshold during resize**: the bar
  appears at 10 rows and hides at 9, switching cleanly on resize without leaving
  artifacts.
- **Double-Esc disambiguation**: a single `Esc` that cancels a selection or clears
  a line must not also clear the status message in the same keystroke; the message
  clear requires the explicit second `Esc` (or a submit).
- **Esc Esc timing**: the two `Esc` presses are treated as a buffer-clear /
  message-clear gesture without depending on a wall-clock double-press timeout.

## Requirements *(mandatory)*

<!--
  Keys are HARDCODED this sprint. Each FR names the action it realizes so the
  keymap engine (006) can bind default + alternate per action without a rewrite.
-->

### Functional Requirements

#### Input-line editing (US1)

- **FR-001**: System MUST move the caret to the start of the **current line** on
  `Home` (action `line_move_start`) and to the end of the current line on `End`
  (action `line_move_end`), operating on the current line in both single- and
  multi-line buffers.
- **FR-002**: System MUST move the caret left to the start of the previous word on
  `Ctrl+Left` (action `word_move_left`) and right to the end of the next word on
  `Ctrl+Right` (action `word_move_right`), using **punctuation-aware** word
  boundaries.
- **FR-003**: System MUST start or extend a character-wise selection on
  `Shift+Left` (action `select_char_left`) and `Shift+Right` (action
  `select_char_right`).
- **FR-004**: System MUST start or extend a word-wise selection on
  `Shift+Ctrl+Left` (action `select_word_left`) and `Shift+Ctrl+Right` (action
  `select_word_right`).
- **FR-005**: System MUST delete from the caret to the start of the current line on
  `Ctrl+U` (action `kill_to_line_start`) and from the caret to the end of the
  current line on `Ctrl+K` (action `kill_to_line_end`).
- **FR-006**: System MUST delete the word before the caret on `Ctrl+W` (action
  `delete_word_before`) using the readline **whitespace rule** (consume any
  immediately preceding whitespace, then the preceding non-whitespace run).
- **FR-007**: All editing, motion, selection, and kill actions MUST operate on the
  **current line** when the buffer is multi-line, behaving identically to the
  single-line case.
- **FR-008**: Each editing behavior MUST be implemented as a **named action**
  (per FR-001–FR-006) with a hardcoded default binding this sprint, structured so
  the future keymap engine can bind a default and an alternate per action without
  changing the action's behavior.
- **FR-009**: System MUST reserve the whole-buffer motion action names
  `multiline_move_start_buffer` and `multiline_move_end_buffer` as named but
  **unmapped** actions (no default binding this sprint).

#### Bracketed-paste rework (US2)

- **FR-010**: System MUST handle a bracketed paste (terminal paste event) by
  inserting the pasted content into the input pad as **one multi-line buffer**,
  one line per pasted line, preserving line breaks as buffer line boundaries.
- **FR-011**: A multi-line paste MUST NOT auto-submit any line; only `Enter`
  submits, and `Enter` MUST submit the **entire** buffer as a single command. This
  replaces the prior behavior where each embedded newline submitted a line.
- **FR-012**: After a paste, the caret MUST rest at the end of the inserted
  content and the buffer MUST remain fully editable by all US1 actions.

#### Scrollback polish (US3)

- **FR-013**: System MUST scroll the transcript up one page **minus the context
  lines** on `PageUp` (action `scroll_page_up`) and down one page minus the
  context lines on `PageDown` (action `scroll_page_down`).
- **FR-014**: The context-lines value MUST default to **3** and MUST be clamped so
  that a page scroll always advances by **at least one line**, even when the output
  pad is small enough that page-minus-context would otherwise be zero or negative.
- **FR-015**: System MUST scroll the transcript by exactly one line on
  `Shift+PageUp` (action `scroll_line_up`) and `Shift+PageDown` (action
  `scroll_line_down`).
- **FR-016**: System MUST jump to the oldest output (top of scrollback) on
  `Shift+Home` (action `scroll_to_top`) and to the newest output (bottom) on
  `Shift+End` (action `scroll_to_bottom`).
- **FR-017**: `Home`/`End` MUST act on the input line (FR-001) and MUST NOT scroll
  the transcript; their former scrollback jobs move to `Shift+Home`/`Shift+End`.

#### Fixed-format status bar (US4)

- **FR-018**: System MUST render a single-line status bar directly beneath the
  input pad, **enabled by default** and configurable on/off.
- **FR-019**: The status bar layout MUST be the **fixed** format
  `mode | cwd<greedypad>| message | exit`, where a greedy pad sits between `cwd`
  and `message` with **no `|` separator after `cwd`**, and `message` is
  right-justified into the remaining width. No template engine is used this sprint.
- **FR-020**: The status bar MUST reserve a **4-character mixed-case mode field**
  (e.g. `LaaT`) so future modes do not reflow the layout; this sprint the only
  mode is the default shell mode, rendered as the literal label `norm`.
- **FR-021**: The status bar MUST be **auto-hidden** when the terminal has fewer
  than **10 rows**, regardless of the enabled setting, and MUST reappear when the
  terminal grows back to 10 or more rows.
- **FR-022**: System MUST provide a `/status` slash command that toggles the
  status bar on and off.
- **FR-023**: The `exit` field MUST reflect the exit code of the most recently
  completed command, and the `cwd` field MUST reflect the current working
  directory.
- **FR-024**: When `cwd` or `message` exceeds the available width, the status bar
  MUST truncate content so the fixed layout never wraps to a second row or breaks
  the `mode`/`exit` fields.

#### Status message lifetime & selection arbitration (US5)

- **FR-025**: A status message MUST persist until the next submitted command
  (`Enter`) — it MUST NOT expire on a timeout — and submitting a command MUST clear
  it.
- **FR-026**: A double `Esc` MUST clear the current status message (action
  `clear_status_message`).
- **FR-027**: At most **one** selection MUST be active across the input pad and the
  transcript/output pad combined; starting a selection in one pad MUST clear any
  selection in the other.
- **FR-028**: With an active selection (in either pad), `Ctrl+C` MUST copy the
  selection and clear it; with no active selection, `Ctrl+C` MUST send SIGINT to
  the running child (copy MUST NOT shadow interrupt).
- **FR-029**: `Esc` semantics MUST be: with an active selection, cancel the
  selection; with no selection in a single-line buffer, clear the current line; in
  a multi-line buffer with no selection, a single `Esc` clears only the current
  line and `Esc Esc` clears the whole buffer.

#### Discoverability (cross-cutting)

- **FR-030**: System MUST provide a `/keys` slash command that lists the active
  (hardcoded) key map by action and binding.
- **FR-031**: `/help` MUST include a one-line pointer to `/keys`.

#### Cross-cutting integrity

- **FR-032**: System MUST preserve TUI integrity and the stable layers (PTY,
  config, slash registry, input router, grid, block store, existing chrome): no
  regression to existing slash commands, the grid/selection behavior from 004, or
  shell-wrapping behavior.
- **FR-033**: New configuration surface (status bar on/off, scrollback context
  lines) MUST integrate with the existing configuration, keeping existing config
  keys working.

### Key Entities *(include if feature involves data)*

- **Input Buffer**: The editable contents of the input pad — one or more logical
  lines with a caret position. The target of all motion, selection, and kill
  actions, which operate on the **current line** within it.
- **Named Action**: A behavior identified by a stable name (e.g.
  `kill_to_line_start`, `scroll_page_up`) with a hardcoded default binding this
  sprint; the unit the future keymap engine binds (default + alternate per action).
- **Key Map**: The set of named actions and their current (hardcoded) bindings,
  surfaced by `/keys` and pointed to from `/help`.
- **Selection**: A single content-anchored range that may live in either the input
  pad or the transcript; at most one exists at a time across both pads, and its
  presence/absence arbitrates `Ctrl+C` and `Esc`.
- **Scroll View State**: The transcript's current scroll offset plus the
  context-lines setting (default 3, clamped to advance ≥ 1 line per page),
  driving `PageUp/PageDown`, `Shift+PageUp/PageDown`, and `Shift+Home/End`.
- **Status Bar**: A single fixed-layout chrome line beneath the input pad with
  fields `mode | cwd<greedypad>| message | exit`; enabled by default, toggled by
  `/status`, auto-hidden below 10 rows, with a reserved 4-char mode field.
- **Status Message**: The transient `message` field content; persists until the
  next submit and is cleared by a submit or a double `Esc` (no timeout).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Word motion, line motion, keyboard selection, and the kill commands
  (`Ctrl+U`/`Ctrl+K`/`Ctrl+W`) all operate correctly in **both** single-line and
  multi-line buffers, verified across the US1 acceptance scenarios in **100%** of
  trials.
- **SC-002**: A multi-line paste lands as one buffer and **never** auto-submits;
  `Enter` submits the whole buffer, in **100%** of paste trials.
- **SC-003**: At most **one** selection is active across the input and transcript
  pads at any time — starting a selection in one pad clears the other — in
  **100%** of cross-pad trials.
- **SC-004**: `Ctrl+C` copies when a selection is active and sends SIGINT when none
  is, and `Esc`/`Esc Esc` follow the cancel→clear-line→clear-buffer rule, in
  **100%** of trials.
- **SC-005**: The status bar renders in the fixed `mode | cwd<greedypad>| message
  | exit` layout, toggles via `/status`, and auto-hides below 10 rows (reappearing
  at ≥ 10 rows), in **100%** of trials including width/resize changes.
- **SC-006**: A status message persists across non-submitting actions and clears on
  the next `Enter` **or** a double `Esc` (never on a timeout) in **100%** of
  trials.
- **SC-007**: `PageUp`/`PageDown` move by one page minus the context lines (default
  3) and always advance **at least one line** even on a short pad; `Shift+PageUp/
  PageDown` move exactly one line; `Shift+Home/End` jump to top/bottom — in
  **100%** of trials.
- **SC-008**: `/keys` lists every active hardcoded binding by action, and `/help`
  shows a one-line pointer to `/keys`, in **100%** of invocations.
- **SC-009**: No regression to existing slash commands, the 004 grid/selection
  behavior, or shell-wrapping for an equivalent command set (no functional
  regression).

## Assumptions

- **Keys are hardcoded this sprint**: configurability is sprint 006. Every behavior
  is nonetheless expressed as a **named action** with a default binding so 006 can
  bind default + alternate per action without behavioral changes.
- **Fixed status layout**: the status bar uses a fixed format with no template
  language (that is sprint 008). The reserved 4-char mode field anticipates future
  modes (e.g. LAAT) without reflowing the layout.
- **Punctuation-aware motion vs. whitespace-rule kill**: `Ctrl+Left/Right` word
  motion treats punctuation as boundaries; `Ctrl+W` uses the readline whitespace
  rule (resolved Q2 in the pre-plan).
- **Esc in multi-line buffers**: single `Esc` cancels a selection or clears the
  current line; `Esc Esc` clears the whole buffer (resolved Q3). The double-`Esc`
  gestures (buffer clear, status-message clear) do not depend on a wall-clock
  double-press timeout.
- **Single selection across pads**: builds on the 004 selection model; this sprint
  adds the input-pad selection and the cross-pad single-selection arbitration that
  disambiguates `Ctrl+C` and `Esc`.
- **Context lines default 3**: configurable, clamped so a page scroll always
  advances at least one line on small pads.
- **Platform**: Linux-first; built on the 004 grid, block store, and chrome, which
  carry over unchanged.
- **Constitution**: honors the constitution (TUI integrity VI especially); the
  combined spec (`docs/specification.md`), architecture, and usage docs are updated
  in the polish phase per Principles I, II, and V.

## Out of Scope (Non-Goals)

These are explicitly deferred and named so the boundary is unambiguous:

- **Configurable keymap engine** — binding default + alternate per action via
  config → **sprint 006**. This sprint only names the actions and hardcodes them.
- **Status template language** — a user-authored status format → **sprint 008**.
  This sprint ships a single fixed layout.
- **LAAT mode and the `Ctrl+Alt+Enter` push/pop input stack** → **sprint 007**.
  The 4-char mode field is reserved but only the default shell mode exists here.
- **`/save` and `/filter` slash commands** (and their keybindings) → **sprint
  007**.
- **Binding `copy_block_without_command` and `copy_current_line` to keys** — no
  good default chosen; waits for the keymap config (006). The block-aware copy
  affordances from 004 remain available via their existing (non-key) paths.
- **Mouse click-vs-drag threshold** — suppressing the stray one-cell selection on a
  bare click is a **known issue**, not a deliverable. crossterm reports mouse
  position at cell resolution only, so the "ignore sub-cell drags" rule is not
  directly expressible; it is tracked as **kwi research WI #45** and rolls forward.

## Dependencies

- The 004 grid rework: the grid, scrollback row anchoring, block store, selection
  model, and chrome that this feature edits on top of.
- Existing kapollo modules retained per the reuse map (PTY, config, slash registry,
  input router, input pad, transcript, status chrome).
- Resolved pre-planning decisions in
  [pre-plan-005-input-and-status.md](../planning/pre-plan-005-input-and-status.md)
  (single selection, `Esc`/`Esc Esc` semantics, fixed status layout, status-message
  lifetime, hardcoded-but-named actions).
- Sets up **sprint 006** (keymap engine binds these named actions) and **sprint
  007** (LAAT builds on the multi-line input buffer landed here); no dependency on
  sprint 008.
