# Contract: Keymap Actions (new named, rebindable bindings)

**Feature**: 007-laat-mode | **Phase**: 1 | Internal interface contract

Covers the two new named actions registered in the sprint-006 keymap engine.
Realizes FR-015/FR-016, FR-018, FR-029. Extends `src/action/mod.rs`.

## 1. New actions

| `Action` variant | `name()` | Default `KeySpec` | Behavior |
|------------------|----------|-------------------|----------|
| `ToggleMultLaat` | `toggle_mult_laat` | `Single(Ctrl+1)` | `Norm→Mult` (even empty/1-line); toggles `Mult↔Laat` when multi-line. |
| `PushInput` | `push_input` | `Single(Ctrl+Alt+Enter)` | Push the input buffer+mode (one-item stack). |

Both follow the 006 pattern: added to `Action::name`, `Action::from_name`, the
`default_map`, `App::dispatch_action`, and `docs/keymap-defaults.toml`.

## 2. Key-string parsing — no change needed

The 006 key-string parser (`src/action/mod.rs::parse_key`) already accepts both
default bindings:
- `Ctrl+1` — `+`-split modifiers; the `1` falls through to the single-char
  case → `KeyCode::Char('1')`.
- `Ctrl+Alt+Enter` — `enter` is in the key-name table → `KeyCode::Enter`.

No new tokens or parser branches are required (verified against `parse_key`).

## 3. Resolution & dispatch

`App::on_key` resolves the chord against the effective keymap (as today) and calls
`dispatch_action`. Two new arms:
- `Action::ToggleMultLaat` → `App::toggle_mult_laat()` (mode transition per
  [input-modes.md](input-modes.md) §1).
- `Action::PushInput` → `App::push_input()` (per [push-pop-stack.md](push-pop-stack.md) §2).

Because `ToggleMultLaat`/`PushInput` change interaction *mode/state* rather than
editing text, they are dispatched through the keymap like the other named actions;
no special-casing ahead of `resolve` is needed (unlike Up/Down, which stay
special-cased because their meaning is mode-dependent — see input-modes §2).

## 4. Config & discoverability

- `/keys` lists `toggle_mult_laat` and `push_input` with their effective bindings
  (FR-029), via the existing `Keymap::listing()`.
- `docs/keymap-defaults.toml` gains both actions' default bindings, kept in sync
  with `Keymap::default_map()` by the existing `tests/keymap_defaults_doc.rs` test
  (it will fail until the doc is updated — the intended TDD signal).
- Per-mode `[keymap.laat]`/`[keymap.mult]` config sections remain **out of scope**;
  these two actions are global, rebindable via the existing top-level `[keymap]`
  surface.

## 5. Behavioral contract (testable)

- K1: `Keymap::default_map()` resolves `Ctrl+1` → `ToggleMultLaat` and
  `Ctrl+Alt+Enter` → `PushInput`.
- K2: `Action::from_name("toggle_mult_laat") == Some(ToggleMultLaat)` and likewise
  for `push_input`; round-trips with `name()`.
- K3: `parse_key`/`KeySpec` parsing of `"Ctrl+1"` and `"Ctrl+Alt+Enter"`
  succeeds and matches the default bindings.
- K4: `docs/keymap-defaults.toml` builds a `Keymap` equal to `default_map()`
  (existing sync test, now including the two new actions).
- K5: `/keys` listing includes both action names.

All of K1–K5 are unit-testable in the existing `tests/keymap_*` suites.
