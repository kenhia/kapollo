# Research: Grid Model + Mouse Selection + Alt-Screen Hand-over

> Answers **Q1** (is the advanced mouse UX achievable?) and surveys the building
> blocks for **Q2/Q3**. Status: research draft for review.

Last updated: 2026-05-31

## TL;DR

- **Yes, it's all achievable.** Terminal multiplexers (tmux, zellij) and GPU
  terminals/embedders (wezterm, Zed's terminal, the `tui-term` widget) do exactly
  this on top of a grid model. None of it is novel; it's well-trodden.
- **It requires owning a cell grid.** Mouse selection and app-driven scrollback
  are impossible without modeling cells, because capturing the mouse turns off
  the host terminal's native selection.
- **Mature Rust building blocks exist:** `alacritty_terminal` (the real thing),
  `vt100` + `tui-term` (lighter, ratatui-native), `wezterm-term`/`termwiz`.

## 1. Why a grid is mandatory for the mouse UX

The current Output Processor uses `vte` only to (a) find OSC 133 marks, (b)
detect alt-screen, and (c) strip styling — it deliberately throws away cursor
motion, in-place overwrites, and color (D4/§6). To support the asks we must keep
and *apply* those operations to a model:

- **Native feel** = correctly applying `\r`, cursor moves (`CUP`, `CUU`...),
  erase (`ED`/`EL`), and SGR color to a rows×cols cell buffer with scrollback.
  That model *is* a terminal grid.
- **Mouse selection**: `crossterm` `EnableMouseCapture` delivers mouse
  down/up/drag/scroll events to the app — but in exchange the **terminal stops
  doing its own click-drag selection**. So the app must (1) translate mouse
  coordinates to grid cells, (2) track a selection range, (3) render the
  highlight, and (4) copy the selected cells to the clipboard. All four need the
  grid.
- **Escape hatch**: virtually every terminal lets the user hold **Shift** (Linux/
  Windows) or **Option** (macOS) to bypass app mouse capture and use *native*
  selection. That's a useful fallback but not a substitute — power users expect
  app-level selection to "just work."

## 2. Alt-screen hand-over (the part Ken flagged)

This is a solved problem in multiplexers. The model:

- kapollo enables mouse reporting toward the **host** terminal (so it receives
  events) and tracks whether the **inner program** has requested mouse reporting
  (the program emits `CSI ? 1000/1002/1003/1006 h` to turn mouse modes on).
- **Main screen, inner app not requesting mouse** → kapollo consumes mouse
  events itself: wheel scrolls the transcript, drag selects text.
- **Alt-screen, or inner app requested mouse** → kapollo **forwards** mouse
  events to the PTY, re-encoded in the mode the app asked for (X10/SGR). `vim`,
  `bpytop`, `htop`, `less -R` then behave natively.
- On alt-screen **enter** kapollo suspends its own selection/scroll and routes
  everything down; on **leave** it restores its own handling. (Same boundary we
  already detect via `?1049h`/`?1049l`.)
- Optional nicety (tmux-style): for main-screen apps with **no** mouse mode,
  translate wheel events into Up/Down key presses so `less`/pagers scroll. Opt-in.

**Conclusion for Q1:** achievable, with a clear, conventional routing rule keyed
on (alt-screen?) × (inner app mouse mode?). The grid model is the enabler.

## 3. Clipboard

- **OSC 52**: write selected text to the system clipboard *through* the host
  terminal. Works over SSH, no platform clipboard dependency. Used by tmux,
  wezterm, neovim. Some terminals gate it behind a setting; size limits vary.
- **Local crate** (`arboard`): direct system clipboard access when running
  locally; no terminal cooperation needed, but doesn't traverse SSH.
- **Recommendation:** OSC 52 as the default (SSH-friendly, terminal-mediated),
  with an optional local-clipboard path. Decide in spec.

## 4. Crate landscape (Q3 input)

| Crate | What it is | Pros | Cons | Fit |
|-------|-----------|------|------|-----|
| **`alacritty_terminal`** | Alacritty's grid+parser+scrollback as a library (~8.6K SLoC, used by Zed) | Most correct & battle-tested; real scrollback; selection primitives; damage tracking | Larger, less-"stable" public API; you drive rendering yourself | Best fidelity; heavier lift |
| **`vt100`** | Compact terminal screen parser/model | Simple API; easy to embed | Scrollback/selection are more your problem; fewer edge cases covered | Good for a fast spike |
| **`tui-term`** | ratatui **widget** wrapping `vt100` (v0.3.4, active, ratatui 0.30 track) | Drops a live PTY screen into a ratatui `Rect`; least glue to "embed a terminal"; matches our stack | Inherits `vt100` limits; widget-shaped (may fight a custom scrollback/selection design) | Fastest path to a working prototype |
| **`wezterm-term` / `termwiz`** | wezterm's terminal model + cell/escape libs | Very complete; great Unicode/grapheme + hyperlink/image support | Big dependency surface; API churn; heavier | Powerful but heavy |
| **hand-rolled** | our own grid over `vte` | Full control; minimal deps | We re-implement a terminal emulator — large, bug-prone, exactly the work D4 tried to avoid | Not recommended |

### Reading
- For a **prototype/spike**: `tui-term` (or raw `vt100`) — quickest to prove the
  feel + mouse routing in our existing ratatui app.
- For the **real thing**: lean `alacritty_terminal` for correctness and proper
  scrollback/selection, unless the spike shows `vt100` is "good enough."

## 5. Grid scope options (Q2)

- **A. Whole main screen as one emulated terminal w/ scrollback.** Output is one
  continuous grid; OSC 133 marks + exit codes become an **annotation layer**
  (a block = a row range + command + exit code, plus retained bytes for `/save`).
  *Best native feel; cleanest selection across the whole transcript; blocks
  become metadata over rows.* Most aligned with the asks.
- **B. Per-block mini-grid.** Each block owns a small `vt100` screen. Keeps the
  block boundary crisp, but stitching many grids into one continuous,
  selectable, scrollable surface is awkward (selection across blocks, reflow on
  resize, scrollback math). More moving parts.
- **C. Grid only during alt-screen (status quo+).** Rejected — doesn't deliver
  inline fidelity, inline color, or main-screen selection (#2/#3).

**Leaning:** **A** — emulate the main screen, layer blocks on top. It gives the
native feel, makes selection/scroll uniform, and preserves the block model as an
index over grid rows (so `/save`, `/filter`, AI all still work).

## 6. Rendering note

ratatui repaints whole frames via a diffing backend; embedding a real grid works
(`tui-term` proves it) and is fine at kapollo's scale. If profiling ever shows
the terminal region is a bottleneck, the region can be painted more directly
(damage-tracked) while ratatui still owns the chrome. Not a near-term concern.

## 7. Risks / unknowns to probe in a spike

- Reflow-on-resize semantics with scrollback (alacritty handles it; vt100 less so).
- Wide chars / graphemes / combining marks fidelity (wezterm strongest here).
- Performance of full-grid rendering through ratatui under flood (we already
  fixed flood responsiveness once; grid raises the per-cell cost).
- Mouse-mode encoding correctness (X10 vs SGR 1006) when forwarding to inner apps.
- OSC 52 clipboard support/size across Ken's terminals.

## 8. Open questions feeding back to 00-overview

- Q2 scope (A/B/C) — leaning A; confirm.
    - Agree, Option A
- Q3 crate — spike with `tui-term`/`vt100`, target `alacritty_terminal` for prod?
    - Yes; but see longer response in `specs/planning/grid-pivot/00-overview.md`, 6.4 and 6.5
- Clipboard default (OSC 52) — confirm.
    - Confirm; we may get more information here during "spike"; Should we look
      at other terminal emulators during spike (I currently use "Window Terminal
      Preview" almost exclusively, might be good to try some on a Ubuntu desktop
      (Gnome/KDE)) and others that work on Windows/Linux/both (I do have a Mac,
      but I'll have to get it up to date so probably out of scope for spike)
