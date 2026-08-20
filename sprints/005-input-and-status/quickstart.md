# Quickstart: Input Editing & Fixed Status Bar (manual validation)

**Feature**: 005-input-and-status | **Date**: 2026-06-05

Live-TTY manual validation for the behaviors that cannot be fully unit-tested
(Constitution III integration/manual exception): paste round-trip, key feel, and
status-bar render/resize/hide. Each step maps to a Success Criterion in
[spec.md](spec.md). Run after the gate is green
(`cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings &&
cargo test`).

**Build & launch**:

```bash
cargo run --bin kap
```

Use a real terminal (not a pipe). A second pane running `tail -f` on the off-screen
log is handy for diagnosing without breaking the TUI.

---

## US1 — Line editing (SC-001, SC-004)

1. Type `echo one two three.four`.
2. `Home` → caret jumps to **start of line** (not scrollback top). `End` → end.
   ✅ FR-001/FR-017.
3. `Ctrl+Left` repeatedly → caret stops at each word start; over `three.four` it
   stops at the `.` (punctuation-aware). `Ctrl+Right` mirrors rightward. ✅ FR-002.
4. `Shift+Left`/`Shift+Right` → grows a character selection; `Shift+Ctrl+Left/Right`
   → grows a word selection. ✅ FR-003/FR-004.
5. Caret at end, `Ctrl+W` → deletes `four` (then `.`? per whitespace rule deletes the
   non-whitespace run `three.four` if no interior space — confirm: `Ctrl+W` removes
   the whole `three.four` run). `Ctrl+U` → deletes to line start; `Ctrl+K` (caret
   mid-line) → deletes to line end. ✅ FR-005/FR-006.
6. Insert a newline (`Shift+Enter`), type a second line, and repeat steps 2–5 on the
   second line — every action operates on the **current line** identically. ✅ FR-007,
   SC-001.

## US2 — Multi-line paste (SC-002)

7. Copy this 3-line block from elsewhere:

   ```text
   echo first
   echo second
   echo third
   ```

8. Paste into the input pad. It lands as **one** 3-line buffer; **nothing submits**.
   ✅ FR-010/FR-011, SC-002. Caret rests at the **end** of the pasted text. ✅ FR-012.
9. Edit a pasted line (e.g. `Ctrl+Left`, change a word) — fully editable. ✅ FR-012.
10. Press `Enter` → the **entire** buffer submits as one command. ✅ FR-011.

## US3 — Scrollback (SC-007)

11. Produce a long output (e.g. `seq 1 500`).
12. `PageUp` → moves up one page **minus 3 context lines** (overlap visible);
    `PageDown` mirrors. On a deliberately short window it still advances ≥ 1 line.
    ✅ FR-013/FR-014, SC-007.
13. `Shift+PageUp`/`Shift+PageDown` → move exactly **one line**. ✅ FR-015.
14. `Shift+Home` → jump to oldest output; `Shift+End` → newest. `Home`/`End` do
    **not** scroll (they edit the input line). ✅ FR-016/FR-017.

## US4 — Fixed status bar (SC-005)

15. Observe the status bar directly beneath the input pad in the fixed layout
    `mode | cwd<greedypad>| message | exit`: the 4-char mode field shows `norm`, cwd
    left, greedy pad, message right-justified, exit on the right (only when non-zero).
    ✅ FR-018–FR-020, FR-023, SC-005.
16. Run a failing command (e.g. `false`) → `exit` field shows the non-zero code; run a
    successful one → exit clears/zeros. ✅ FR-023.
17. Narrow the terminal width → `message` truncates first, then `cwd`
    (middle-ellipsis), `mode`/`exit` never break, bar never wraps. ✅ FR-024.
18. `/status` → toggles the bar off, again → on. ✅ FR-022, SC-005.
19. Shrink the terminal below 10 rows → bar auto-hides regardless of the toggle; grow
    back to ≥ 10 rows → it reappears. ✅ FR-021, SC-005.

## US5 — Message lifetime & selection arbitration (SC-003, SC-004, SC-006)

20. Trigger a status message (e.g. set `[clipboard] osc52=false` +
    `local_fallback=false`, select+copy → "copy failed" message). It **persists**
    across non-submitting actions (typing, motion) — **no** timeout expiry.
    ✅ FR-025, SC-006.
21. Press `Enter` (submit) → message clears. Trigger it again, then `Esc Esc` →
    message clears. ✅ FR-025/FR-026, SC-006.
22. Start a selection in the **input pad**, then start one in the **transcript** →
    the input selection clears (at most one across both pads). And vice-versa.
    ✅ FR-027, SC-003.
23. With a selection active, `Ctrl+C` → copies and clears the selection. With **no**
    selection, `Ctrl+C` → sends SIGINT to a running child (start `sleep 30` first).
    ✅ FR-028, SC-004.
24. `Esc` with a selection → cancels it. `Esc` on a single-line buffer (no selection)
    → clears the line. In a multi-line buffer, single `Esc` clears only the current
    line; `Esc Esc` clears the whole buffer. ✅ FR-029, SC-004.

## Discoverability (SC-008)

25. `/keys` → lists every active hardcoded binding by action and key. ✅ FR-030,
    SC-008.
26. `/help` → includes a one-line pointer to `/keys`. ✅ FR-031, SC-008.

## Regression (SC-009)

27. Confirm existing slash commands (`/help`, `/clear`, `/quit`), 004 mouse
    selection/copy, and shell-wrapping all still work. ✅ FR-032, SC-009.

---

## Pass criteria

All ✅ checks hold in 100% of trials for their mapped SC (SC-001…009). Any flicker,
reflow, wrap, lost output, or stranded terminal mode (raw / mouse / bracketed paste
after exit or crash) is a **Constitution VI failure** and blocks ship.
