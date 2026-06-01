# Contract: Spike Binary Runtime

**Feature**: 003-grid-spike | Applies to: `spike-vt100`, `spike-alacritty`, `spike-wezterm`

Every spike binary obeys the **same** runtime contract so the three stages are
directly comparable and the manual test script (quickstart.md) is identical.

## Invocation

```text
spike-<crate> [SHELL]
```

- `SHELL` (optional positional): shell to spawn. Default: `$SHELL` or `/bin/bash`.
- No other flags required. Binaries are throwaway; argument parsing is minimal.
- On start: enter raw mode + enable mouse capture (crossterm); spawn the PTY shell;
  take over the alternate screen of the *host* terminal for the slice's own UI.
- On exit (`/exit`-style quit, child EOF, or Ctrl-Q): restore the host terminal
  (leave raw mode, disable mouse capture, leave alt screen) — even on panic.

## Layout

- Single output region = the emulated **main screen** of the child (scrollback
  enabled). No input pad, no block chrome, no status line required (out of scope).
- Minimum viable chrome only: enough to see the grid and the selection highlight.

## Keyboard

| Key | Behavior |
|-----|----------|
| (printable / control) | forwarded verbatim to the child PTY |
| `Ctrl-C` (no active selection) | **SIGINT to child** (FR-015) |
| `Ctrl-C` (active selection) | **copy selection**, clear selection (FR-016) |
| `ESC` (active selection) | cancel selection (FR-018) |
| `PgUp` / `PgDn` | scroll scrollback by a page (parity with wheel) |
| `Ctrl-Q` | quit the spike (restore terminal) |

> `Ctrl-C` routing is **state-based**: the binary checks `Selection.state` first.

## Mouse

| Event | No active selection | Active selection |
|-------|---------------------|------------------|
| left-press (no Shift) | begin drag (set anchor) → Dragging | **cancel selection → Idle** (FR-018; no copy) |
| left-press (Shift) | forward to child (FR-017) | forward to child |
| mouse-move while pressed | extend selection; auto-scroll past edge (FR-010) | — |
| release | **no action** — if a drag was in progress it finalizes to Active and the highlight stays; nothing is copied | — (selection unchanged) |
| right-press | open "Hello, World." menu (FR-019) | **copy selection, then deselect** (FR-016) |
| wheel up/down | scroll scrollback (FR-012) | scroll (selection survives, R5) |

> **Copy is never implicit.** Release only *finalizes* the selection (it becomes
> Active and stays highlighted); it does **not** copy. Copy happens **only** via an
> explicit trigger while a selection is Active: **right-press** or **Ctrl-C**, each
> of which copies and then deselects (returns to Idle). This keeps a finalized
> selection visible and re-copyable until the user explicitly copies or cancels
> (second left-press / ESC).

## Routing override (alt-screen / child mouse modes)

While the child has entered the alternate screen (`?1049h`) OR enabled mouse
reporting (`?1000/1002/1003/1006h`), ALL mouse + key events are forwarded to the
child and the binary suspends its own selection/scroll handling (FR-013, FR-014).
Normal handling resumes on the corresponding reset sequence or alt-screen exit.

## Clipboard

- Default: emit OSC 52 (`spike-support::osc52_frame`).
- Fallback: if a `--clipboard=arboard` flag is passed (optional), use `arboard`
  instead, to evaluate terminals that drop OSC 52.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | clean quit (child EOF or Ctrl-Q) |
| non-zero | unrecoverable spike error (terminal still restored) |
