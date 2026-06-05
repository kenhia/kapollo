# Feature Specification: Grid Rework — Native Terminal Grid, Mouse Selection & Block Store

**Feature Branch**: `004-grid-rework`  
**Created**: 2026-06-04  
**Status**: Draft  
**Input**: User description: "proceed with the spec — the new MVP will include the secondary store (command + output + exit code block, in memory only for MVP, but structured so adding a DB as a secondary backing will be natural)"

## Overview

kapollo's first MVP (sprints 001/002) rendered shell output as an append-only,
style-stripped transcript of **blocks** (command + output + exit code). Living with it
proved the original "no grid model" decision (D4) wrong: kapollo does not *feel* like a
native terminal. In-place redraws (progress bars, `\r` overwrites, cursor moves), inline
color, and general fidelity all suffer, and there is no way to offer mouse-driven text
selection without owning the cells on screen.

This feature reworks the rendering core in place (Path A of
[02-rework-vs-rewrite.md](../planning/grid-pivot/02-rework-vs-rewrite.md)) to model the
main screen as a **real terminal grid** with scrollback, add **mouse-driven selection,
copy, and scroll-wheel** support with correct hand-over to full-screen applications, and
re-home the **block model** as an annotation layer backed by an in-memory **block store**
that is structured so a database backing can be added later without reshaping callers.

The stable plumbing (PTY, config, slash registry, input router, chrome, shell hooks)
carries over largely unchanged. This realizes decisions **D25–D30** recorded during the
grid-pivot planning effort.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Native terminal rendering of the main screen (Priority: P1)

A user runs ordinary interactive commands — ones that draw progress bars, overwrite the
current line with `\r`, move the cursor, clear regions, or emit inline ANSI color — and
sees them rendered faithfully, exactly as they would appear in a native terminal, inside
kapollo's transcript area above the input pad.

**Why this priority**: This is the entire reason for the pivot. Faithful, in-place,
colorized rendering is the "feels like a real terminal" win, and every other capability in
this feature (selection over owned cells, scrollback, block annotation) is built on top of
the grid this story introduces. It is independently demonstrable and delivers the core
value even if nothing else ships.

**Independent Test**: Run a command with a progress bar (e.g. a long copy with a spinner),
a `\r`-overwriting counter, and a command that emits ANSI colors. Confirm the progress bar
updates **in place on one line** (no scrollback spam), the counter overwrites correctly,
colors render, and wide/combining characters display without corruption — matching how the
same program looks in the host terminal.

**Acceptance Scenarios**:

1. **Given** a command that prints a progress bar using carriage returns, **When** it runs,
   **Then** the bar updates on a single line in place rather than appending a new line per
   update.
2. **Given** a command that emits inline SGR color and text attributes (bold, underline,
   reverse), **When** its output renders, **Then** the colors and attributes appear in the
   transcript faithfully.
3. **Given** a program that moves the cursor and overwrites earlier cells on the current
   screen, **When** it redraws, **Then** the displayed grid reflects the final cell state,
   not a concatenation of every intermediate write.
4. **Given** output containing wide (CJK/emoji) and combining characters, **When** it
   renders, **Then** glyph width and placement are correct and columns do not drift.
5. **Given** a full-screen application is launched (e.g. `vim`, `htop`, `bpytop`), **When**
   it takes over the alternate screen, **Then** kapollo renders the alternate screen
   faithfully and restores the prior main-screen content cleanly on exit.

---

### User Story 2 - Mouse selection, copy, and scroll with correct app hand-over (Priority: P2)

A user selects text with click-and-drag directly in kapollo's transcript, copies it to the
clipboard, and scrolls back through history with the wheel — and when a full-screen
application wants the mouse itself, kapollo transparently hands mouse and keyboard input to
that application instead of intercepting it.

**Why this priority**: Mouse-driven selection is the headline new capability the grid
unlocks (owning the cells is what makes app-driven selection possible at all). It is the
most visible UX upgrade over the first MVP. It depends on US1's grid existing but is
otherwise an independent, demonstrable slice.

**Independent Test**: Click-drag to select a range of text; confirm a highlight follows the
selection and copying (right-click or Ctrl-C on an active selection) places the exact
selected text on the clipboard. Scroll the wheel to move through scrollback. Launch `vim`,
enable its mouse mode, and confirm clicks reach `vim`; hold Shift while dragging and confirm
the host terminal's native selection is used instead.

**Acceptance Scenarios**:

1. **Given** main-screen output is visible, **When** the user click-drags over a region,
   **Then** a selection highlight is drawn over exactly the cells covered, anchored to the
   content so it does not drift as new output arrives or the view scrolls.
2. **Given** an active selection, **When** the user right-clicks or presses Ctrl-C, **Then**
   the selected text is copied to the clipboard and the selection is cleared.
3. **Given** no active selection, **When** the user presses Ctrl-C, **Then** an interrupt
   (SIGINT) is sent to the running child as before (copy does not shadow interrupt).
4. **Given** the user drags past the top or bottom edge of the visible area, **When** the
   drag continues, **Then** the view auto-scrolls and the selection extends with it.
5. **Given** a full-screen application has requested mouse reporting, **When** the user
   clicks or scrolls, **Then** those events are forwarded to the application and kapollo's
   own selection/scroll is suspended until the application releases the mouse.
6. **Given** any state, **When** the user holds Shift while interacting with the mouse,
   **Then** kapollo passes the interaction through so the host terminal's native selection
   works as an escape hatch.
7. **Given** an active selection, **When** the user submits a command, **Then** the
   selection is cleared (matching Windows Terminal behavior), avoiding stale/over-run
   selections during subsequent output.

---

### User Story 3 - Block store over the grid (command + output + exit code) (Priority: P3)

A user's command history is captured as discrete **blocks** — each carrying the command
line, its output, and its exit code — anchored over the grid's scrollback, retained in an
in-memory store, and surfaced through existing affordances (`/save`, `/filter`, exit-code
chrome) plus new block-aware copy options.

**Why this priority**: The block model is a non-negotiable part of kapollo's vision (D8):
it powers `/save`, `/filter`, and the future AI layer. The rework must preserve it on the
new grid core rather than regress it, and this story re-homes it as an annotation layer
with a dedicated store. It builds on US1 (grid rows to anchor to) and complements US2
(block-aware selection), so it is sequenced third while remaining essential to the MVP.

**Independent Test**: Run several commands, then `/save` the last one and confirm the saved
text matches the command's output exactly (including styling-relevant content per the
configured fidelity). `/filter` the transcript and confirm block boundaries and exit codes
are honored. Right-click before selecting and confirm options to copy a block's output with
or without its command line.

**Acceptance Scenarios**:

1. **Given** a sequence of commands, **When** each finishes, **Then** kapollo records a
   block with the command line, the command's output, and the exit code, anchored to the
   grid rows the output occupies.
2. **Given** recorded blocks, **When** the user runs `/save` for a block, **Then** the saved
   content is the block's output as retained by the store (byte/text-faithful to the
   configured fidelity), not a lossy re-scrape.
3. **Given** recorded blocks, **When** the user runs `/filter`, **Then** filtering operates
   over block boundaries and exit codes exactly as in the prior MVP.
4. **Given** a command's exit code, **When** the block completes, **Then** the exit status is
   reflected in the transcript chrome (success/failure indication) as before.
5. **Given** the cursor is over a block, **When** the user right-clicks with no active
   selection, **Then** a menu offers copying the block's output **with** its command and
   **without** its command, plus the current line.
6. **Given** the in-memory store reaches its retention cap, **When** older blocks age out,
   **Then** eviction is bounded and deterministic, and callers that ask for evicted block
   text receive an unambiguous "unavailable" result rather than wrong content.

---

### Edge Cases

- **Alt-screen toggling under load**: a program rapidly enters/exits the alternate screen —
  the view must switch cleanly each time and never leak alt-screen content into main-screen
  scrollback or block annotations.
- **Selection during a flood**: output streams while a selection is held — the highlight
  stays anchored to its content and scrolls with it; on command submit the selection clears
  (US2-7), preventing over-run.
- **Resize during a selection or alt-screen app**: a terminal resize while text is selected
  or while a full-screen app is active must not corrupt the grid, the selection anchor, or
  block row ranges.
- **Output past the scrollback / store caps**: when retained rows or blocks exceed their
  caps, eviction is bounded; requests for content scrolled/evicted past the cap resolve to
  an explicit "unavailable", never to silently-wrong text.
- **Clipboard unavailable**: when the OSC 52 path is not honored by the host (or is disabled)
  and the local fallback is unavailable, copy fails gracefully with a user-visible notice
  rather than silently dropping the data.
- **Mouse events with no running child / at the prompt**: selection and scroll work over the
  prompt area without being misinterpreted as input to a non-existent child.
- **A single click without a drag**: a bare click must not create a stray one-cell selection;
  selection requires an actual drag.

## Requirements *(mandatory)*

### Functional Requirements

#### Rendering (US1)

- **FR-001**: System MUST model the shell's main screen as a terminal grid of styled cells
  with a cursor, applying control sequences (cursor movement, line/region clears, `\r`
  overwrites, scroll regions) so in-place redraws display their final state rather than an
  append-only history of writes.
- **FR-002**: System MUST render inline text styling — foreground/background color (including
  256-color and truecolor) and attributes (bold/intensity, italic, underline, reverse) —
  faithfully in the transcript (realizes D30, revising the earlier color-deferral).
- **FR-003**: System MUST handle wide (double-width) and combining/grapheme-cluster
  characters with correct width and placement so columns do not drift.
- **FR-004**: System MUST maintain a bounded **scrollback** of prior main-screen rows that
  the user can scroll through.
- **FR-005**: System MUST detect alternate-screen entry/exit and render the alternate screen
  faithfully while it is active, restoring prior main-screen content on exit, without mixing
  alt-screen content into scrollback or blocks.
- **FR-006**: System MUST keep the input-pad-at-bottom / transcript-above metaphor and the
  existing chrome (status, input pad) intact.

#### Mouse, selection & clipboard (US2)

- **FR-007**: Users MUST be able to select main-screen text by click-and-drag, with a
  visible highlight drawn over the covered cells.
- **FR-008**: Selection MUST be anchored to content (the underlying logical rows), so it
  does not drift as new output arrives or the user scrolls.
- **FR-009**: System MUST auto-scroll and extend the selection when the user drags past the
  top or bottom edge of the visible area.
- **FR-010**: A bare click without movement MUST NOT create a selection; selection requires
  an actual drag.
- **FR-011**: With an active selection, right-click or Ctrl-C MUST copy the selected text to
  the clipboard and then clear the selection.
- **FR-012**: With no active selection, Ctrl-C MUST send SIGINT to the running child (copy
  MUST NOT shadow interrupt).
- **FR-013**: System MUST copy to the clipboard via OSC 52 (terminal-mediated, works over
  SSH) with a local clipboard fallback, and MUST surface a visible notice when copying
  fails rather than dropping data silently.
- **FR-014**: System MUST support scroll-wheel and PageUp/PageDown navigation through
  scrollback; the selection MUST survive scrolling via its content anchor.
- **FR-015**: When a full-screen application requests mouse reporting (or the alternate
  screen is active), System MUST forward mouse and relevant input events to the application
  and suspend kapollo's own selection/scroll until the application releases the mouse.
- **FR-016**: Holding Shift during mouse interaction MUST pass the interaction through to the
  host terminal so its native selection works as an escape hatch.
- **FR-017**: Submitting a command MUST clear any active selection (matching Windows Terminal
  behavior) to prevent stale/over-run selections during subsequent output. (Configurable
  clear-on-submit behavior MAY follow later; the MVP fixes it to clear.)

#### Block model & store (US3)

- **FR-018**: System MUST record each executed command as a **block** carrying the command
  line, the command's output, and the exit code, with the block anchored to the grid rows
  its output occupies (block-as-annotation-over-grid, realizing D29's structure).
- **FR-019**: System MUST retain block content in an **in-memory block store** that is the
  canonical source for a block's text (superseding D29's reconstruct-from-grid lean for
  v1), exposed to callers through a single text accessor so the storage mechanism can change
  without reshaping callers.
- **FR-020**: The block store MUST be structured so that a persistent (database) backing can
  be added later as a secondary store without reshaping the accessor surface; for the MVP it
  is **in-memory only** (no persistence).
- **FR-021**: System MUST preserve `/save` such that saved content is the block's output from
  the store, faithful to the configured fidelity, not a lossy re-scrape.
- **FR-022**: System MUST preserve `/filter` operating over block boundaries and exit codes
  as in the prior MVP.
- **FR-023**: System MUST reflect each block's exit status in the transcript chrome
  (success/failure indication) as before.
- **FR-024**: System MUST offer block-aware copy affordances: copy a block's output **with**
  its command line, **without** its command line, and copy the current line. (An advanced
  multi-block range-select copy MAY follow later and is out of scope for the MVP.)
- **FR-025**: The block store MUST enforce a bounded retention cap with deterministic
  eviction; requests for the text of an evicted (or past-cap) block MUST return an explicit
  "unavailable" result, never silently-wrong content.
- **FR-029**: System MUST capture each block's start and end wall-clock timestamps (from the
  shell-integration command-start/command-end signals, with the sentinel fallback) and
  expose the elapsed duration, so timing is available for chrome, `/filter`, and the future
  AI layer; timestamps MUST be stored in a form that survives a future persistent backing.

#### Cross-cutting integrity

- **FR-026**: System MUST extend configuration to cover the new surface — mouse enable,
  selection/copy behavior, clipboard path (OSC 52 / local / fallback order), and
  scroll/scrollback settings — keeping existing config keys working.
- **FR-027**: System MUST preserve TUI integrity throughout: application logs are written
  off-screen (never into the transcript), a panic boundary restores the terminal, and
  teardown leaves the host terminal in a clean state (raw mode, mouse capture, and alternate
  screen all released) on every exit path.
- **FR-028**: The rework MUST be an in-place replacement of the rendering/transcript/block
  core (Path A) that keeps the stable layers (PTY, config, slash registry, input router,
  chrome, shell hooks) functioning, with no regression to existing slash commands or the
  shell-wrapping behavior.

### Key Entities *(include if feature involves data)*

- **Grid (Screen)**: The emulated main screen — rows × columns of styled cells with a
  cursor — plus a bounded scrollback of prior rows. The canonical render/selection surface.
- **Cell**: A single grid position: glyph (grapheme cluster), width, and style (color +
  attributes).
- **Scrollback**: The bounded, growing sequence of prior main-screen rows, addressable by a
  content-stable row anchor so selections and block ranges survive scrolling and (within the
  cap) eviction.
- **Block**: An annotation over the grid: `{ command, output, exit_code, start_row,
  end_row, started_at/ended_at (wall-clock) → duration }`. The source of truth for `/save`,
  `/filter`, exit-code chrome, and the future AI layer.
- **Block Store**: The in-memory retainer of block content, canonical source for block text
  via a single accessor, bounded with deterministic eviction, and structured so a database
  backing can be added as a secondary store later.
- **Selection**: A content-anchored range over grid rows/columns, with states (idle →
  dragging → active) driving the highlight and copy.
- **Clipboard Target**: The destination for copied text — OSC 52 (primary) with a local
  fallback and a defined fallback order.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A progress bar or `\r`-overwriting counter renders as a **single updating
  line** with zero extra scrollback lines per update (0 spurious lines), matching the host
  terminal.
- **SC-002**: For a representative set of colorized/attributed outputs, **100%** of color and
  attribute states render correctly (no dropped or wrong colors) versus the host terminal.
- **SC-003**: Full-screen applications (`vim`, `htop`, `bpytop`) are **fully usable** — they
  render correctly, receive mouse/keyboard input when they request it, and the prior screen
  restores cleanly on exit — in **100%** of launch/exit cycles tested.
- **SC-004**: Click-drag selection copies **exactly** the visually selected text (character-
  for-character) in **100%** of trials, including while output streams and after scrolling.
- **SC-005**: Copied selections reach the clipboard via OSC 52 (or the configured fallback)
  and paste back identically; when no clipboard path is available the user sees a failure
  notice in **100%** of such cases (no silent drops).
- **SC-006**: A selection held during a sustained output flood stays anchored to its content
  (does not drift onto unrelated text) and is cleared on command submit in **100%** of
  trials.
- **SC-007**: `/save` of a completed block reproduces that block's output faithfully (to the
  configured fidelity) in **100%** of trials for non-evicted blocks; evicted blocks report
  "unavailable" rather than wrong content.
- **SC-008**: `/filter` and exit-code chrome behave identically to the prior MVP for an
  equivalent command set (no functional regression).
- **SC-009**: On every exit path tested (normal quit, Ctrl-Q, panic), the host terminal is
  restored to a clean state (no leftover raw mode, mouse capture, or alternate screen) in
  **100%** of cases.
- **SC-010**: Adding a future database backing to the block store requires **no change** to
  block-text caller sites (the single accessor remains the only touch point), validated by
  the accessor abstraction being the sole text-retrieval entry point.

## Assumptions

- **Engine**: The production terminal-emulation engine is the one selected by the 003 spike —
  `wezterm-term`, git-pinned, with `alacritty_terminal` as the named fallback (D27,
  [recommendation.md](../../delos/docs/recommendation.md)). Requirements above are written in
  capability terms; the engine supplies the grid, scrollback, stable row anchoring, grapheme
  segmentation, and OSC 8 awareness.
- **Block fidelity**: The block store retains each command's output as the canonical block
  text (option (b) from [02-rework-vs-rewrite.md](../planning/grid-pivot/02-rework-vs-rewrite.md)
  §4), making `/save` faithful. This **supersedes D29's** v1 "reconstruct from grid rows"
  lean; the single text accessor (`block.text()` / `block.text_with_command()`) is retained
  regardless of backing.
- **Persistence is out of scope for this MVP**: the store is in-memory only. The structure
  anticipates a later database backing and a privacy-configurable, toggleable persistence
  layer (queued async write off the in-memory store), but none of that ships here.
- **Path A (rework in place)**: the stable layers (PTY, config, slash, input router, chrome,
  shell hooks) carry over; only the output→grid, transcript render, block model, and event
  loop are reworked (D-Path, doc 02 §2–§3).
- **Selection semantics**: terminal-style drag selection plus the right-click pre-selection
  copy menu (block output with/without command, current line). The "advanced range select"
  across multiple blocks is deferred beyond the MVP (brainstorm 6.2).
- **Clipboard**: OSC 52 primary with a local fallback; the exact fallback order is
  configurable. The spike validated OSC 52 framing and a local fallback.
- **Images / OSC 8 hyperlinks**: image rendering is out of scope (model-level only, the
  spike's "cherry"); OSC 8 hyperlink data may be retained at the model level but interactive
  hyperlink UX is not required for this MVP.
- **Platform**: Linux-first (D9); the chosen engine keeps the cross-platform door open but
  cross-platform is not a goal of this MVP.
- **Constitution**: This honors the constitution (TUI integrity VI especially); D25–D30 are
  the superseding decisions this feature realizes.

## Dependencies

- The 003 grid spike's crate recommendation and the promoted `spike-support` helpers
  (coordinate/mode/clipboard math) and `selection.rs` state machine as the design seed.
- Existing kapollo modules retained per the reuse map (PTY, config, slash, input router,
  chrome, shell OSC 133/7 hooks).
- Grid-pivot decisions **D25–D30** ([02-rework-vs-rewrite.md](../planning/grid-pivot/02-rework-vs-rewrite.md) §6).
