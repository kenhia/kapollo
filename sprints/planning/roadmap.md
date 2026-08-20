# Roadmap

> The general plan for this project. Keep it current; detail lives in the
> sprint records.

## Now

- **010 — core UX, input & shell commands** (korg proposal #176, branch
  `010-core-ux-input-shell`). All eleven covered items resolved 2026-08-19;
  awaiting review + ship. The payload was **#34** — the in-flight command FIFO
  that ends typed-ahead output loss. Also: running indicator (#35), input-pad
  prompt (#37), `Ctrl+L` semi-clear (#42), whitespace-suppression knobs (#46),
  OSC 133 `A`/`B` prompt capture + the divider prompt fold (#47),
  click-vs-drag selection (#45, research + implemented), the Constitution
  reference cleanup (#1464). **#36** was verified already shipped in 005/006
  and closed without code; **#48** does not reproduce in the current grid and
  is pinned by regression tests replaying the shells' real SIGWINCH repaint
  bytes. Details in `sprints/010-core-ux-input-shell.md`.

## Next

- **Templated ("fancy") status bar** — user-definable status-line content via a
  small template string, replacing sprint 005's fixed format. Pre-planned in
  `planning/pre-plan-fancy-status-bar.md`; that file used to claim sprint 008,
  which the harness migration took, so it lands under a later number.

## Later / Ideas

- **Completions** and **persistent input history** — the README names these two
  as the gaps keeping kapollo from daily use. Neither has a korg work item yet.
