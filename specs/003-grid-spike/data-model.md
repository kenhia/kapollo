# Phase 1 Data Model: Terminal-Grid Spike

**Feature**: 003-grid-spike | **Date**: 2026-06-01

The spike produces no persistent application data; the "entities" here are the
in-memory model objects of each slice plus the deliverable artifacts. They are
documented so all three stages implement the *same* shape (apples-to-apples).

---

## Entities

### Stage

One throwaway binary crate evaluating one candidate.

| Field | Type | Notes |
|-------|------|-------|
| id | enum `S1 \| S2 \| S3` | ordering of evaluation |
| crate_name | string | `vt100` / `alacritty_terminal` / `wezterm-term` |
| crate_version | string | pinned version or git `rev` (R1) |
| binary | string | `spike-vt100` / `spike-alacritty` / `spike-wezterm` |
| writeup | path | `delos/docs/s{1,2,3}-*.md` |

### VerticalSlice (runtime composition, identical per stage)

| Component | Responsibility |
|-----------|----------------|
| PtyShell | spawn shell, pump bytes both directions, propagate resize (`spike-support`) |
| GridModel | the crate-specific parser/screen + scrollback (the thing under test) |
| Renderer | map grid cells → ratatui styled spans, main screen only |
| ModeDetector | scan child output for alt-screen + mouse-mode escapes (`spike-support`, shared) |
| SelectionController | owns Selection state, mouse routing, auto-scroll, copy |
| Clipboard | OSC 52 emit (default) + optional `arboard` fallback (`spike-support`) |

### Selection

| Field | Type | Notes |
|-------|------|-------|
| state | enum (see state machine) | drives copy-trigger & right-click behavior |
| anchor | `(abs_row: usize, col: u16)` | content coordinates (R5) |
| active_end | `(abs_row: usize, col: u16)` | content coordinates |
| viewport_top | `usize` | first visible absolute row; mutated by scroll/auto-scroll |
| region | `Rect` | output-region bounds; selection clamped to it (FR-009) |

### Viewport / Scrollback

| Field | Type | Notes |
|-------|------|-------|
| history_len | usize | total retained rows (crate-dependent cap) |
| top_row | usize | window start over history; `screen_y → abs_row = top_row + screen_y` |
| height | u16 | visible rows of the output region |

### Scorecard

The shared rubric (one column per stage). Schema in
[contracts/scorecard.md](contracts/scorecard.md). Twelve weighted criteria; each
cell is a short rating + note.

### CrateRecommendation

| Field | Type | Notes |
|-------|------|-------|
| chosen_crate | string | exactly one (SC-003) |
| rationale | prose | tied to weighted rubric evidence |
| selection_feasible | bool + note | SC-004 confirmation/refutation |
| altscreen_feasible | bool + note | SC-005 confirmation/refutation |
| feeds | list | D25–D30 + rework spec |

---

## Selection state machine

The active/inactive distinction is what disambiguates the Ctrl-C/right-click copy
rules (FR-015/FR-016) and the SIGINT conflict.

```text
        ┌────────────────────────────────────────────────────────────┐
        │                                                            │
        ▼                                                            │
   ┌─────────┐  left-press (no Shift)   ┌──────────┐  release  ┌──────────┐
   │  Idle   │ ───────────────────────▶ │ Dragging │ ────────▶ │  Active  │
   └─────────┘                          └──────────┘           └──────────┘
     ▲   │                                  │  │                  │   │
     │   │ left-press + Shift               │  │ drag past edge   │   │ left-press (2nd) / ESC
     │   │ → forward to child               │  │ → auto-scroll,   │   │ → cancel ──┐
     │   │                                  │  │   extend end     │   │            │
     │   │ right-press (no selection)       │  └──────────────────┘   │ right-press / Ctrl-C
     │   │ → "Hello, World." menu           │                         │ → copy, then ─┐
     │   └──────────────────────────────────┘                         │               │
     └────────────────────────────────────────────────────────────────┴───────────────┘
                                   (return to Idle)
```

**Transitions**
- `Idle + left-press (no Shift)` → `Dragging` (set anchor = active_end = press cell).
- `Idle + left-press (Shift held)` → forward mouse to child (FR-017); stay `Idle`.
- `Idle + right-press` → open trivial "Hello, World." context menu (FR-019); stay `Idle`.
- `Idle + Ctrl-C` → **SIGINT to child** (FR-015, preserves kapollo's 002 FR-024).
- `Dragging + mouse-move` → update active_end; if past top/bottom edge, auto-scroll
  and clamp (FR-010, R5).
- `Dragging + release` → `Active` (selection finalized but live; **no copy**, FR-011).
- `Active + right-press` OR `Active + Ctrl-C` → **copy** selection to clipboard,
  then deselect → `Idle` (FR-016).
- `Active + second left-press` OR `Active + ESC` → cancel → `Idle` (FR-018).
- Any state, child in alt-screen or child-mouse-mode → selection suspended; mouse
  routed to child (FR-013, FR-014).

---

## Validation rules (the unit-tested core)

These pure functions in `spike-support` carry the only automated tests (TDD):

1. **`screen_to_content(top_row, screen_y) -> abs_row`** and inverse; clamp to region.
2. **`auto_scroll(top_row, drag_y, height, history_len) -> top_row'`** — up when
   `drag_y < 0`, down when `drag_y >= height`, clamped at `[0, history_len-height]`.
3. **`normalize(anchor, end) -> (start, end)`** — document order for copy.
4. **`detect_mode(byte_stream) -> [ModeEvent]`** — alt-screen enter/exit and child
   mouse-mode enable/disable from DEC private mode sequences (R4).
5. **`osc52_frame(bytes) -> String`** — `ESC ] 52 ; c ; base64 ST` framing (R3).
