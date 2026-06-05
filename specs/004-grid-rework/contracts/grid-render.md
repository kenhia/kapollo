# Contract — Grid Render (internal interface)

The grid render layer turns the emulated screen (`wezterm-term`) into ratatui styled spans
for the transcript pane, with correct scrollback windowing and alt-screen switching. Covers
FR-001…FR-006, FR-021…FR-024, and the no-flicker success criteria SC-001/SC-003.

## Surface

```text
Grid:
    advance_bytes(bytes: &[u8])                     // feed PTY output
    resize(rows: u16, cols: u16)                    // PTY winsize change
    viewport_rows(scroll_offset: usize) -> impl Iterator<Item = Row>
    stable_row_at(viewport_row: u16) -> StableRowIndex
    changed_rows() -> Range<StableRowIndex>         // damage since last render
    is_alt_screen_active() -> bool
    cursor() -> (u16, u16)

render:
    rows_to_lines(rows, selection: Option<&Selection>) -> Vec<ratatui::Line>
    // maps Cell fg/bg/attrs -> ratatui Style; overlays selection highlight
```

## Guarantees

1. **Style fidelity** (FR-002): colors, bold/italic/underline/reverse map to ratatui
   `Style`. Wide (CJK/emoji) cells occupy 2 columns; continuation cells are skipped.
2. **In-place updates** (FR-003, SC-001): a carriage-return progress line updates one grid
   row, not many — guaranteed by feeding raw bytes to the emulator (it owns `\r`/cursor
   moves), not by line-appending.
3. **Incremental redraw** (SC-003): only `changed_rows()` need repaint; full repaint only on
   resize/scroll. No flicker, no dropped frames under flood.
4. **Scrollback windowing** (FR-004, FR-021/022): `viewport_rows(scroll_offset)` yields the
   window starting `scroll_offset` rows above the live bottom, clamped to
   `[0, scrollback.len()]`. New output while scrolled-back does not move the window unless
   the user is at bottom (follow-tail).
5. **Alt-screen switch** (FR-005, FR-023): when `is_alt_screen_active()`, render the alt
   buffer; on exit, the prior main-screen viewport + scrollback are intact (engine-owned).
6. **Cursor**: rendered from `cursor()`; never drawn off-viewport.

## Test obligations

- Feeding `"a\rb"` leaves a single row reading `b…` (in-place CR), not two rows.
- A styled SGR sequence produces the matching ratatui `Modifier`/`Color` on the span.
- A wide char advances the column by 2; the continuation cell renders empty.
- `viewport_rows(n)` for `n` within bounds returns the correct historical window; `n` past
  the top clamps to the oldest retained row.
- Entering then leaving alt-screen restores the exact pre-alt main viewport.
- `changed_rows()` after a single-row update reports a 1-row range.

## Notes

- The engine owns the main escape parse; `output/parser.rs` is reduced to the OSC 133/7 +
  mode side-tap (R5/R7), feeding marks to the block store and routing — it no longer applies
  SGR/cursor itself.
- Rendering reads stable-row ids so the selection overlay aligns with anchored selections
  even as new output arrives (R6).
