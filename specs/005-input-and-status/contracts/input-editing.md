# Contract: Input Editing (motion / selection / kill) & Named Actions

**Feature**: 005-input-and-status | Internal interface contract (not a network API)
**Modules**: `src/input` (`mod.rs`, `editing.rs`, `selection.rs`), `src/action`

These are the pure-logic seams TDD targets first (Constitution III). All operate on
the **current line** of the Input Buffer (FR-007). Signatures are indicative.

---

## 1. Word-boundary scanners (pure, `src/input/editing.rs`)

```rust
/// Punctuation-aware word motion (FR-002). Classes: whitespace / word
/// (alphanumeric + '_') / punctuation. A boundary sits between adjacent runs
/// of different class.
fn word_boundary_left(line: &[char], from: usize) -> usize;
fn word_boundary_right(line: &[char], from: usize) -> usize;

/// Readline whitespace rule (FR-006): consume preceding whitespace, then the
/// preceding non-whitespace run (punctuation included). Returns the start index
/// of the span to delete (end is `from`).
fn delete_word_before_start(line: &[char], from: usize) -> usize;
```

**Contract / cases** (must be covered):
- `word_move_left` from mid-word → start of that word; from word start → start of
  previous word; across `foo.bar` → stops at the `.` boundary (punctuation-aware).
- `word_move_right` symmetric to the right.
- Motion **clamps to the current line** — never crosses a `\n` (FR-007).
- `delete_word_before` on `"ls -la │"` (caret at end, leading spaces) deletes `-la`
  then a second invoke deletes `ls`; on `"foo.bar│"` deletes `foo.bar` as one run
  (whitespace rule, punctuation not a boundary). Empty/at-line-start → no-op.

## 2. Current-line ops (`src/input/mod.rs`, methods on `InputPad`)

```rust
fn line_move_start(&mut self);   // FR-001 → current_line_bounds().0
fn line_move_end(&mut self);     // FR-001 → current_line_bounds().1
fn word_move_left(&mut self);    // FR-002
fn word_move_right(&mut self);   // FR-002
fn kill_to_line_start(&mut self);// FR-005 — delete [line_start, cursor)
fn kill_to_line_end(&mut self);  // FR-005 — delete [cursor, line_end)
fn delete_word_before(&mut self);// FR-006
fn insert_paste(&mut self, text: &str); // FR-010/FR-012 (see input-paste below)
```

**Contract**: each behaves **identically** in single-line and multi-line buffers,
scoped to the caret's current line (FR-007). `kill_to_line_start`/`kill_to_line_end`
delete only within the current line (do not consume the line break). After any op,
the caret invariant `0 <= cursor <= char_count` holds.

## 3. Input selection (`src/input/selection.rs`)

```rust
struct InputSelection { anchor: usize, caret: usize }
impl InputSelection { fn range(&self) -> (usize, usize); fn is_empty(&self) -> bool; }

// On InputPad:
fn select_char_left(&mut self);   // FR-003
fn select_char_right(&mut self);  // FR-003
fn select_word_left(&mut self);   // FR-004
fn select_word_right(&mut self);  // FR-004
fn cancel_selection(&mut self);   // → None (FR-029)
fn selected_text(&self) -> Option<String>;
```

**Contract**: first `select_*` with no selection anchors at the current caret; each
extends `caret` (and mirrors `cursor`). `range()` is normalized (`min,max`).
Word-wise selection uses the same punctuation-aware boundaries as motion (FR-004).

## 4. Named-action registry (`src/action`)

```rust
enum Action { LineMoveStart, LineMoveEnd, WordMoveLeft, /* … */,
              ClearStatusMessage, /* named for /keys, but a contextual Esc Esc gesture — not a KeyChord */
              MultilineMoveStartBuffer, MultilineMoveEndBuffer /* reserved, unmapped */ }

struct KeyChord { code: KeyCode, mods: KeyModifiers }

/// Hardcoded default bindings (mapped actions only; FR-008).
fn default_bindings() -> &'static [(Action, KeyChord, &'static str /* name */)];
fn resolve(chord: KeyChord) -> Option<Action>;
fn listing() -> Vec<(/*name*/ String, /*keys*/ String)>; // for /keys (FR-030)
```

**Contract**:
- Every action in the data-model §3 **Mapped actions** table appears **exactly once**
  in `default_bindings()`; no two share a chord.
- `MultilineMoveStartBuffer` / `MultilineMoveEndBuffer` exist as `Action` variants
  but have **no** entry in `default_bindings()` and `resolve` never returns them
  (FR-009).
- `ClearStatusMessage` is a named `Action` (listed by `/keys`) but is **not** a
  `KeyChord`: `Esc Esc` is a contextual two-key gesture arbitrated in `on_key`, so it
  has **no** `default_bindings()` entry and `resolve` never returns it (FR-026).
- `resolve(chord)` returns the bound action for a chord that maps; `None` otherwise.
- `listing()` includes every mapped action (plus `ClearStatusMessage`) by stable name
  and human-readable chord/gesture (drives `/keys`; FR-030) and is **stable-ordered**.

**Test seam**: `tests/input_editing.rs` (motion/kill/selection),
`tests/input_selection.rs` (selection + arbitration), and a registry test asserting
the one-binding-per-mapped-action invariant and that the reserved actions and
`ClearStatusMessage` are absent from `default_bindings()`/`resolve`.
