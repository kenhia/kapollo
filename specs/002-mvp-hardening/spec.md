# Feature Specification: kapollo MVP Hardening — Render, Chrome, Passthrough & Performance

**Feature Branch**: `002-mvp-hardening`  
**Created**: 2026-05-30  
**Status**: Draft  
**Input**: First user test ([.scratch/kapollo-mvp-usertest.md](../../.scratch/kapollo-mvp-usertest.md)), Brainstorm decisions ([specs/planning/brainstorm.md](../planning/brainstorm.md), especially D22/D23/D24 plus D3/D4/D9/D12/D14/D16/D17/D19), and the prior MVP spec ([specs/001-mvp-repl/spec.md](../001-mvp-repl/spec.md)). This is a consolidation/hardening sprint to close bugs and Definition-of-Done gaps before Tier 1 work begins. Linux-only (D9); no terminal grid model (D4/§6); file-only logging.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Read output that renders cleanly and correctly (Priority: P1)

A user runs ordinary commands (`ls`, `echo $SHELL`, `cat file`) and reads the
results in the transcript pad. Every captured block renders as clean,
printable text: no leaked control bytes, no stray characters bleeding across
rows when the pad scrolls, and no output overwriting chrome. The transcript
surface is fully owned by the renderer, so each frame is drawn correctly
regardless of what the wrapped program emitted (bare carriage returns, OSC
color-query responses, residual escape sequences).

**Why this priority**: The transcript is kapollo's core surface. The first
user test showed output overwriting the frame, only-first-line corruption,
and stray `L` characters appearing on scroll. These make kapollo unusable for
its primary purpose, so correct rendering is the highest-priority fix.

**Independent Test**: Launch `kap`, run `ls`, then repeatedly run
`echo $SHELL` until the transcript scrolls past a full screen; confirm no
characters overwrite the input/status chrome, no stray characters (e.g. a
trailing `L`) appear on any row after scroll, and every line of every block is
intact. Run `cat` on a file containing tabs and multiple lines and confirm the
first line is not corrupted.

**Acceptance Scenarios**:

1. **Given** kapollo is running, **When** a command produces multi-line
   output, **Then** every line of the block renders as clean printable text
   and none of it overwrites the input pad or status chrome.
2. **Given** a wrapped program emits a bare carriage return, an OSC
   color-query response (e.g. `]11;rgb:2020/2020/2020`), or other residual
   escape sequences, **When** that output is captured into a block, **Then**
   those bytes are normalized/stripped and do not appear as visible
   characters in the rendered block.
3. **Given** the transcript has filled the pad, **When** new output causes the
   transcript to scroll, **Then** no stray characters from a prior frame
   remain on any cell (the renderer clears/owns the whole surface each frame).
4. **Given** a block whose first line previously rendered incorrectly,
   **When** the same command is run again, **Then** the first line renders
   identically and correctly to subsequent lines.

---

### User Story 2 - See a clean, informative chrome (Priority: P1)

A user sees a minimal, modern chrome: no box/border around the transcript, a
single horizontal rule above the input area that carries the current working
directory and (only when relevant) a non-zero exit code, a blank line between
output blocks for readability, and a colorized `λ` prompt character echoed
before each command instead of `$`.

**Why this priority**: The chrome redesign both directly addresses user-test
feedback and shrinks the surface where render corruption (US1) can occur.
Removing the transcript frame eliminates an entire class of "output overwrote
the border" bugs.

**Independent Test**: Launch `kap` and confirm the transcript has no
surrounding box; confirm a single horizontal rule sits above the input area
showing the cwd; run a command that exits 0 and confirm no exit code is shown;
run a command that exits non-zero (e.g. `false`) and confirm the non-zero exit
code is shown on the rule. Run two commands and confirm a blank line separates
the two output blocks, and that each command is echoed with a colorized `λ`
prefix.

**Acceptance Scenarios**:

1. **Given** the split-pad UI is active, **When** it is rendered, **Then**
   there is no box/border drawn around the transcript (output) pad.
2. **Given** the input area, **When** it is rendered, **Then** a single
   horizontal rule line sits above it (replacing the prior box and "input"
   label) and carries the current working directory.
3. **Given** the last command exited with code 0, **When** the status is
   rendered, **Then** no exit code is shown; **Given** the last command exited
   non-zero, **Then** that exit code is shown on the rule.
4. **Given** two or more commands have run, **When** their blocks are
   rendered, **Then** a blank line separates each output block from the next.
5. **Given** a command is submitted, **When** it is echoed into the
   transcript, **Then** it is prefixed with the prompt character `λ`
   (configurable) rather than `$`, and that character is colorized (red by
   default, configurable) when color is enabled.

---

### User Story 3 - Run interactive programs without corruption or residue (Priority: P1)

A user runs full-screen/alt-screen programs (`vi`, `bpytop`) from the input
pad. The program receives only the user's keystrokes — no spurious control
characters (such as an OSC color-query response) are injected into it. When the
program exits, the terminal and split-pad UI are restored cleanly every time.

**Why this priority**: The first user test showed `vi` receiving a stray
`]11;rgb:...` response as input and the terminal not being restored after `vi`
or `bpytop` exited. Both make interactive tools unusable, blocking daily-driver
adoption (D3 passthrough; §7 clean enter/exit).

**Independent Test**: Run `vi test.txt`, confirm no spurious characters appear
in the buffer or on the command line, edit and `:q`, and confirm the split-pad
UI is restored intact. Repeat with `bpytop`; on exit confirm the terminal is
fully restored (cursor visible, normal mode, no leftover alt-screen, no
corrupted state) every time.

**Acceptance Scenarios**:

1. **Given** kapollo launches an alt-screen program, **When** the program
   starts, **Then** no spurious control characters (e.g. an OSC color-query
   response) are delivered to the program as input.
2. **Given** an alt-screen program (`vi`, `bpytop`) is running in passthrough,
   **When** the program exits, **Then** the terminal is restored to a clean
   state and the split-pad UI returns with the prior transcript intact.
3. **Given** repeated entry into and exit from alt-screen programs, **When**
   each program exits, **Then** the terminal is restored cleanly on 100% of
   exits with no accumulated corruption.

---

### User Story 4 - Stay responsive and interruptible under huge output (Priority: P1)

A user runs a command that floods the transcript with millions of lines (e.g.
`yes | head -n 5000000`). kapollo processes it in roughly the same wall-clock
time the bare shell would take, stays responsive, and Ctrl-C interrupts the
flood promptly.

**Why this priority**: The user test showed this flood pinning one core at
~100% for minutes while a bare terminal finished in ~2s, with the UI frozen and
Ctrl-C ineffective. Two root causes are already diagnosed (O(n²) cap
enforcement in the ring buffer and event-loop starvation). This is
self-contained, high-value, and required for trustworthy operation.

**Independent Test**: Run `yes | head -n 5000000`; confirm it completes in
roughly shell-native time (not minutes), the UI stays responsive throughout,
and pressing Ctrl-C during the flood interrupts it promptly.

**Acceptance Scenarios**:

1. **Given** a command floods output far past the per-block cap, **When**
   kapollo enforces the cap, **Then** enforcement cost is amortized roughly
   O(1) per byte (incremental line counting and bulk trim), not a full
   rescan-per-chunk.
2. **Given** a heavy output stream is arriving, **When** the event loop runs,
   **Then** per-iteration drain work is bounded so key input is serviced
   promptly and the UI does not freeze.
3. **Given** a flood is in progress, **When** the user presses Ctrl-C, **Then**
   the running command is interrupted promptly (within a small, bounded delay).
4. **Given** a block is already known to be truncated to its tail, **When**
   further bytes for that block arrive, **Then** kapollo may cheaply discard
   them rather than buffering and re-trimming.

---

### User Story 5 - Use color, accurate cwd, scrolling, and `/exit` (Priority: P2)

A user benefits from the smaller Definition-of-Done fixes: chrome color works
(and `NO_COLOR` disables it), the status cwd follows `cd`, `/exit` works as an
alias for `/quit`, and the transcript scrolls with the keyboard
(PgUp/PgDn/Home/End), with all scrolling keys documented in `/help`.

**Why this priority**: These are individually small but collectively close
stated DoD gaps and visible papercuts from the user test. They are P2 because
the core run loop is usable without them once US1–US4 land.

**Independent Test**: With color enabled, confirm the `λ` prompt is red;
relaunch with `NO_COLOR=1` and confirm color is suppressed. Run `cd /tmp` then
`pwd` and confirm the status cwd updates to `/tmp`. Type `/exit` and confirm
kapollo quits. Fill the transcript, press PgUp/PgDn to scroll and Home/End to
jump to top/bottom, and confirm `/help` lists these keys.

**Acceptance Scenarios**:

1. **Given** color is enabled (no `NO_COLOR`), **When** chrome is rendered,
   **Then** the `λ` prompt (and other chrome color per D22) is colorized;
   **Given** `NO_COLOR` is set, **Then** all chrome color is suppressed.
2. **Given** the user runs `cd /tmp` in a hook-supported shell, **When** the
   prompt returns, **Then** the status line cwd updates to `/tmp` (via OSC 7).
3. **Given** kapollo is running, **When** the user types `/exit`, **Then**
   kapollo exits exactly as `/quit` does.
4. **Given** a transcript taller than the pad, **When** the user presses
   PgUp/PgDn, **Then** the transcript scrolls up/down; **When** the user
   presses Home/End, **Then** it jumps to the top/bottom.
5. **Given** the user runs `/help`, **When** help is shown, **Then** it lists
   the transcript scrolling key bindings (PgUp/PgDn, Home/End).

---

### Edge Cases

- **Sentinel-fallback shells**: A shell that does not emit OSC 7 (no hook)
  gets no live cwd update; the status cwd MAY remain at its last known value.
  This is acceptable and consistent with the degraded support for such shells
  (D23).
- **OSC responses mid-block**: A terminal may emit OSC color-query *responses*
  at unpredictable times; normalization MUST strip them from captured block
  text wherever they appear, not only at block start.
- **Truncated-block flood**: Once a block is truncated to its tail, continued
  flooding MUST NOT degrade responsiveness or re-introduce O(n²) cost.
- **Scroll while output streams**: When the user has scrolled up and new output
  arrives, scrolling behavior MUST remain coherent (no stray-character residue;
  US1 invariants hold).
- **Color disabled**: With `NO_COLOR`, the `λ` prompt still renders as the
  configured character but without color.
- **Resize with no border**: With the transcript frame removed, resize MUST
  still reflow cleanly and the renderer MUST own the full surface.

## Requirements *(mandatory)*

### Functional Requirements

**A. Render-pipeline correctness**

- **FR-001**: kapollo MUST normalize captured block output to clean printable
  text before rendering, stripping bare carriage-return artifacts, OSC
  responses (including color-query responses such as `]11;rgb:…`), and residual
  escape sequences so they never appear as visible characters in a block
  (D4/§6: styling is stripped, not rendered, this sprint).
- **FR-002**: The transcript renderer MUST fully own and clear its drawing
  surface on each frame so that no stray characters from a prior frame remain
  on any cell after scrolling or redraw.
- **FR-003**: Block output MUST NOT overwrite the input pad or status chrome;
  output rendering MUST be confined to the transcript pad's area.
- **FR-004**: The first line of every block MUST render with the same
  correctness as all subsequent lines (no first-line-only corruption).

**B. Chrome redesign**

- **FR-005**: kapollo MUST NOT draw a box/border around the transcript (output)
  pad.
- **FR-006**: kapollo MUST render a single horizontal rule line above the input
  area (replacing the prior input box and "input" label) that carries status
  information.
- **FR-007**: The status rule MUST show the current working directory.
- **FR-008**: The status rule MUST show the last command's exit code ONLY when
  it is non-zero; an exit code of 0 MUST NOT be shown.
- **FR-009**: kapollo MUST render a blank line between consecutive output
  blocks in the transcript.
- **FR-010**: kapollo MUST echo each submitted command prefixed with a
  configurable prompt character defaulting to `λ` (replacing `$`).
- **FR-011**: kapollo MUST colorize the prompt character (default red,
  configurable) as chrome color when color is enabled (D22).

**C. Passthrough robustness**

- **FR-012**: kapollo MUST NOT inject spurious control characters (e.g. OSC
  color-query responses) into an alt-screen program as input.
- **FR-013**: kapollo MUST restore the terminal to a clean state every time an
  alt-screen program (`vi`, `bpytop`, etc.) exits passthrough (cursor visible,
  normal mode, no leftover alt-screen, prior transcript intact) (D3; §7).

**D. Performance / concurrency**

- **FR-014**: Output ring-buffer cap enforcement
  ([src/session/ringbuf.rs](../../src/session/ringbuf.rs)) MUST be amortized
  roughly O(1) per byte: line counts MUST be tracked incrementally (no full
  rescan per push) and over-cap data MUST be trimmed in bulk (no byte-at-a-time
  popping) (D14).
- **FR-015**: The event loop ([src/app.rs](../../src/app.rs)) MUST bound the
  work performed per drain pass (a chunk count and/or byte budget) and/or
  service a pending interrupt promptly, so key input (including Ctrl-C) is not
  starved during heavy output.
- **FR-016**: kapollo MAY discard captured bytes for a block once that block is
  known to be truncated to its tail, to avoid buffering and re-trimming flood
  output.
- **FR-017**: Ctrl-C MUST interrupt a running command promptly (within a small,
  bounded delay) even while the command is flooding output.

**E. Quick wins / DoD gaps**

- **FR-018**: Chrome color MUST function when enabled and MUST be suppressed
  when `NO_COLOR` is set in the environment (D22; honoring the existing
  NO_COLOR decision).
- **FR-019**: The status cwd MUST follow `cd`, updated from the shell emitting
  OSC 7 (`ESC]7;file://host/abs/path ST`) from the same prompt hook that emits
  OSC 133, parsed in the existing vte layer; kapollo MUST NOT scrape cwd from
  rendered prompt text (D23/D19/D12).
- **FR-020**: kapollo MUST provide `/exit` as an alias for `/quit`.
- **FR-021**: kapollo MUST support keyboard-only transcript scrolling: PgUp and
  PgDn to scroll, Home and End to jump to top and bottom of the transcript
  (D24).
- **FR-022**: `/help` output MUST list the transcript scrolling key bindings
  (PgUp/PgDn, Home/End).
- **FR-023**: New configurable values (prompt character, prompt color) MUST be
  read from the existing config (`~/.config/kapollo/config.toml`) with sensible
  defaults (`λ`, red) applied when absent (D15).

### Key Entities *(include if feature involves data)*

- **Block**: Unchanged in shape from the MVP (command + captured output +
  exit code), but its captured output is now **normalized** to clean printable
  text (FR-001) and its enforcement metadata tracks line count incrementally
  and a "truncated" flag for cheap flood discard (FR-014/FR-016).
- **Status / Chrome state**: The data rendered on the single status rule —
  current working directory (sourced from OSC 7; FR-019) and the last exit code
  (shown only when non-zero; FR-008).
- **Configuration**: Extended with a prompt character and prompt color, read
  from the existing config file with defaults (`λ`, red) (FR-023).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Across a session that scrolls the transcript past multiple full
  screens, 0 stray characters from prior frames remain and 0 bytes of block
  output overwrite the input/status chrome (US1; FR-001–FR-004).
- **SC-002**: OSC color-query responses, bare carriage returns, and residual
  escape sequences appear as visible characters in rendered blocks 0% of the
  time across the validation command set (FR-001).
- **SC-003**: The transcript pad has no surrounding border; the status rule
  shows cwd always and exit code only when non-zero; a blank line separates
  every pair of adjacent blocks; the prompt is a colorized `λ` — verified in
  100% of validation runs (US2; FR-005–FR-011).
- **SC-004**: Launching and exiting `vi` and `bpytop` restores the terminal to
  a clean state and injects 0 spurious control characters into the program, in
  100% of attempts (US3; FR-012/FR-013).
- **SC-005**: `yes | head -n 5000000` completes in roughly shell-native
  wall-clock time (same order of magnitude as the bare shell, not minutes),
  with the UI staying responsive throughout (US4; FR-014/FR-015).
- **SC-006**: During a 5M-line flood, Ctrl-C interrupts the command promptly
  (within a small bounded delay, not requiring the flood to finish) in 100% of
  attempts (US4; FR-017).
- **SC-007**: Chrome color is visible when enabled and fully suppressed under
  `NO_COLOR` in 100% of runs (FR-018).
- **SC-008**: After `cd /tmp` in a hook-supported shell, the status cwd shows
  `/tmp` on the next prompt in 100% of attempts (FR-019).
- **SC-009**: `/exit` exits kapollo identically to `/quit`; keyboard scrolling
  (PgUp/PgDn/Home/End) works and is listed in `/help`, in 100% of attempts
  (FR-020–FR-022).

## Out of Scope *(explicit non-goals)*

- **Block ANSI color passthrough**: Preserving/rendering the wrapped program's
  own SGR colors inside transcript blocks is deferred to Tier 2. This sprint
  still *strips* program styling (D22; D4/§6).
- **Mouse-wheel scrolling / mouse capture**: Deferred to a later opt-in config
  (`mouse = true`, default off) because capturing the mouse breaks the host
  terminal's native selection/copy (D24).
- **Tier 1 features**: Slots/macros, command palette, richer config surface,
  `/cd`, and `/history` are out of scope this sprint.
- **macOS / Windows**: Linux-only; cross-platform parity remains deferred (D9).

## Assumptions

- **Platform**: Linux only (D9).
- **No grid model**: kapollo continues to not implement a terminal cell grid;
  ANSI parsing is used only for alt-screen detection and style stripping plus
  OSC 7/133 handling (D4/§6).
- **Shells**: fish and bash are the validated hook-supported shells; OSC 7 cwd
  tracking applies to them. Sentinel-fallback shells get no live cwd (D17/D23).
- **Logging**: File-only logging is unchanged; nothing in this sprint writes to
  the TUI surface.
- **TDD**: Tests are authored first per the project's TDD mandate; render,
  ring-buffer, passthrough, and parsing changes are covered by tests (extending
  the existing `tests/` suite).
- **Caps**: Existing per-block / whole-transcript cap defaults (D14) are
  retained; only the enforcement algorithm changes (performance), not the
  caps' semantics.
- **Color defaults**: When color is enabled and unconfigured, the prompt `λ`
  is red; when `NO_COLOR` is set, color is suppressed (D22).
