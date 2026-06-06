# Contract: `[keymap]` configuration surface

The TOML surface for binding actions, parsed in `src/config.rs` and built into a
`Keymaps` (see [data-model.md](../data-model.md) §6–8). Tested by
`tests/keymap_config.rs` (+ `tests/config.rs` for the default/unknown cases).

## Table shape

```toml
# Default-mode bindings. Top level of [keymap] = the default mode.
[keymap]
word_move_left     = "Ctrl+Left"                  # string  -> primary only
word_move_right    = "Ctrl+Right"
insert_newline     = ["Shift+Enter", "Alt+Enter"] # array   -> primary + alternate
kill_to_line_start = ""                            # empty   -> cleared / disabled
copy_current_line  = "Ctrl+Y"

# Per-mode override (anticipating LAAT, sprint 008). Overrides only the actions
# it lists; inherits the rest from the default mode.
[keymap.laat]
scroll_line_up   = "Ctrl+P"
scroll_line_down = "Ctrl+N"
```

## Value forms (`RawBinding`)

| TOML value | Meaning |
|------------|---------|
| `"Ctrl+Left"` | primary only, no alternate |
| `["Shift+Enter", "Alt+Enter"]` | primary + alternate (FR-003) |
| `["Ctrl+Left"]` | one-element array == primary only |
| `""` or `[]` | **cleared**: action disabled, former default removed (FR-011) |
| `["a", "b", "c"]` | warns, keeps first two |

## Parsing & validation behavior

- **Default identity**: with **no** `[keymap]` table, `Keymaps::default` equals
  `Keymap::default_map()` — byte-identical to the hardcoded 004/005 defaults
  (FR-002).
- **Override semantics**: each configured entry overlays the default map — bind,
  rebind, or unbind. User entries always win over defaults (defaults applied
  first, R7).
- **Unknown action name** → `tracing::warn!` naming the entry; ignored (FR-013).
- **Unknown mode subtable** (`[keymap.<unknown>]`) → `tracing::warn!`; ignored,
  not fatal (FR-013).
- **Unparseable key string** → `tracing::warn!` naming the binding; that key is
  skipped; kapollo still starts (FR-009).
- **Conflict** (two distinct actions resolving from the same `KeySpec`) →
  `tracing::warn!` naming both actions; **last-declared wins** in config order
  (FR-010).
- **Per-mode inheritance**: a mode keymap = the default map overlaid by that
  mode's listed actions only (FR-012).
- `[keymap]` is added to `TOP_LEVEL_KEYS`; the engine (not the static
  `warn_unknown_keys` table) validates the open action-name keyspace.

## Test obligations (`tests/keymap_config.rs`)

1. `no_keymap_table_yields_default_map` (FR-002).
2. `string_binding_sets_primary_only`.
3. `array_binding_sets_primary_and_alternate` (FR-003).
4. `empty_value_clears_and_disables_action` (FR-011) — former default no longer
   resolves.
5. `unknown_action_name_is_warned_and_ignored` (FR-013) — other bindings still
   apply.
6. `unparseable_key_is_skipped_and_others_apply` (FR-009).
7. `conflicting_bindings_last_declared_wins` (FR-010).
8. `per_mode_section_overrides_only_listed_actions_and_inherits_rest` (FR-012).
9. `unknown_mode_section_is_warned_and_ignored` (FR-013).

## Example config (`docs/keymap-defaults.toml`)

A generated, user-facing example ships a `[keymap]` table **fully populated with
every action's default binding** (commented, copy-paste-ready) so users can see
the complete default map and edit from it. It is kept honest by a sync test
(`tests/keymap_defaults_doc.rs`): loading the file MUST build a `Keymap` equal to
`Keymap::default_map()`, so the example can never drift from the real defaults
(Constitution III + V).
