# Contract — Mouse, Selection & Clipboard (internal interface)

Mouse routing, the selection FSM, and clipboard hand-off. Covers FR-007…FR-017, FR-026, and
success criteria SC-004…SC-008.

## Surface

```text
ModeTap (output side-tap):
    detect_mode(bytes: &[u8]) -> ModeDelta   // ?1049, ?1000/1002/1003/1006 h/l
    alt_screen_active() -> bool
    child_mouse_enabled() -> bool

SelectionController:
    on_mouse(ev: MouseEvent, ctx: RoutingCtx) -> Routed
    on_command_submit()                       // FR-017: clear active selection
    on_key(ev: KeyEvent) -> Routed            // Esc clears; copy hotkey copies
    selection() -> Option<&Selection>

Routed = ToChild(bytes) | Consumed | Bypass   // Bypass = host terminal native (Shift)

Clipboard:
    copy(text: &str) -> CopyResult            // OSC 52 then arboard, per config order
```

## Routing rules (RoutingCtx = alt_screen, child_mouse, shift)

| Condition | Wheel | Click/Drag | Rationale |
|-----------|-------|-----------|-----------|
| `shift` held | Bypass | Bypass | Host-terminal native selection (FR-016) |
| `alt_screen` or `child_mouse` | ToChild | ToChild | App owns the mouse (FR-014, SC-005) |
| otherwise | Consumed → scroll grid | Consumed → selection FSM | kapollo owns (FR-007, FR-009) |

## Selection FSM

```text
Idle ─MouseDown(owned,!shift)─▶ Dragging ─MouseDrag─▶ Dragging ─MouseUp─▶ Active
Active ─copy hotkey / on release(config)─▶ copy(text)            (FR-011/012)
Active ─on_command_submit / new MouseDown / Esc─▶ Idle (clear)   (FR-017)
* ─alt_screen|child_mouse ON─▶ suspend (no selection state)      (FR-014)
```

- Anchors are `(StableRowIndex, col)` → no drift under new output (FR-008, R6).
- Coordinate math (`coords`) maps screen pixels ↔ content cells, accounting for wide cells
  and the current `scroll_offset`.

## Clipboard guarantees

1. **OSC 52 primary** (FR-010): base64-framed, terminal-mediated, SSH-friendly.
2. **arboard fallback** (FR-010): when OSC 52 unavailable/unhonored; order configurable.
3. **Visible failure** (FR-013): both failing → status notice, never a silent drop.
4. Copied text is exactly the selected cells joined with `\n` at row breaks (no off-by-one —
   the spike's S1 off-by-one is avoided by stable-row anchoring).

## Test obligations

- Routing table: each (shift, alt_screen, child_mouse) combo yields the expected `Routed`.
- `detect_mode` flips `child_mouse` on `?1000h` and back on `?1000l`; `alt_screen` on
  `?1049h/l`.
- FSM: down→drag→up produces an `Active` selection with the dragged range; `on_command_submit`
  clears it (FR-017 — the flood-overrun fix).
- `osc52_frame(text)` round-trips base64; `copy` tries primary then fallback per `order`;
  both-fail returns an error the app renders as a notice.
- Selected text join has no trailing/leading off-by-one across a multi-row selection.

## Notes

- `SelectionController`, `coords`, and `Clipboard` are promoted from the 003 `spike-support`
  + `selection.rs` (portable verbatim across spikes), now anchored to real `StableRowIndex`.
- The mode side-tap is the **same** detector that feeds OSC 133/7 block marks (R5/R7) — one
  place to route.
