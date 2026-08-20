# Roadmap

> The general plan for this project. Keep it current; detail lives in the
> sprint records.

## Now

- **008 — kprojects harness.** Off Spec-Kit onto the minimal harness
  (korg #1462). Layout and conventions only; no behaviour change.

## Next

- **Re-scope the two standing proposals before starting either.** Sprints 005–007
  shipped work that korg still shows as open, and neither proposal was refreshed
  afterwards — see sprint 008's follow-ups for the evidence per item. korg #37,
  #43, #44 and #46 all look shipped; #42, #45 and #47 do not.
- **kapollo core UX** — korg proposal #176: the output-loss bug on concurrent
  submit (#34), a running-command indicator (#35), transcript scrolling with
  context overlap and single-line scroll (#36), and the spurious shell reprompt
  `/status` triggers via PTY SIGWINCH (#48).
- **Input & shell commands** — korg proposal #184, sequenced after #176. Of its
  six items, Ctrl-L semi-clear (#42) and the user prompt in the divider (#47)
  are genuinely outstanding, and the mouse click-vs-drag threshold (#45) is
  still research. The proposal may be mostly finished already.
- **Templated ("fancy") status bar** — user-definable status-line content via a
  small template string, replacing sprint 005's fixed format. Pre-planned in
  `planning/pre-plan-fancy-status-bar.md`; that file used to claim sprint 008,
  which the harness migration took, so it lands under a later number.

## Later / Ideas

- **Completions** and **persistent input history** — the README names these two
  as the gaps keeping kapollo from daily use. Neither has a korg work item yet.
