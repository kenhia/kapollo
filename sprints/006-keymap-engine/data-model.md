# Phase 1 Data Model: Configurable Keymap Engine

Entities are the types the keymap engine introduces or extends. They live in
`src/action` (the registry grown into a keymap), with the config-facing `Raw`
types in `src/config.rs`. All are pure values; none own I/O.

---

## 1. Action (extended)

The named-behavior enum from sprint 005, extended with the actions this sprint
makes bindable.

- **Existing (005)**: `LineMoveStart`, `LineMoveEnd`, `WordMoveLeft`,
  `WordMoveRight`, `SelectCharLeft`, `SelectCharRight`, `SelectWordLeft`,
  `SelectWordRight`, `KillToLineStart`, `KillToLineEnd`, `DeleteWordBefore`,
  `ScrollPageUp`, `ScrollPageDown`, `ScrollLineUp`, `ScrollLineDown`,
  `ScrollToTop`, `ScrollToBottom`, `ClearStatusMessage` (contextual),
  `MultilineMoveStartBuffer`/`MultilineMoveEndBuffer` (reserved, unbound).
- **New (006)**:
  - `InsertNewline` — insert a newline in the input buffer (R5).
  - `CopyCurrentLine` — copy the newest (bottom) transcript line (R4).
  - `CopyBlockWithoutCommand` — copy the most recently completed block's output
    without its command line (R4).

**Attributes**: `name() -> &'static str` (stable, surfaced by `/keys` and config)
— extended with the three new names (`insert_newline`, `copy_current_line`,
`copy_block_without_command`).

**Validation**: the action-name set is closed in code; config binds keys to names
but cannot create actions. An unknown name in config is warned and ignored
(FR-013).

**Relationships**: bound to one or more `KeySpec`s by a `Keymap`.

---

## 2. KeyChord (unchanged)

A single key plus its editing-relevant modifier bits — the existing 005 type.

- **Attributes**: `code: KeyCode`, `mods: KeyModifiers` (masked to
  `SHIFT|CONTROL|ALT`).
- **Methods (existing)**: `new(code, mods)` (masks), `display() -> String`
  (canonical `Ctrl+Left` rendering — the parser's inverse).

---

## 3. KeySpec (new)

The parsed target of a key string: a single chord or the two-key `Esc Esc` chord.
This is the design realization of the spec's "Key chord" entity (which describes a
key+mods *or* the `Esc Esc` sequence as one concept).

```text
enum KeySpec {
    Single(KeyChord),
    Chord(KeyChord, KeyChord),   // only Esc Esc this sprint (FR-008)
}
```

- **Methods**: `parse(&str) -> Result<KeySpec, KeyParseError>` (R1; case-
  insensitive, modifier-order-tolerant), `display() -> String` (round-trips).
- **Validation**: `parse` rejects unknown modifier names, unknown key names, empty
  specs, and chords other than two whitespace-separated single keys; a chord of
  more than two keys, or a chord that is not `Esc Esc`, is a parse error (FR-008).
- **Relationships**: the lookup key of a `Keymap`.

---

## 4. KeyParseError (new)

Why a key string failed to parse, for the FR-009 warning.

- **Attributes**: the offending input string + a reason (`unknown modifier`,
  `unknown key`, `empty`, `unsupported chord`).
- **Use**: surfaced in a `tracing::warn!` that names the binding; the binding is
  then skipped (kapollo still starts).

---

## 5. Binding (new)

An action's primary and optional alternate key.

```text
struct Binding {
    primary: Option<KeySpec>,     // None == disabled / cleared
    alternate: Option<KeySpec>,
}
```

- **Validation**: built from a config value that is a string (→ primary only), a
  one- or two-element array (`[primary]` or `[primary, alternate]`), or an empty
  string / empty array (→ both `None`, disabled — FR-011). Arrays longer than two
  elements warn and keep the first two.
- **Relationships**: a `Keymap` holds one `Binding` per bound action.

---

## 6. Keymap (new)

The set of bindings in effect for one mode: the resolution table the event loop
queries.

- **Construction**:
  - `Keymap::default_map()` — built from the (data-fied) 005 `BINDINGS` plus the
    006 additions (copy variants, `InsertNewline` with its alternate). This is the
    zero-config map and MUST equal current behavior (FR-002).
  - `Keymap::with_overrides(base, overrides)` — overlay configured `(Action,
    Binding)` overrides onto a base map: bind, rebind, or unbind (empty value).
- **Methods**:
  - `resolve(&self, spec: KeySpec) -> Option<Action>` — single-key lookup the
    event loop uses (replaces the free `action::resolve`).
  - `listing(&self) -> Vec<(String, String)>` — `(action name, key display)` for
    `/keys`, including primary + alternate and unbound markers (FR-014).
  - conflict detection during construction: if two distinct actions resolve from
    the same `KeySpec`, `tracing::warn!` names both and the last-declared wins
    (FR-010).

**Relationships**: `Keymaps` holds the default `Keymap` plus per-mode overlays.

---

## 7. Keymaps (new)

The full multi-mode keymap held by `App`.

```text
struct Keymaps {
    default: Keymap,
    modes: BTreeMap<String, Keymap>,   // each = default overlaid by the mode's overrides
}
```

- **Methods**: `for_mode(&self, mode: &str) -> &Keymap` (the named mode, or
  `default` when absent — FR-012 inheritance), `default(&self) -> &Keymap`.
- **Construction**: from parsed config (`RawKeymap`) overlaid on `default_map()`;
  unknown mode subtables warned and ignored (FR-013).
- **Lifecycle**: built once at startup; rebuilt and swapped on `/reload-config`,
  but only on successful reload (FR-016); swapping it does not touch the input
  buffer (FR-017).

---

## 8. RawKeymap (new, config-facing)

The `serde` deserialization shape for the `[keymap]` table (in `src/config.rs`).

- The default-mode entries are a `map<action-name, RawBinding>`.
- Per-mode subtables (`[keymap.<mode>]`) are a `map<mode-name, map<action-name,
  RawBinding>>`.
- `RawBinding` accepts a TOML **string** or **array of strings** (untagged), plus
  the empty-string / empty-array clear form.
- `into_config()` parses each `RawBinding` into a `Binding` (via `KeySpec::parse`,
  warning + skipping bad strings) and builds `Keymaps`.

**Note**: because the action-name keyspace is open relative to the static
`warn_unknown_keys` table, the engine — not that table — validates action names
and emits the unknown-action warnings (FR-013, research R7). `[keymap]` is added
to `TOP_LEVEL_KEYS`.

---

## 9. App state (extended)

`App` (in `src/app.rs`) gains:

- `keymaps: Keymaps` — the effective multi-mode map (replaces the implicit static
  `BINDINGS` lookup in `on_key`).
- `config_path: Option<PathBuf>` — the resolved path the run loaded from, retained
  so `/reload-config` can re-read it (research R6). Threaded in from `lib.rs`.

The current mode is the default mode this sprint (the 005 status bar shows a
`norm` label; per-mode maps are built and tested but only the default is
populated, per the spec's assumptions).

---

## Entity relationships

```text
Keymaps ──has one──▶ Keymap (default)
   │
   └──has many──▶ Keymap (per mode)        each = default ⊕ mode overrides

Keymap ──maps──▶ KeySpec → Action          (resolution table)
Binding ──holds──▶ primary: Option<KeySpec>, alternate: Option<KeySpec>
KeySpec ──is──▶ Single(KeyChord) | Chord(KeyChord, KeyChord)
Action ──named by──▶ name(): &'static str  (stable; surfaced by /keys + config)

App ──holds──▶ Keymaps + config_path
RawKeymap ──parses into──▶ Keymaps         (warn + skip on bad keys/names; last-wins on conflict)
```

## Notes on transitions

- **Startup**: `lib.rs` resolves config + path → `Keymaps::from(config)` overlaid
  on `default_map()` → `App::new(config, path, keymaps)`.
- **`/reload-config`**: re-`Config::load(path)` → rebuild `Keymaps`. On success,
  swap `app.config` + `app.keymaps`; on failure, keep both and report the error.
  `app.input` is never touched.
- **Keystroke**: `App::on_key` builds a `KeySpec::Single` from the event and calls
  `keymaps.for_mode(current_mode).resolve(spec)`; contextual arms (plain `Enter`,
  `Ctrl+C`, `Esc`/`Esc Esc`) are checked before/around the keymap lookup exactly
  as in 005 (FR-018).
