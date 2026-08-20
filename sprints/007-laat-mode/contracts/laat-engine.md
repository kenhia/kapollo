# Contract: LAAT Engine (highlight, step, exit-code gating)

**Feature**: 007-laat-mode | **Phase**: 1 | Internal interface contract

Covers LAAT stepping and exit-code gating. Realizes FR-001…FR-007.

## 1. State

While `mode == Laat`, `App` holds `LaatState { highlight, failed_lines, pending }`
(see [data-model.md](../data-model.md)):
- `highlight: usize` — the currently highlighted buffer line (0-based).
- `failed_lines` — the set of lines flagged as **probable** failures.
- `pending: Option<usize>` — the line last submitted, awaiting completion.

On entry to `Laat`, `highlight = 0`, `failed_lines` empty, `pending = None`.

## 2. Navigation

`Up`/`Down` move the caret between lines (per input-modes contract) and the
highlight tracks the caret's line. `Shift+Arrow` selection is unchanged and is for
combined submission, not stepping (FR-017).

## 3. Submission & gating

```mermaid
sequenceDiagram
    participant U as User
    participant A as App (Laat)
    participant S as Shell
    U->>A: Enter (highlight = line i)
    A->>S: submit line i (normal single-line submission)
    A->>A: pending = Some(i)
    S-->>A: CommandEnd { exit_code }
    alt exit == 0
        A->>A: highlight = i+1; failed_lines -= i; pending = None
    else exit != 0
        A->>A: failed_lines += i; highlight stays = i; pending = None
    end
```

- `Enter` submits the highlighted line(s) as a **normal single-line submission**
  (FR-003/FR-005): it flows through the existing `submit` → `run_shell` path; its
  output goes to the transcript and `last_exit` updates exactly as a norm-mode
  command would.
- The advance/flag decision happens on the **existing** `CommandEnd { exit_code }`
  observation (the boundary side-tap that already sets `App.last_exit`), gated by
  `pending`. No second exit-tracking path is introduced (research R5).
- Submitting the **last** line and succeeding leaves `highlight` past the end; the
  buffer is complete (no wrap).

## 4. Failure recovery (FR-006)

On a probable failure (`failed_lines` contains `highlight`), the user may:
- press `Enter` to **re-run** the same line (e.g. after fixing the environment);
- press `Down` then `Enter` to **treat as success** and advance past it;
- press `Esc Esc` to **abort** the whole buffer (leaves `Laat`, clears the buffer);
- **push** the buffer (`Ctrl+Alt+Enter`), fix the issue with an ad-hoc command,
  **pop** on the next submit, and continue.

## 5. Rendering

`ui::input_pad::render` (when `mode == Laat`) draws:
- the highlighted line with a highlight **background** style;
- any line in `failed_lines` with a distinct **probable-failure background**.

Rendering reads `LaatState`; it adds no new state. Color honors the existing
`color_enabled()` gate (no color ⇒ a non-color affordance is acceptable; the
highlight position is still conveyed by the caret line).

## 6. Behavioral contract (testable)

- L1: Enter `Laat` on a 3-line buffer ⇒ `highlight == 0`, status mode `1T`.
- L2: Submit line 0, `CommandEnd { Some(0) }` ⇒ `highlight == 1`, `failed_lines`
  empty (FR-004).
- L3: Submit line 1, `CommandEnd { Some(7) }` ⇒ `highlight == 1`, `failed_lines`
  contains 1 (FR-004).
- L4: After L3, `Down` then `Enter` on line 2, `CommandEnd { Some(0) }` ⇒
  `highlight == 3` (past end), line-1 flag retained until cleared (FR-006).
- L5: After L3, `Enter` re-submits line 1; `CommandEnd { Some(0) }` ⇒
  `highlight == 2`, line-1 flag cleared (FR-006/FR-004).
- L6: `Esc Esc` in `Laat` ⇒ mode `Norm`, LAAT buffer cleared (FR-007).

The `LaatState` transition function (apply an exit code given `pending`) is pure
and unit-tested for L2–L5; the highlight/flag rendering uses the Constitution III
manual exception (quickstart).
