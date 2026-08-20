# Roadmap

> The general plan for this project. Keep it current; detail lives in the
> sprint records.

## Now

- **008 — kprojects harness.** Off Spec-Kit onto the minimal harness
  (korg #1462). Layout and conventions only; no behaviour change. **Done.**

## Next

- **korg proposal #176 — core UX, input & shell commands.** The two standing
  proposals were verified item by item and merged on 2026-08-19: #184 is
  declined and #176 now carries **all ten** open items. Only one thing closed
  — **#43 `/save`**, genuinely shipped in sprint 007.

  Ten items is large for one sprint; the natural split is the original seam.
  **#34 leads either way** — it is the only correctness bug in the set.

  | | Items |
  |---|---|
  | render + correctness | #34 output loss on concurrent submit · #35 running-command indicator · #36 scrolling context overlap + single-line · #37 input-pad prompt char · #48 spurious reprompt on `/status` (PTY SIGWINCH) |
  | input + shell commands | #42 Ctrl-L semi-clear · #44 transcript filter over block boundaries · #45 click-vs-drag research · #46 whitespace-suppression config knobs · #47 prompt into divider |

  Two planning notes: **#44** collides with the existing `/filter <cmd>` name
  and needs that decided before it is built; **#47**'s divider prerequisite is
  satisfied but it still needs OSC 133 `A`/`B` marks out of the injected shell
  hooks (`src/pty/shell.rs` emits only `C`/`D` + OSC 7).

- **Templated ("fancy") status bar** — user-definable status-line content via a
  small template string, replacing sprint 005's fixed format. Pre-planned in
  `planning/pre-plan-fancy-status-bar.md`; that file used to claim sprint 008,
  which the harness migration took, so it lands under a later number.

## Later / Ideas

- **Completions** and **persistent input history** — the README names these two
  as the gaps keeping kapollo from daily use. Neither has a korg work item yet.
