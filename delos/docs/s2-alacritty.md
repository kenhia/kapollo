# S2 — `alacritty_terminal`

Nuts-and-bolts writeup for the `alacritty_terminal` stage (crate `spike-alacritty`),
pinned to **0.26.0** (pulls `vte` 0.15).

## Approach

- **Render path: direct grid blit, no widget.** Unlike S1 there is no `tui-term`
  equivalent, so we read cells straight from `Term::grid()` and write them into the
  ratatui `Buffer` ourselves (`buf.cell_mut((x, y))` → `set_symbol` + `set_style`).
  This is the same `Buffer`-poking we already do for S1's selection overlay, just
  extended to the whole viewport. A small `cell_style`/`conv_color` pair maps
  `Cell::flags` → `Modifier` (BOLD/ITALIC/UNDERLINE/INVERSE/DIM) and the
  `vte::ansi::Color` fg/bg (`Spec`→`Rgb`, `Indexed`→`Indexed`, `Named`→16-color
  palette; terminal-default colors fall through to `None`).
- **Feeding bytes:** `vte::ansi::Processor::advance(&mut term, &chunk)`. `Term`
  implements the `vte` `Handler` trait, so the parser writes directly into the grid.
- **Emulator → PTY write-back:** `Term` reports replies (DSR, terminal-ID, etc.)
  through an `EventListener`. Because `send_event(&self, …)` is `&self`, the proxy
  holds an `mpsc::Sender<Vec<u8>>` and forwards `Event::PtyWrite` bytes; the main
  loop drains the receiver and writes them to the shell. (S1's `vt100` had no such
  channel — it simply doesn't answer queries, which can hang some programs.)
- **Sizing:** a tiny `TermDim { cols, lines }` implementing the `Dimensions` trait
  (`columns`/`screen_lines`/`total_lines`), passed to `Term::new` and `Term::resize`.
  Scrollback depth comes from `Config::scrolling_history`, so `total_lines ==
  screen_lines` in the descriptor.
- Reused **unchanged** from `spike-support`: `screen_to_content`,
  `content_to_screen`, `normalize`, `detect_mode`, `osc52_frame`, `copy_local`, the
  `PtyShell` reader-thread wrapper — and the **entire `selection.rs` state machine**
  is byte-for-byte identical to S1 (same 8 tests). That portability is itself a
  spike result: the pure model is engine-agnostic.

## Coordinate bridge (the interesting part)

`alacritty_terminal` keeps the scroll position internally as `grid().display_offset()`
(0 = live bottom, growing as you scroll up) — structurally identical to vt100's
bottom-relative scrollback, so the **same `BASE = SCROLLBACK_LEN` bridge** applies:

```text
top_row     = BASE - display_offset            (BASE at the live tail)
content_row = screen_to_content(top_row, y) = BASE - display_offset + y
buffer_line(visible_row) = Line(visible_row - display_offset)
```

The notable improvement over S1: `display_offset` is owned by the emulator, so
`top_row` is **derived fresh each frame/event** rather than persisted and re-applied.
Scrolling is `term.scroll_display(Scroll::Delta(±n) | PageUp | PageDown)` — no
`set_scrollback(BASE - top_row)` round-trip. One less piece of mutable state.

Grid indexing uses signed lines: visible row `vr` reads buffer `Line(vr -
display_offset)`, where negative lines are scrollback. All values stay in range for
`vr ∈ 0..screen_lines` and `display_offset ≤ scrolling_history`, so both the render
blit and `viewport_matrix` (used by copy) index the grid directly and stay mutually
consistent.

## What worked

- **Compiled clean on the first build** against `ratatui` 0.30 + `crossterm` 0.29 —
  no version-pinning gymnastics (the headline pre-spike risk, again absent).
- The `coords` bridge ported verbatim because `display_offset` mirrors vt100
  scrollback. Auto-scroll on drag past an edge is just a `scroll_display(Delta(±1))`
  followed by recomputing `top_row` from the new offset.
- **Authoritative mode flags:** `term.mode()` exposes a `TermMode` bitflags with
  `ALT_SCREEN` and the mouse-report flags (`MOUSE_REPORT_CLICK | MOUSE_DRAG |
  MOUSE_MOTION`). We corroborate the streaming `detect_mode` hint against these each
  time output arrives — same belt-and-suspenders routing as S1, and the flag set is
  richer/more granular than vt100's single `mouse_protocol_mode()`.
- **PTY write-back via `EventListener`** is a genuine capability S1 lacked: programs
  that probe the terminal (cursor-position reports, device attributes) get answered,
  so they won't stall. This matters for a shipping model.
- Styling round-trips: `Cell` carries explicit `fg`/`bg`/`flags`, so SGR attributes
  map straightforwardly onto ratatui `Style` — no widget needed to get colored,
  bold, reversed output.

## What fought back

- **No off-the-shelf render widget.** We hand-roll the cell→`Buffer` blit, including
  grapheme/wide-cell handling. For the spike a 1-cell-per-column blit is fine, but a
  shipping renderer must honor `Flags::WIDE_CHAR` / `WIDE_CHAR_SPACER` to avoid
  double-printing wide glyphs — work `tui-term` did for free in S1.
- **Heavier dependency tree.** `alacritty_terminal` pulls `rustix-openpty`,
  `signal-hook`, `polling`, `cursor-icon`, and a stack of `windows-*` crates (24 new
  deps locked on first build) versus the thin `tui-term`. Confined to `delos/`, so
  no impact on the shipping graph, but a real cost if adopted.
- **`&self` event sink forces a channel.** `send_event(&self, …)` can't mutate, so
  any emulator→app feedback needs interior mutability (we used `mpsc`). Workable but
  an extra moving part compared to vt100's "no replies at all."
- Same **absolute-row caveat as S1**: `display_offset` is bottom-relative, so a
  selection can still drift if the child emits output mid-drag. Negative `Line`
  indices are *internally* stable anchors, but there's no monotonic absolute row
  counter surfaced for app-level use either. (Carry-forward question into S3.)

## Damage / dirty tracking (scorecard #8 — the S1 → S2 step change)

`alacritty_terminal` **does** expose damage tracking: `term.damage() -> TermDamage`
yields changed line spans, and `term.reset_damage()` clears them after a frame. This
is the clearest capability gap over `vt100` (which has no public damage API and
forces a full redraw). The spike renders full-frame for simplicity, but the API is
there for partial-redraw optimization — a real plus for a shipping grid.

## Scorecard notes (S2 column)

- Render correctness: **Good** — explicit per-cell SGR (`fg`/`bg`/`flags`) maps onto
  ratatui `Style`; we blit the grid ourselves.
- Grapheme segmentation: **OK** — cells store the grapheme + `WIDE_CHAR` flags, but
  *we* must honor them; no widget does it for us (S1 got this free via `tui-term`).
- Scrollback API: **Good** — `scroll_display(Scroll::{Delta,PageUp,PageDown,Top,
  Bottom})` + `display_offset()`; signed `Line` indices give stable scrollback
  addressing, richer than vt100's bottom-only offset (still no app-level absolute id).
- Selection primitives: **Good** — native `bounds_to_string`/`selection_to_string`
  exist; we instead reuse the shared `coords` bridge + a `viewport_matrix` for
  render/copy consistency.
- Mouse/alt-screen ergonomics: **Good** — `TermMode` bitflags expose `ALT_SCREEN`
  and granular mouse-report modes.
- Hyperlinks (OSC 8): **OK** — `Cell` can carry hyperlink data (`cell.hyperlink()`),
  unlike vt100's none; not wired in the spike.
- Images: **None** — no Sixel/Kitty/iTerm image support.
- Damage/dirty tracking: **Good** — `term.damage()` / `reset_damage()` give real
  dirty-span tracking (the standout gain over S1).
- API ergonomics: **OK** — capable but larger surface: `Processor`, `Handler`,
  `EventListener`, `Dimensions`, signed `Line` indexing, channel for write-back.
- Build/dep weight: **Weak** — 24 transitive deps (rustix-openpty, signal-hook,
  polling, windows-\*) vs S1's thin `tui-term`.
- Maintenance health: **Good** — actively developed (powers Alacritty); 0.26 current.
- `/save` reconstruction: **Good** — grid + signed scrollback lines + damage give a
  solid basis for serialization, better-anchored than S1's bottom-relative text.

## Validation status

- Automated: `cargo fmt --check` / `cargo clippy --all-targets -D warnings` /
  `cargo test` all green (8 `selection` unit tests; the slice itself compiled clean
  on first build).
- Isolation: root `cargo tree | grep -E 'vt100|alacritty_terminal|wezterm-term|
  termwiz|tui-term'` is empty; root `cargo build` + `cargo test` still pass — spike
  deps confined to `delos/`.
- **Interactive feel (Constitution III):** requires a human at a real terminal —
  run `cargo run -p spike-alacritty` inside `delos/`, exercise drag-select across
  the scrollback boundary, right-click copy, `--clipboard=arboard`, and a
  full-screen TUI child (e.g. `vim`) to confirm alt-screen handover and that
  terminal-probe replies (cursor reports) flow back. Record subjective notes here.

## Manual validation results

Maps to the acceptance scenarios and success criteria.

1. **Render (SC-001 / FR-007)**: Good
2. **Selection + content coords (SC-004 / FR-008/009)**: Fair
3. **Auto-scroll on drag-past-edge (FR-010)**: Good
4. **Copy (SC-008 / FR-011/016)**: Good
5. **SIGINT vs copy (SC-008 / FR-015)**: Good
6. **Shift bypass (FR-017)**: Good
7. **Wheel scroll (FR-012)**: Good
8. **Alt-screen handover (SC-005 / FR-013)**: Good
9. **Child mouse mode (FR-014)**: Not tested
10. **Flood (FR-022 #8)**: Fair

### Notes

Due to our hyperlink test, I couldn't see the "usable prompt" once the prompt
was at the bottom of the screen. Doing `echo '---...'` with enough dashes
worked, prompt still there, hidden by hyperlink test.

- (2): `seq 10`, select a couple numbers, `seq 5` selection shifts (if window scrolls)
- (9): I don't know an app that I can use for test
- (10): Responsive, same selection drift as (2)
