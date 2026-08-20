# Contract: Key-string grammar (parse / format)

The human-writable binding grammar and its parser. Pure logic in `src/action`.
Tested by `tests/keymap_parse.rs`.

## Grammar

```text
key-spec   := single | chord
chord      := single WS single            ; only "Esc Esc" is valid this sprint
single     := { modifier "+" } key
modifier   := "ctrl" | "alt" | "shift"    ; case-insensitive
key        := named | printable
named      := "left" | "right" | "up" | "down" | "home" | "end"
            | "pageup" | "pagedown" | "enter" | "esc" | "tab"
            | "backspace" | "delete" | "insert" | "space"
printable  := any single Unicode scalar that is not WS or "+"
```

- **Case-insensitive** (FR-007): tokens are lowercased before matching, so
  `Ctrl+Left`, `ctrl+left`, and `CTRL+LEFT` are equal.
- **Modifier order is irrelevant** (FR-007): `Shift+Ctrl+Left` == `Ctrl+Shift+Left`.
- **Canonical short modifier names only** (R1): `ctrl`/`alt`/`shift`. `control`,
  `super`, `cmd`, `meta` are **not** accepted (parse error).
- **Chords** (FR-008): exactly two whitespace-separated single keys, and only the
  `Esc Esc` pair; any other multi-key sequence is a parse error.

## API

```rust
pub enum KeySpec { Single(KeyChord), Chord(KeyChord, KeyChord) }

impl KeySpec {
    pub fn parse(s: &str) -> Result<KeySpec, KeyParseError>;
    pub fn display(self) -> String;   // canonical; the inverse of parse
}

pub struct KeyParseError { /* offending input + reason */ }
```

## Parse rules

| Input | Result |
|-------|--------|
| `Ctrl+Left` | `Single(KeyChord{ Left, CONTROL })` |
| `ctrl+left` | identical chord (case-insensitive) |
| `Shift+Ctrl+Left` | `Single(KeyChord{ Left, SHIFT\|CONTROL })` (order-free) |
| `Alt+Enter` | `Single(KeyChord{ Enter, ALT })` |
| `a` | `Single(KeyChord{ Char('a'), NONE })` |
| `Esc Esc` | `Chord(Esc/NONE, Esc/NONE)` |
| `Control+Left` | **error** (unknown modifier — short names only) |
| `Ctrl+Nope` | **error** (unknown key) |
| `` (empty) | **error** (empty) |
| `Esc Esc Esc` / `Ctrl+a Ctrl+b` | **error** (unsupported chord) |

## Format rules

- `display()` emits modifiers in canonical order `Ctrl+`, `Alt+`, `Shift+` then the
  key name (matching the existing `KeyChord::display()`), and joins chord halves
  with a single space.
- For every valid spec, `parse(display(spec)) == spec` (round-trip).

## Test obligations (`tests/keymap_parse.rs`)

1. `case_insensitive_modifiers_and_keys_resolve_equal`.
2. `modifier_order_is_irrelevant`.
3. `short_modifier_names_only_long_forms_reject` (`Control+Left` errors).
4. `unknown_key_name_is_rejected`.
5. `empty_string_is_rejected`.
6. `esc_esc_parses_as_a_chord`.
7. `multi_key_sequences_other_than_esc_esc_reject`.
8. `display_round_trips_through_parse` for a representative set.
