# Contract: Scrollback (page / line scroll, top / bottom) & key retarget

**Feature**: 005-input-and-status | Internal interface contract
**Modules**: `src/session/mod.rs` (`Transcript`), `src/app.rs` (key bindings)

Covers US3 (FR-013–FR-017). See [research.md](../research.md) R7.

---

## Transcript scroll API (`src/session::Transcript`)

```rust
/// Page up/down by one viewport MINUS the context lines, floored to ≥ 1 line,
/// clamped to [0, max_scroll] (FR-013/FR-014).
fn scroll_page_up(&mut self, context_lines: u16);
fn scroll_page_down(&mut self, context_lines: u16);

/// Exactly one line (FR-015) — existing scroll_up(1)/scroll_down(1), exposed as
/// the named actions scroll_line_up / scroll_line_down.
fn scroll_line_up(&mut self);
fn scroll_line_down(&mut self);

/// Jump to oldest / newest (FR-016) — existing scroll_to_top / scroll_to_bottom.
```

**Contract** (page advance amount):

```text
advance = (viewport.get().max(1) as usize).saturating_sub(context_lines as usize).max(1)
scroll_page_up   : offset = (offset + advance).min(max_scroll)
scroll_page_down : offset = offset.saturating_sub(advance)
```

- Default `context_lines = 3` (config `scroll.context_lines`, FR-014).
- Page advance is **always ≥ 1 line**, even when `viewport - context ≤ 0` on a short
  pad (FR-014) — this is the key behavioral guarantee.
- Clamping to `[0, max_scroll]` matches existing 004 behavior (no over-scroll).

## Key binding retarget (`src/app.rs` `on_key`)

| Key | Before (004) | After (005) | Action | FR |
|-----|--------------|-------------|--------|----|
| `Home` | scroll to top | **input line start** | `LineMoveStart` | FR-001/FR-017 |
| `End` | scroll to bottom | **input line end** | `LineMoveEnd` | FR-001/FR-017 |
| `Shift+Home` | — | scroll to top | `ScrollToTop` | FR-016 |
| `Shift+End` | — | scroll to bottom | `ScrollToBottom` | FR-016 |
| `PageUp` | full page | page **minus context** | `ScrollPageUp` | FR-013 |
| `PageDown` | full page | page **minus context** | `ScrollPageDown` | FR-013 |
| `Shift+PageUp` | — | one line | `ScrollLineUp` | FR-015 |
| `Shift+PageDown` | — | one line | `ScrollLineDown` | FR-015 |

**Contract**: `Home`/`End` MUST NOT scroll the transcript after 005 (FR-017); their
former jobs are reachable only via `Shift+Home`/`Shift+End`. No other 004 scroll
behavior regresses (FR-032).

**Test seam**: `tests/scrollback_context.rs` — drives `scroll_page_up/down` across
viewport/context combinations including the short-pad ≥ 1-line floor, plus line
scroll and top/bottom; binding retarget verified in the manual quickstart.
