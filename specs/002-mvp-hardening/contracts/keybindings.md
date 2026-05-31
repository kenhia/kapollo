# Contract: Transcript scrolling key bindings

Keyboard-only transcript scrolling for this sprint (FR-021, FR-022, D24). Mouse
capture is explicitly out of scope and deferred to a later opt-in (D24).

## Bindings (normal split-pad mode only)

| Key | Action | Maps to |
|-----|--------|---------|
| `PageUp` | Scroll the transcript **up** (toward older output) by one page | `Transcript::scroll_up(step)` |
| `PageDown` | Scroll the transcript **down** (toward newer output) by one page | `Transcript::scroll_down(step)` |
| `Home` | Jump to the **top** (oldest output) | set scroll offset to max |
| `End` | Jump to the **bottom** (newest output) | set scroll offset to `0` |

- **Page step**: one page = `viewport_height - 1` lines (overlap of one line for
  continuity), or a fixed sensible step; the exact value is an implementation
  detail but MUST produce visible movement (the user test reported PgUp/PgDn
  doing nothing — the offset MUST actually be applied and clamped correctly).
- **Scroll model**: `scroll_offset` counts lines **up from the newest output**;
  `0` = pinned to newest (bottom). `Home` sets the offset to the maximum
  (`total_lines - viewport`); `End` sets it to `0`.
- **Clamping**: scrolling MUST clamp at both ends (cannot scroll above the oldest
  line or below the newest); over-scroll is a no-op, not an error or wrap.
- **Auto-pin on submit**: submitting a command resets the offset to `0` (newest),
  preserving existing behavior.
- **Streaming while scrolled**: when the user has scrolled up and new output
  arrives, the view stays where the user put it (does not auto-jump), and the
  US1 render invariants hold (no stray characters, no chrome overwrite).

## Passthrough

These bindings apply only in normal mode. During alt-screen passthrough, all keys
(including PageUp/PageDown/Home/End) are forwarded verbatim to the program
(per the passthrough input contract, research R4).

## Out of scope (deferred — D24)

- Mouse-wheel scrolling and any mouse capture. Adding it later will be opt-in via
  config (`mouse = true`, default off) and must disable capture during passthrough.
