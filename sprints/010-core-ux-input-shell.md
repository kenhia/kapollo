# 010 — core UX, input & shell commands

korg proposal #176 · items #34 #35 #37 #42 #45 #46 #47 #48 #1464, plus #36
closed as already shipped. Branch `010-core-ux-input-shell`, run 2026-08-19.

## Goal

Clear the deck: everything open in kapollo, merged into proposal #176 after the
2026-08-19 verification pass. The payload is **#34** — output loss when a
command is submitted while another is running — the only correctness bug in the
set. Ken's instruction: proceed with implementation, make decisions as needed,
surface anything noteworthy here.

## ⚠ For Ken — decisions made and things worth knowing

- **#36 was closed as already shipped, not implemented here.** Every acceptance
  criterion landed in sprints 005/006: `scroll.context_lines` (default 3) with
  the degenerate clamp in `Transcript::page_advance`, `scroll_line_up`/`_down`
  on Shift+PgUp/PgDn, rebindable via the keymap, documented in usage.md. The
  2026-08-19 verification pass had listed #36 "genuinely open, no
  implementation found" — the same failure mode that pass corrected twice
  elsewhere, in the other direction. Evidence is on the korg item.
- **#48 does not reproduce.** A raw-PTY probe of the real shells showed both
  repaint the prompt **in place** on SIGWINCH (fish 3.7 + starship:
  `\r ESC[A ESC[A ESC[J` + redraw; bash 5.2: `\r ESC[K \r` + redraw).
  Replaying those byte shapes against the grid across one-row shrink/grow (the
  `/status` geometry) leaves exactly one prompt — pinned in
  `tests/reprompt_resize.rs`. The remaining one-row shift on toggle is the pane
  genuinely gaining/losing a row, same as a real terminal resize. The WI's
  recommended "A/B-gated suppression" is moot; closed with the evidence.
- **#35's written design was obsolete; re-grounded it.** The WI proposed
  recoloring the transcript's echoed prompt and moving the between-blocks blank
  line — both grounded in the pre-grid block renderer, dead code since sprint
  004 (the transcript is the shell-painted wezterm grid; kapollo can't restyle
  it). Implemented on surfaces kapollo owns: the status bar's exit slot shows
  `…` while a command runs (glyph, so NO_COLOR-safe and visible when
  consecutive exit codes match), and the input-pad prompt glyph wears the new
  `running_color` (default yellow).
- **#45 went one step past research.** The recommended rule was implemented
  (it is ~10 lines + tests): release activates a selection only if the pointer
  left the anchor cell during the drag. A plain click no longer leaves a
  one-cell selection that hijacks Ctrl-C (copy instead of SIGINT!) and
  right-click; a deliberate single-cell selection still works by dragging out
  and back. Feasibility verdicts recorded on the item: SGR-Pixels (1016) is
  unusable through crossterm 0.29, which would mis-parse pixel coords as cells.
- **#47's config shape simplified.** Instead of the WI's separate
  one-line/two-line modes, the prompt's shape is auto-detected from the
  captured span: a multi-line prompt brings its last line down, a one-line
  prompt brings its trailing `prompt_bring_down` chars (default 2). One
  toggle: `[divider] prompt` (default off). The brought-down tail replaces the
  λ input-pad glyph while active — two prompts side by side would be noise.
- **bash A/B marks survive prompt managers**: the `PS1` wrap is re-applied at
  the END of every `PROMPT_COMMAND` cycle, so starship-style setups that
  rewrite `PS1` each prompt can't strip the marks. Verified live against fish
  3.7 + starship (your actual two-line λ prompt) and bash 5.2.
- **Known corner, unchanged by design**: LAAT exit-code gating still applies to
  every `CommandEnd` (pre-sprint semantics). If a LAAT line is submitted while
  an unrelated command is still running, the earlier command's exit code gates
  the line. The in-flight queue could key gating to the LAAT line's own block
  if this ever bites; deemed not worth the plumbing for a stepping mode that is
  inherently serial.
- **Uncommitted `.vscode/mcp.json` change left in your working tree** (removes
  a dead `kwi` MCP server entry). Not sprint work; not committed.
- Branch name `010-core-ux-input-shell` chosen without confirmation (you were
  away; numbering follows 009).

## The #34 bug, precisely

Submitting while a command ran corrupted the block model three ways
(src/app.rs pre-sprint):

1. `run_shell_labeled` overwrote the single `current_block` /
   `current_store_block` slots, orphaning the still-running block.
2. `processor.begin_command()` reset `capturing`, so the in-flight command's
   remaining output was dropped from the transcript block.
3. The eventual OSC 133 `D` closed whichever block the slots now held — the
   *new* command's block sealed with the *old* command's exit code.

The PTY byte stream is strictly ordered (output₁, D₁, prompt, echo₂, C₂,
output₂, D₂ …), so the fix is a FIFO: `OutputProcessor::process()` yields
capture-gated events in stream order, and the new `BlockAssembler`
(src/output/assembler.rs) owns the in-flight queue — submissions push, the end
mark closes the front and pops. `begin_command` fires only when the queue is
idle. Typed-ahead input keeps working exactly as in a real terminal (the kernel
buffers the bytes; the shell reads them at its next prompt) — kapollo never
queues the *write*, only the block bookkeeping. `/pipe` completion is now keyed
to its own block id instead of "whatever command ends next" (a latent bug the
old shape made unavoidable).

## What shipped

| Item | What landed |
|---|---|
| **#34** output loss on concurrent submit | `BlockAssembler` in-flight FIFO + event-stream `OutputProcessor::process()`; synthetic regression test + live fish/bash typed-ahead test (`tests/concurrent_submit.rs`) |
| **#35** running indicator | Status-bar exit slot shows `…` while running; input-pad prompt wears `running_color` (new top-level key, default yellow) |
| **#36** scrolling overlap + line scroll | Closed as shipped (sprints 005/006); evidence on the korg item |
| **#37** input-pad prompt | `λ ` prefix on the first line, indent-aligned continuations, cursor accounting; `input_prompt` toggle (default on) |
| **#42** Ctrl-L semi-clear | `semi_clear` rebindable action + `Grid::clear_viewport` (`ESC[nS ESC[H` — verified against the pinned wezterm-term rev that `ESC[2J` erases in place and would LOSE the rows) |
| **#45** click-vs-drag | Research verdicts + the moved-latch rule in `SelectionController`; plain click places no selection |
| **#46** whitespace suppression | `suppress_multiline_whitespace` (default false, overrides) + `suppress_multiline_trailing_whitespace_lines` (default true) → `WhitespaceSuppression` policy at submit |
| **#47** prompt into divider | Hooks emit OSC 133 `A`/`B`; processor diverts the span into `ProcessorEvent::Prompt`; `[divider] prompt` folds the head into the rule and brings the tail down as the input-pad prefix |
| **#48** /status reprompt | Does not reproduce; PTY probe + 3 regression tests replaying the shells' real repaint bytes (`tests/reprompt_resize.rs`) |
| **#1464** Constitution refs | All 17 surviving citations inlined or dropped; sprint records keep theirs as history |

Gate: `just check` green throughout; **336 tests across 41 suites** at close
(305 across 38 before the sprint). Docs updated in the same commits: usage.md,
specification.md (new FR-S26…S32), architecture.md (assembler + hook marks),
keymap-defaults.toml, README.

## Decisions

- **Association moved out of the processor.** `OutputProcessor` now only
  parses, gates capture, and tracks prompt spans; block association lives in
  `BlockAssembler`, which both the app and the integration tests drive — one
  code path, unit-testable without a PTY.
- **Sentinel mode keeps capturing across `CommandEnd`** (it has no `C` mark to
  re-open capture), so a typed-ahead command's output isn't lost there either;
  spans with nothing in flight are dropped by the assembler.
- **A prompt span that loses its `B` mark aborts at `C`/`D`** rather than
  mis-capturing command output as prompt text (defends against a prompt
  manager clobbering the bash `PS1` wrap mid-session).
- **Semi-clear scrolls by cursor row + 1**, not the full viewport, so no
  trailing blank rows pollute scrollback; it is view-side only (the shell is
  not informed — same contract as grid-injected synthetic blocks).
- Everything else: see the per-item entries under "For Ken" above.

## Follow-ups

- The **templated status bar** (pre-plan exists) is now the natural next
  sprint; the running marker currently rides the exit slot and could become a
  template field there.
- If a duplicated prompt ever reappears on `/status` toggle in live use,
  capture the bytes with a PTY probe and extend `tests/reprompt_resize.rs`
  with the new shape — that is the designated home for that class of bug.
- `ui::transcript::lines()` remains test-only legacy of the pre-grid renderer
  (exercised by `tests/chrome.rs`); candidate for deletion in a cleanup sprint
  together with its tests, which pin behavior nothing user-visible uses.
