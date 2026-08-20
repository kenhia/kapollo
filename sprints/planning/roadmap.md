# Roadmap

> The general plan for this project. Keep it current; detail lives in the
> sprint records.

## Now

Nothing in flight. Two chores shipped 2026-08-19 and the backlog was
re-verified in the same pass:

- **008 — kprojects harness.** Off Spec-Kit onto the minimal harness
  (korg #1462). Layout and conventions only; no behaviour change. **Done.**
- **009 — `/filter` → `/pipe`.** Hard rename, no alias (korg #1463). The
  command pipes one block's output through a shell command; the old name said
  it filtered the transcript, and that misnomer had already caused a finished
  feature to be reported as unbuilt. **Done.**

## Next

- **korg proposal #176 — core UX, input & shell commands.** The two standing
  proposals were verified item by item and merged on 2026-08-19: #184 is
  declined and #176 carries everything open. **#34 leads** — it is the only
  correctness bug in the set.

  | | Items |
  |---|---|
  | render + correctness | #34 output loss on concurrent submit · #35 running-command indicator · #36 scrolling context overlap + single-line · #37 input-pad prompt char · #48 spurious reprompt on `/status` (PTY SIGWINCH) |
  | input + shell commands | #42 Ctrl-L semi-clear · #45 click-vs-drag research · #46 whitespace-suppression config knobs · #47 prompt into divider |
  | harness fallout | #1464 dangling `Constitution` references |

  **Three items closed as already shipped:** #43 `/save`; #44 `/filter`, after
  Ken confirmed the shipped pipe-the-previous-block behaviour was the intent all
  along and its work item's "filter the transcript" framing overstated it; and
  #1463, the rename that came out of that (sprint 009). See sprint 008's two
  postscripts for the verification trail.

  Two planning notes: **#47**'s divider prerequisite is satisfied but it still
  needs OSC 133 `A`/`B` marks out of the injected shell hooks
  (`src/pty/shell.rs` emits only `C`/`D` + OSC 7). **#1464** is sprint 008's own
  fallout — deleting the constitution left 18 references pointing at a file that
  no longer exists; the fix is to inline the two principles that were actually
  load-bearing, not to resurrect the document.

- **Templated ("fancy") status bar** — user-definable status-line content via a
  small template string, replacing sprint 005's fixed format. Pre-planned in
  `planning/pre-plan-fancy-status-bar.md`; that file used to claim sprint 008,
  which the harness migration took, so it lands under a later number.

## Later / Ideas

- **Completions** and **persistent input history** — the README names these two
  as the gaps keeping kapollo from daily use. Neither has a korg work item yet.
