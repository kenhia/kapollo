# Contract: Keymap engine (default map, resolution, conflicts)

The in-memory keymap that the event loop resolves against. Pure logic in
`src/action`. Tested by `tests/keymap_engine.rs`.

## Default map

`Keymap::default_map()` is the data-fied form of the sprint-005 `BINDINGS` table
plus the sprint-006 additions. It MUST reproduce current behavior exactly
(FR-002), and additionally:

- Bind `Action::InsertNewline` to `["Shift+Enter", "Alt+Enter"]` (primary +
  alternate, FR-004/R5).
- Bind `Action::CopyCurrentLine` and `Action::CopyBlockWithoutCommand` to default
  chords (FR-005/R4); proposed `Ctrl+Y` (copy current line) and `Alt+Y` (copy
  block without command), validated to not conflict with any existing default.
  Their keyboard bindings act on the **bottom-most transcript output** (the newest
  line / most recently completed block), since no mouse position is available.

The reserved/contextual actions (`ClearStatusMessage`,
`MultilineMoveStartBuffer`, `MultilineMoveEndBuffer`) remain **unbound** in the
default single-key resolution table; `ClearStatusMessage` is still surfaced in
`/keys` as the `Esc Esc` gesture (FR-014/FR-018).

## API

```rust
impl Keymap {
    pub fn default_map() -> Keymap;
    pub fn with_overrides(base: &Keymap, overrides: &[(Action, Binding)]) -> Keymap;
    pub fn resolve(&self, spec: KeySpec) -> Option<Action>;
    pub fn listing(&self) -> Vec<(String, String)>;   // (action name, key display)
}

struct Keymaps { default: Keymap, modes: BTreeMap<String, Keymap> }
impl Keymaps {
    pub fn for_mode(&self, mode: &str) -> &Keymap;     // mode or default (inherits)
    pub fn default(&self) -> &Keymap;
}
```

## Resolution rules

- `resolve(KeySpec::Single(chord))` returns the bound `Action`, or `None` when
  unbound. The chord is matched with the editing-relevant modifier mask
  (`SHIFT|CONTROL|ALT`), exactly as 005's `resolve` did, so incidental flags never
  defeat a match.
- Both an action's **primary and alternate** resolve to that action (FR-003): a
  press of either key returns it.
- A **cleared** action (no primary, no alternate) resolves from no chord (FR-011).
- `for_mode("name")` returns the named mode's keymap (default overlaid by its
  overrides), or the default keymap when the mode is absent (FR-012 inheritance).

## Conflict detection

- During construction (default map build + override application), if two
  **distinct** actions would resolve from the same `KeySpec`, emit a
  `tracing::warn!` naming both actions and keep the **last-declared** binding
  (FR-010, warn + last-wins).
- An action whose own primary and alternate collapse to the same chord is **not**
  a conflict (it still fires; edge case in spec).

## `/keys` listing

- `listing()` returns every action with a binding (primary, and alternate when
  present) plus the `Esc Esc` gesture row for `clear_status_message`. Actions that
  are unbound (reserved, or cleared by config) are shown as unbound (FR-014).
- The listing reflects the **live effective** map — after config overrides and
  after `/reload-config` — not the static defaults.

## Test obligations (`tests/keymap_engine.rs`)

1. `default_map_matches_legacy_bindings` — every former `BINDINGS` entry resolves
   to its action (FR-002).
2. `insert_newline_has_primary_and_alternate_by_default` — both `Shift+Enter` and
   `Alt+Enter` resolve to `InsertNewline` (FR-004).
3. `copy_variants_are_bound_by_default` — `CopyCurrentLine` and
   `CopyBlockWithoutCommand` each resolve from their default chord (FR-005).
4. `override_rebinds_and_old_key_stops_resolving` (FR-002/FR-011 interaction).
5. `cleared_action_resolves_from_no_chord` (FR-011).
6. `conflict_keeps_last_declared` (FR-010).
7. `for_mode_inherits_default_for_unlisted_actions` (FR-012).
8. `listing_reflects_effective_map_including_unbound` (FR-014).
