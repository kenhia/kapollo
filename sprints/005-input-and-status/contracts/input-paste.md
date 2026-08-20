# Contract: Bracketed Paste

**Feature**: 005-input-and-status | Internal interface contract
**Modules**: `src/lib.rs` (terminal setup/teardown), `src/app.rs` (event loop),
`src/input/mod.rs`

Covers US2 (FR-010–FR-012). See [research.md](../research.md) R1.

---

## Terminal setup / teardown (`src/lib.rs`)

```rust
// Setup (alongside existing raw mode + mouse capture + alt-screen):
execute!(out, EnableBracketedPaste)?;

// Teardown — MUST be on BOTH the normal exit path AND the panic guard
// (Constitution VI), alongside DisableMouseCapture / LeaveAlternateScreen /
// disable_raw_mode:
execute!(out, DisableBracketedPaste)?;
```

**Contract**: bracketed paste is enabled exactly when raw mode is, and disabled on
every exit path. After a panic, the terminal is **not** left in bracketed-paste mode
(integration/manual verification per Constitution III exception).

## Event handling (`src/app.rs` event loop)

```rust
match event::read()? {
    // … existing Key / Mouse / Resize arms …
    Event::Paste(text) => self.input.insert_paste(&text),
    _ => {}
}
```

**Contract**:
- A bracketed paste arrives as a single `Event::Paste(String)` and **never** reaches
  the `(Enter, _) => submit` key arm — no pasted line auto-submits (FR-011).
- `insert_paste` splits `text` on `\n` into buffer line boundaries (normalizing
  `\r\n`/`\r` → `\n`), inserting at the caret as **one** buffer (FR-010).
- After paste, the caret rests at the **end** of the inserted content (FR-012) and
  the buffer is fully editable by all US1 actions.
- `Enter` after a multi-line paste submits the **entire** buffer as one command
  (FR-011) — preserving multi-line submission semantics already used by
  `Shift+Enter`/`Alt+Enter`.

**Edge cases**:
- Empty paste (`""`) → no change.
- Paste with a trailing newline → trailing empty line preserved; caret at buffer end.
- Paste into a non-empty buffer mid-line → splices at the caret, lines around it
  re-joined correctly.

**Test seam**: `tests/input_paste.rs` exercises `insert_paste` directly (split,
caret-at-end, splice, edge cases); the live `Event::Paste` round-trip and the
no-auto-submit guarantee are verified in the manual quickstart (live TTY).
