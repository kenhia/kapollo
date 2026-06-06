# Pre-plan 007 — LAAT mode, `/save` & `/filter`

> Source: `.scratch/005-pre-planning.md`. A new modal input mode and the first
> exercise of kapollo's "modes" concept, grouped with the `/save` and `/filter`
> slash commands (different code, but one mental model: working with the output
> of prior commands).

## Goal

Add a **Line-At-A-Time** mode for running a sequence of commands one at a time —
paste or type several commands, then step through them, running each and
checking its output before advancing. This is also the testbed for the broader
"modes" idea.

Mode naming: full name **`LaaT`** (4 chars, mixed case), short form **`1T`**
(2 chars) for the status mode field reserved in sprint 005.

## In scope

### Slash commands `/save` and `/filter`

- **`/save <file>`** — write the previous block's exact stored output (from the
  block store, R3) to `<file>`. This is the original deferral from sprint 004
  (kwi WI #43); the store/seam already exists.
- **`/filter <cmd>`** — equivalent to piping the *previous command's* output
  through `<cmd>`. E.g. `ps -aux` then `/filter rg postgres` behaves like
  `ps -aux | rg postgres` (conceptually: save previous output to a temp,
  `cat <temp> | <cmd>`). Original deferral (kwi WI #44).

### LAAT (Line-At-A-Time) mode

### Entering / populating

- Enter LAAT mode via a binding (TBD — see Q1). Either paste-then-enter or
  enter-then-paste; also support typing multiple lines directly.
- The pasted/typed lines populate a **LAAT buffer**, one command per line.
- **Load a script into the LAAT buffer** (tie-in with the push/pop stack below).

### Stepping & execution

- The first line is highlighted. Arrow keys move the highlight between lines;
  `Shift+arrow` selects multiple lines.
- `Enter` submits the highlighted line(s).
- **Wait-for-completion gating** (decision below): after a submitted line
  finishes, if its **exit code is 0**, advance the highlight to the next line;
  if **non-zero**, stay on the current line and change its highlight background
  to flag "this command probably failed."
- The user explicitly leaves LAAT mode, which clears the LAAT buffer.

### Push/pop input stack (`Ctrl+Alt+Enter`, NOTE_2)

- "Push" the current input buffer **and mode**, switch to normal mode until the
  next submit, then "pop" — restoring the prior buffer and mode.
- A one-item push/pop stack — a new primitive that LAAT (and future modes) build
  on; lets the user duck out to run an ad-hoc command mid-sequence.

### Multi-line input as its own mode (`Mult`) — from 005 smoke test

> Observation during the sprint 005 walkthrough (2026-06-06): the multi-line
> buffer needs its own mode so arrow keys behave intuitively. Today, in a
> multi-line buffer, `Up`/`Down` recall input history instead of moving the
> caret between lines — so editing a typo on an earlier line throws the buffer
> away. This is the second concrete "mode" (alongside LAAT) and validates the
> modes concept.

- **Entering `Mult` mode.** Start typing a line, then `Alt+Enter` adds a line
  **and** switches into `Mult` mode. (Mode label reserved in 005's 4-char field,
  e.g. `Mult`.) Plain `Enter` still submits the whole buffer.
- **Arrows move the caret between lines while in `Mult`.** `Up`/`Down` move the
  caret up/down a visual line within the buffer rather than recalling history.
  - Use case: user types a line, `Alt+Enter` into `Mult`, types a second line,
    spots a typo on the first line, presses `Up` → **caret moves up a line**.
    - Expected: caret moves up a line.
    - Current (005): the input buffer is thrown away and the previous history
      entry is recalled.
- **History recall at the edges (the "chat-style" behavior).** Same model many
  chat inputs (including this one) use:
  - `Up` while the caret is **already on the first line** stashes the current
    buffer **temporarily** and recalls the previous history entry.
  - `Down` from the recalled entry (when back at the top) **restores** the
    stashed temporary buffer.
  - Symmetric at the bottom edge for `Down`.
- **Relation to history's existing draft cursor.** `InputHistory` already tracks
  a recall cursor (`None` = live draft); extend it to also hold the **stashed
  draft buffer** so the temporary content can be restored on `Down` (today
  `recall_*` returns only the stored entry text, and 005's `on_key` replaces the
  whole buffer via `set_contents`).

## Decisions (resolved in pre-planning)

- **Execution gates on completion.** Submitting a line waits for the command to
  finish. Exit 0 → advance; non-zero → stay put and highlight as a probable
  failure. "Probable" is deliberate — some commands (e.g. Windows `robocopy`)
  use non-zero success codes.
- **Mode label** is `LaaT` / `1T`.

## Out of scope

- Per-mode keymap configuration (depends on 006 if pursued).
- Persisting/saving LAAT buffers between sessions.

## Open questions

- **Q1 — Entry binding.** Which key (or `/`-command) toggles LAAT? Interaction
  with the keymap engine if 006 ships first.
- **Q2 — Failure recovery UX.** On a non-zero exit, what are the options — edit
  the line in place and re-run, skip, abort the whole buffer? Does the user
  override the "probably failed" and advance manually?
- **Q3 — Multi-line submit.** When `Shift+arrow` selects several lines and the
  user hits `Enter`, are they sent as one combined submission or run
  sequentially with gating between each?
- **Q4 — "Load a script" source.** File path via a command/arg? Relation to a
  future `/save`'d block? Define the load surface.
- **Q5 — Output association.** How is each line's output/exit shown against the
  buffer (inline, in the transcript, both)?
- **Q6 — `/save` target semantics.** Path resolution (relative to cwd?), default
  filename if omitted, overwrite vs. append/confirm, and behavior when the
  previous block is unavailable/evicted (explicit notice, per FR-025).
- **Q7 — `/filter` execution model.** Run `<cmd>` via the shell (so pipes/globs
  work) or exec directly? Does `/filter` create a new block in the transcript,
  and does its own output become the new "previous output" for a chained
  `/filter`? How are non-zero filter exits surfaced?
- **Q8 — `Mult` mode entry/exit.** Does `Alt+Enter` from a single line always
  enter `Mult`, or only when the buffer becomes multi-line? How does the user
  leave `Mult` — on submit (`Enter`), on `Esc Esc` clearing the buffer (005
  FR-029), or an explicit toggle? Does deleting back to one line auto-exit?
- **Q9 — `Mult` vs. LAAT overlap.** Both are multi-line, arrow-navigated modes.
  Is `Mult` a distinct mode or just LAAT-without-gating? Clarify which keys
  differ (LAAT: highlight + step + exit-code gating; `Mult`: free caret editing,
  single combined submit).
- **Q10 — Stashed-draft scope.** Is the temporary stashed buffer kept only while
  recalling history (cleared on submit), and does it survive a mode switch
  (`Mult` ↔ normal, push/pop stack)?

## Dependencies / sequencing

- **Depends on 005** for the multi-line input buffer and the reserved mode field
  in the status bar.
- **Benefits from 006** if LAAT wants configurable, per-mode bindings (otherwise
  its keys are hardcoded like 005's).
