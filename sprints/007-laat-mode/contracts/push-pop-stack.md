# Contract: Push/Pop Input Stack

**Feature**: 007-laat-mode | **Phase**: 1 | Internal interface contract

Covers the one-item push/pop input stack. Realizes FR-018…FR-020, SC-006.

## 1. State

`App` holds a single `Option<InputSnapshot>` (see [data-model.md](../data-model.md)):

```text
InputSnapshot {
    buffer: String,
    cursor: usize,
    mode: InputMode,
    stash: Option<String>,          // chat-style stashed draft (FR-011)
    laat: Option<LaatState>,        // present when mode == Laat
}
```

## 2. Push (`PushInput`, default `Ctrl+Alt+Enter`)

```mermaid
sequenceDiagram
    participant U as User
    participant A as App
    U->>A: Ctrl+Alt+Enter (PushInput)
    alt slot empty
        A->>A: snapshot = (buffer, cursor, mode, stash, laat)
        A->>A: clear pad; mode = Norm
    else slot occupied
        A->>A: no-op (one-item semantics, FR-020)
    end
```

- Saves the current buffer, caret, mode, stash, and LAAT state.
- Resets the input pad to empty and sets `mode = Norm` for ad-hoc input.
- **One-item semantics**: a push while the slot is occupied is a **no-op** — the
  first saved state is never overwritten or dropped (SC-006). (FR-020)

## 3. Pop (automatic, on the next `submit`)

```mermaid
sequenceDiagram
    participant U as User
    participant A as App
    U->>A: submit ad-hoc command
    A->>A: route + run the command
    alt snapshot present
        A->>A: restore buffer, cursor, mode, stash, laat; clear slot
    end
```

- The pop happens **after** routing/running the submitted ad-hoc command, on the
  next `submit` following a push (FR-019). **Any** submitted line pops — a shell
  command or a slash command alike; to keep the pushed state longer the user
  re-pushes (and re-stashes) after the ad-hoc submission.
- Restores the buffer, caret, mode, the stashed draft (FR-011), and the LAAT
  stepping state if the saved mode was `Laat`.
- The slot is cleared after the pop (back to a single-item-empty stack).

## 4. Interaction with other features

- A pushed `Laat` state pops back into `Laat` with its `highlight`/`failed_lines`
  intact (the user resumes stepping exactly where they left off).
- The stashed draft survives the round-trip (FR-011) because it lives in the
  snapshot, satisfying SC-003 across a push/pop.
- `/save`'s pending overwrite prompt and push/pop are independent; a push does not
  occur while a prompt is pending (the prompt consumes keys first, R9).

## 5. Behavioral contract (testable)

- P1: In `Mult` with `"a\nb"`, push ⇒ pad empty, mode `Norm`, snapshot holds
  `"a\nb"`+`Mult` (FR-018).
- P2: After P1, submit `"ls"` ⇒ `ls` runs, then pad restores `"a\nb"`, mode `Mult`
  (FR-019).
- P3: After P1, a second push ⇒ no-op; snapshot still holds the first state
  (FR-020, SC-006).
- P4: Push a `Laat` buffer mid-failure, pop ⇒ mode `Laat`, `highlight` and the
  failure flag restored.
- P5: Push a `Mult` buffer that has a stashed draft, pop ⇒ stash restored (FR-011).

P1–P5 are unit-testable against the snapshot save/restore logic; the keymap binding
of `Ctrl+Alt+Enter` and the live round-trip use the Constitution III manual
exception (quickstart).
