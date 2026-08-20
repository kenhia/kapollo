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
- **`/load <file>`** — read a script file's lines into the LAAT buffer (one
  command per line) and enter LAAT mode (see Q4).

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
- **Keymap integration (006 shipped).** All new keys (the `Ctrl-Alt-1` mode
  toggle, the `Ctrl+Alt+Enter` push/pop) and the new slash commands are
  registered as **named actions** in the keymap engine — rebindable and listed by
  `/keys`, with `Ctrl-Alt-1` etc. as the *defaults*. Per-mode config sections
  (`[keymap.laat]`/`[keymap.mult]`) remain out of scope this sprint.
- **Entering a mode from norm.** `Ctrl-Alt-1` in **norm** (even on a single or
  empty line) enters **Mult**; once multi-line, `Ctrl-Alt-1` toggles
  **Mult ↔ LAAT**. `Alt+Enter` (a 2nd line) and `/load <file>` are the other
  entry paths (the latter lands directly in LAAT).
- **`/filter` runs via the shell** so pipes/globs/aliases work; its result is a
  new transcript block whose output **becomes the new “previous output,”** so
  `/filter` chains.
- **`/save` and `/load` resolve paths relative to kapollo’s current cwd**
  (which follows `cd`), with `~` tilde expansion.

## Out of scope

- Per-mode keymap **config sections** (`[keymap.laat]` / `[keymap.mult]`) — the
  new bindings are named, rebindable actions (see the keymap-integration decision
  below), but mode-scoped config tables wait for a later sprint.
- Persisting/saving LAAT buffers between sessions.

## Open questions

- **Q1 — Entry binding.** Which key (or `/`-command) toggles LAAT? Interaction
  with the keymap engine if 006 ships first.
    - slash commands (currently) don't coexist with content in the input buffer,
      I'm thinking for this sprint we add a keybinding (default: `Ctrl-Alt-1`)
      to toggle LAAT <--> Mult
    - **Resolved:** `Ctrl-Alt-1` in **norm** enters **Mult** (even single/empty
      line); once multi-line it toggles **Mult ↔ LAAT**. `/load` also enters LAAT
      directly. The toggle is a **named, rebindable action** (keymap engine, 006).
- **Q2 — Failure recovery UX.** On a non-zero exit, what are the options — edit
  the line in place and re-run, skip, abort the whole buffer? Does the user
  override the "probably failed" and advance manually?
    - User can:
        - rerun with `Enter` (failure not related to command now fixed)
        - determine command was okay (non-zero success code), `DownArrow` `Enter`
        - abort with `Esc` `Esc`
        - Push LAAT input buffer, fix issue, Pop input buffer, arrows to select, then continue
- **Q3 — Multi-line submit.** When `Shift+arrow` selects several lines and the
  user hits `Enter`, are they sent as one combined submission or run
  sequentially with gating between each?
    - I think we keep selection and submission separate (in all modes) currently
      in a multiline (still currently norm mode, but mult soon) if I select
      everything it submits; lets keep this behavior in norm/mult/laat
- **Q4 — "Load a script" source.** File path via a command/arg? Relation to a
  future `/save`'d block? Define the load surface.
    - yes, file path via command arg, e.g. `/load ~/my_install_script`
    - **Resolved:** path resolves relative to kapollo's current cwd (follows
      `cd`), with `~` tilde expansion; `/load` lands the file's lines in the LAAT
      buffer (one command per line) and enters LAAT.
- **Q5 — Output association.** How is each line's output/exit shown against the
  buffer (inline, in the transcript, both)?
    - output just as if this was a single line, norm mode command submission
    - exit code displayed in status for the command that was submitted (last
      exit code is what we display now, same behavior)
- **Q6 — `/save` target semantics.** Path resolution (relative to cwd?), default
  filename if omitted, overwrite vs. append/confirm, and behavior when the
  previous block is unavailable/evicted (explicit notice, per FR-025).
    - `/save` without target gives a status bar message "'/save' requires path",
      the input buffer is not cleared allowing user to either `Esc` or provide
      path arg and resubmit
    - prompt if exists, "File exists, [O]verwrite, [A]ppend, [C]ancel?"
    - status message error "Save failed, previous buffer not found"
    - **Resolved:** path resolves relative to kapollo's current cwd (follows
      `cd`), with `~` tilde expansion.
- **Q7 — `/filter` execution model.** Run `<cmd>` via the shell (so pipes/globs
  work) or exec directly? Does `/filter` create a new block in the transcript,
  and does its own output become the new "previous output" for a chained
  `/filter`? How are non-zero filter exits surfaced?
    - new block in transcript, the command will be "/filter ...rest of command"
    - on non-zero exit, we update the exit code in the status bar AND add status
      message "filter non-zero exit" (often okay, if I `rg` and not found, I get
      a non-zero)
    - **Resolved:** runs via the **shell** (pipes/globs/aliases work); the result
      block **becomes the new “previous output,”** so `/filter` chains.
- **Q8 — `Mult` mode entry/exit.** Does `Alt+Enter` from a single line always
  enter `Mult`, or only when the buffer becomes multi-line? How does the user
  leave `Mult` — on submit (`Enter`), on `Esc Esc` clearing the buffer (005
  FR-029), or an explicit toggle? Does deleting back to one line auto-exit?
    - `Alt+Enter` (adding a second line to input buffer) places us into `mult`
      (user can optionally toggle into laat once in mult). If user removes
      additional lines so buffer is back to one line, we transition back to
      norm.
    - User can leave mult by `Esc Esc`, command submit, or pushing existing
      command
- **Q9 — `Mult` vs. LAAT overlap.** Both are multi-line, arrow-navigated modes.
  Is `Mult` a distinct mode or just LAAT-without-gating? Clarify which keys
  differ (LAAT: highlight + step + exit-code gating; `Mult`: free caret editing,
  single combined submit).
    - keys are the same for laat and mult, I think of LaaT as "Mult + highlight
      + step + exit-code gating"
- **Q10 — Stashed-draft scope.** Is the temporary stashed buffer kept only while
  recalling history (cleared on submit), and does it survive a mode switch
  (`Mult` ↔ normal, push/pop stack)?
    - it survives until popped. Push a "mult", do several other commands, pop
      gives us the buffer and the "mult" mode. If user then invoked "laat" and
      pushed, when popped it would be the buffer and the "laat" mode.

## Dependencies / sequencing

- **Depends on 005** for the multi-line input buffer and the reserved mode field
  in the status bar.
- **Benefits from 006** if LAAT wants configurable, per-mode bindings (otherwise
  its keys are hardcoded like 005's).
