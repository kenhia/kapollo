# Phase 0 Research: Terminal-Grid Spike

**Feature**: 003-grid-spike | **Date**: 2026-06-01

This document resolves the open technical unknowns before building the slice. The
spec had **no `[NEEDS CLARIFICATION]` markers** (the planning docs settled the
direction); the unknowns here are implementation-level choices for the spike code.

All crate versions are the **current** published versions as of 2026-06-01, per the
maintainer's instruction to use current versions. Toolchain: Rust 1.95.0.

---

## R1 — `wezterm-term` is not published on crates.io (S3 dependency strategy)

**Decision**: Depend on `wezterm-term` as a **git dependency pinned to a specific
wezterm commit/tag**, not a crates.io version. `termwiz` (0.23.3 is on crates.io)
comes along transitively as wezterm-term's dependency and provides the cell/surface
escape model.

```toml
# delos/spike-wezterm/Cargo.toml
[dependencies]
wezterm-term = { git = "https://github.com/wezterm/wezterm.git", rev = "<pin>" }
# termwiz arrives transitively; pin via the same rev.
```

**Rationale**: `cargo search`/crates.io confirm `wezterm-term` does not exist on
crates.io (only third-party forks like `tattoy-wezterm-term`). The real terminal
*model* (screen, scrollback, line reflow) lives in `wezterm-term`, which the wezterm
project ships only via its monorepo. A pinned `rev` keeps the spike reproducible.
`termwiz` alone is the lower-level escape/cell library and lacks the screen+scrollback
model the slice needs, so depending on `termwiz` standalone is insufficient for S3.

**Alternatives considered**:
- *`termwiz` only (0.23.3, crates.io)* — rejected: no built-in terminal screen/
  scrollback model; we'd be hand-rolling the very thing S3 is meant to evaluate.
- *A published fork (`tattoy-wezterm-term`)* — rejected: unknown divergence from
  upstream; defeats the purpose of evaluating wezterm's real model.
- *Vendoring the crate* — rejected: heavier than a git pin for throwaway code.

**Spike action**: pick the pin at S3 start (latest green wezterm tag), record the
exact `rev` in `delos/docs/s3-wezterm.md`. Note build-weight/compile-time in the
scorecard (wezterm-term pulls a large transitive tree — this is itself a data point).

---

## R2 — Render/backend versions for the spike workspace

**Decision**: The `delos/` workspace uses **current** versions, independent of the
shipping crate: `ratatui` 0.30.0, `crossterm` 0.29.0, `portable-pty` 0.9.0. Kapollo
ships `ratatui` 0.29 / `crossterm` 0.28 / `portable-pty` 0.8; the spike does **not**
match those.

**Rationale**: (1) The workspaces are disjoint (FR-003), so there is no version-
sync constraint. (2) The rework will almost certainly upgrade ratatui anyway;
exercising 0.30 in the spike **de-risks that upgrade** and surfaces API drift early.
(3) `tui-term` 0.3.4 targets the ratatui 0.30 line, so S1's optional accelerator
wants 0.30. (4) `crossterm` 0.29 mouse/event API is what we'll carry forward.

**Alternatives considered**:
- *Match kapollo's 0.29/0.28/0.8* — rejected: no benefit (disjoint graphs) and it
  would force `tui-term` onto an older line; misses the chance to vet the upgrade.

---

## R3 — Clipboard: OSC 52 primary, `arboard` as the evaluated fallback

**Decision**: Default copy path is **OSC 52**, hand-emitted (`ESC ] 52 ; c ;
<base64> ST`) from `spike-support` — no crate needed for the primary path. For
terminals that do not honor OSC 52, evaluate **`arboard` 3.6.1** as the local
fallback (cross-platform: X11/Wayland/Windows/macOS).

**Rationale**: OSC 52 works over SSH and needs no system clipboard access, matching
the planning default. `arboard` is the better-maintained, more widely-used local
clipboard crate vs `copypasta` (0.10.2), with active releases and broad platform
support. The spike only needs to *evaluate* the fallback, not productize it.

**Alternatives considered**:
- *`copypasta` 0.10.2* — viable but less active than `arboard`; keep as a note.
- *No fallback (OSC 52 only)* — rejected: FR-021 requires evaluating a fallback;
  Windows Terminal/older terminals' OSC 52 support varies, which is the whole point
  of recording per-terminal behavior (FR-020, SC-006).

**Spike action**: record per host terminal whether OSC 52 copy worked (the matrix in
quickstart.md), and whether `arboard` was needed.

---

## R4 — Alt-screen and child-mouse-mode detection (routing)

**Decision**: Detect mode transitions by **scanning the child's PTY output stream**
for the relevant DEC private mode set/reset sequences, in `spike-support`, regardless
of crate:
- Alt screen: `CSI ? 1049 h` (enter) / `CSI ? 1049 l` (exit) — also accept legacy
  `?47h/l` and `?1047h/l`.
- Child mouse reporting: `CSI ? 1000/1002/1003/1006 h` (enable) / `… l` (disable).

While alt-screen is active OR child mouse reporting is enabled, the slice **forwards**
mouse/input to the child and suspends its own grid ownership/selection; on exit it
restores ownership.

**Rationale**: A stream-level detector is crate-agnostic, so the routing logic is
shared (identical-plumbing principle) and each crate's parser only has to render the
main screen. Some crates (alacritty_terminal, wezterm-term) also expose alt-screen
state on their model; we cross-check the crate's own signal against our detector and
note discrepancies in the writeup (a fidelity data point). `vt100` exposes screen
mode via its `Screen`/`alternate_screen` API; if its signal is reliable we prefer it
for S1 and record that.

**Alternatives considered**:
- *Rely solely on each crate's alt-screen flag* — rejected as the primary mechanism:
  divergent APIs would make the routing non-identical across stages; instead we keep
  one shared detector and treat the crate flag as corroboration.

**Note**: This detector lives in `spike-support` and is **unit-testable** (feed byte
sequences, assert state transitions) — one of the few pure helpers we TDD.

---

## R5 — Selection coordinate model (content vs screen)

**Decision**: Anchor selection in **content coordinates** = `(absolute_row, col)`
where `absolute_row` indexes into the full scrollback history (0 = oldest retained
line), not the visible viewport. The viewport is a window `[top_row, top_row+height)`
over that history. Screen→content mapping: `absolute_row = top_row + screen_y`.
Auto-scroll on drag-past-edge mutates `top_row`; the selection anchor/active end are
stored in content space so they are invariant under scrolling.

**Rationale**: FR-008 requires selection to survive scrolling; storing endpoints in
content space makes that automatic. The mapping math (clamp, edge detection,
auto-scroll step, range normalization when end < anchor) is **pure and unit-tested**
in `spike-support` — the second TDD'd helper.

**Alternatives considered**:
- *Screen coordinates + reconcile on scroll* — rejected: fragile, must patch
  endpoints on every scroll event; exactly the bug class FR-008 guards against.

**Edge rules** (encoded as tests):
- Drag y < 0 (above top) → scroll up by N, clamp at `top_row = 0`.
- Drag y ≥ height (below bottom) → scroll down by N, clamp at bottom of scrollback.
- end before anchor → normalize so copy yields document order.
- Selection scoped to output region: clamp `col`/`row` to the output rect (FR-009).

---

## R6 — `tui-term` for S1 acceleration (use it or not)

**Decision**: Start S1 with **`tui-term` 0.3.4** (a ratatui widget wrapping `vt100`)
to reach first pixels fast; if its widget abstraction blocks the selection/scroll
work, drop to the **raw `vt100` 0.16.2 API**. Record which path was taken and why in
`delos/docs/s1-vt100.md`.

**Rationale**: The spec (assumption) explicitly allows deciding during S1. `tui-term`
minimizes boilerplate for render; but because the slice needs custom
content-coordinate selection + auto-scroll, the raw `vt100` `Screen` may be required
for cell access. Time-box the `tui-term` attempt, then fall back without ceremony.

**Compatibility caveat**: confirm `tui-term` 0.3.4 resolves against `ratatui` 0.30 at
S1 start (T014). If 0.3.4 pins an older ratatui line, either pin the matching
`tui-term` release or skip straight to the raw `vt100` 0.16.2 API — the fallback path
above already covers this with no schedule impact.

---

## R7 — PTY plumbing reuse

**Decision**: `spike-support` owns a small `spawn_shell_pty()` using `portable-pty`
0.9.0 that mirrors kapollo's existing setup (default user shell, sane `TERM`, resize
propagation, reader thread → channel of bytes). It is re-implemented in `delos/` (not
imported from kapollo) to preserve workspace isolation (FR-003).

**Rationale**: Reusing the *approach* (not the dependency) keeps `delos/` disjoint.
The plumbing is small and identical across stages, so it belongs in the shared crate.

**Alternatives considered**:
- *Import kapollo's pty module as a path dep* — rejected: would couple the
  workspaces and risk pulling kapollo (and a portable-pty version clash) into the
  spike graph; violates the isolation rule.

---

## Resolved decisions summary

| # | Topic | Decision |
|---|-------|----------|
| R1 | wezterm-term source | git dep pinned to a wezterm `rev`; termwiz transitively |
| R2 | render/backend versions | current: ratatui 0.30, crossterm 0.29, portable-pty 0.9 (disjoint from kapollo) |
| R3 | clipboard | OSC 52 default (hand-emitted); `arboard` 3.6.1 as evaluated fallback |
| R4 | mode detection | shared stream-level escape detector (alt-screen + child mouse modes); unit-tested |
| R5 | selection coords | content (absolute scrollback) coordinates; pure mapping math unit-tested |
| R6 | S1 accelerator | try `tui-term` 0.3.4 first, fall back to raw `vt100` 0.16.2 |
| R7 | PTY | re-implement portable-pty spawn in `spike-support` (no kapollo import) |

All unknowns resolved. No remaining `[NEEDS CLARIFICATION]`.
