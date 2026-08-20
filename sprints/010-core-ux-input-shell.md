# 010 — core UX, input & shell commands

korg proposal #176 · items #34 #35 #37 #42 #45 #46 #47 #48 #1464 (#36 closed as
already shipped, see below)

## Goal

Clear the deck: everything open in kapollo, merged into proposal #176 after the
2026-08-19 verification pass. The payload is **#34** — output loss when a
command is submitted while another is running — the only correctness bug in the
set. The rest is render/input polish (#35 #36 #37 #42 #46), the OSC 133 `A`/`B`
plumbing and the two features gated on it (#47 #48), one research item (#45),
and sprint 008's docs fallout (#1464).

Run start: 2026-08-19, branch `010-core-ux-input-shell`. Ken's instruction:
proceed with implementation, make decisions as needed, surface anything
noteworthy here.

## ⚠ For Ken — decisions made and things worth knowing

*(Written during the sprint; roll-up of anything decided without asking.)*

- **#36 was closed as already shipped, not implemented here.** Every
  acceptance criterion landed in sprint 005/006: `scroll.context_lines`
  (default 3) with the degenerate clamp in `Transcript::page_advance`
  (src/session/mod.rs), `scroll_line_up`/`scroll_line_down` on Shift+PgUp/PgDn
  (src/action/mod.rs BINDINGS), modifier configurable via the sprint-006
  keymap, and it is documented in docs/usage.md ("Scroll one line at a time").
  The 2026-08-19 verification pass listed #36 as "genuinely open, no
  implementation found" — the same failure mode that pass corrected twice
  elsewhere, in the other direction. Closed on korg with the evidence.
- **#35's written design was obsolete; re-grounded it.** The WI proposes
  recoloring the transcript's echoed prompt and moving the between-blocks
  blank line to command completion — both grounded in the pre-grid block
  renderer (`ui::transcript::lines`), which since sprint 004 is test-only dead
  code. The real transcript is the wezterm grid, painted by the shell; kapollo
  can't restyle content it doesn't own. Implemented instead on surfaces
  kapollo does own: the status bar's exit field shows a running marker while a
  command is in flight (visible under NO_COLOR), and the input-pad prompt
  char (#37) wears a distinct running color. Details below.
- **Uncommitted `.vscode/mcp.json` change left in the working tree** (removes
  a dead `kwi` MCP server entry). Not sprint work; not committed.
- Branch name `010-core-ux-input-shell` chosen without confirmation (Ken was
  away; numbering follows 009).

## The #34 bug, precisely

Submitting while a command runs corrupted the block model three ways
(src/app.rs pre-sprint):

1. `run_shell_labeled` overwrote the single `current_block` /
   `current_store_block` slots, orphaning the still-running block.
2. `processor.begin_command()` reset `capturing`, so the in-flight command's
   remaining output was dropped from the transcript block.
3. The eventual OSC 133 `D` closed whichever block the slots now held — the
   *new* command's block sealed with the *old* command's exit code.

The PTY byte stream itself is strictly ordered (output₁, D₁, prompt, echo₂,
C₂, output₂, D₂ …), so the fix is a FIFO of in-flight commands: submissions
push, `D` closes the front and pops, capture gating is left alone while
something is in flight. Typed-ahead input keeps working exactly as in a real
terminal (the kernel buffers it; the shell reads it at the next prompt) — and
input meant as stdin for a running program (e.g. a REPL) still reaches the PTY
immediately, because kapollo never queues the *write*, only the block
bookkeeping.

## What shipped

*(updated as items land)*

## Decisions

*(updated as items land)*

## Follow-ups

*(updated as items land)*
