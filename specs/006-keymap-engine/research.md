# Phase 0 Research: Configurable Keymap Engine

This document resolves the open design choices for sprint 006. All **product**
decisions were settled in pre-planning
([pre-plan-006-keymap-engine.md](../planning/pre-plan-006-keymap-engine.md)) and
encoded in [spec.md](spec.md) with zero `[NEEDS CLARIFICATION]` markers; what
remains are the **engineering** decisions about how to realize them on top of the
sprint-005 action registry.

---

## R1 — Key-string grammar and parser

**Decision**: A hand-rolled tokenizer. A single-key spec is split on `+` into
modifier tokens and one final key token; tokens are lowercased before matching.
Modifiers map `ctrl → CONTROL`, `alt → ALT`, `shift → SHIFT` (canonical short
names; `control`/`super`/`cmd` are **not** accepted this sprint). The key token
maps via a small name table to `crossterm::event::KeyCode` (`left`, `right`,
`up`, `down`, `home`, `end`, `pageup`, `pagedown`, `enter`, `esc`, `tab`,
`backspace`, `delete`, `insert`, `space`, and single printable chars). A two-key
**chord** is written as two whitespace-separated specs (`Esc Esc`); only `Esc Esc`
is supported (FR-008). Parsing yields the existing `KeyChord` for single keys and
a small `KeySpec` enum (`Single(KeyChord)` | `Chord(KeyChord, KeyChord)`) for the
general case.

**Rationale**: The grammar is tiny and closed (a fixed key-name set + three
modifiers). A dependency (e.g. a key-parsing crate) would violate Constitution VII
for no benefit. Lowercasing tokens gives FR-007 case-insensitivity for free;
splitting on `+` and collecting modifiers as a set gives modifier-order tolerance.
The existing `KeyChord::display()` already formats the canonical string, so the
parser is its inverse and round-trips.

**Alternatives considered**:
- *A parser crate / `FromStr` on crossterm types* — crossterm has no public
  key-string parser; pulling a crate in is disproportionate.
- *Supporting `Control`/`Super`/`Cmd` aliases now* — deferred; pre-plan chose
  canonical short names. Easy to add later without breaking configs.
- *Arbitrary N-key chords* — out of scope (FR-008); only `Esc Esc` is needed for
  the 005 gesture. A general chord engine is speculative (Constitution VII).

---

## R2 — Modeling `Esc Esc` alongside single-key bindings

**Decision**: Introduce `KeySpec { Single(KeyChord), Chord(KeyChord, KeyChord) }`
as the parsed binding target. The effective keymap stores `KeySpec → Action`.
Single-key resolution in `App::on_key` looks up `KeySpec::Single(chord)`. The
`Esc Esc` chord remains **handled contextually** in the event loop (it clears the
status message / multi-line buffer per 005 FR-026/FR-029) — the keymap **surfaces**
the `clear_status_message` ↔ `Esc Esc` association for `/keys` and lets it be
rebound in principle, but the contextual two-key gesture logic in `App` is not
replaced this sprint.

**Rationale**: 005 already tracks `esc_pending` as a keypress flag in `App`; the
`Esc Esc` semantics are inherently stateful and context-sensitive (cancel
selection vs clear line vs clear buffer vs clear message). Re-routing that through
a generic chord resolver would be a larger, riskier change than the spec asks for
(FR-018 explicitly keeps contextual gestures context-sensitive). Representing the
chord in the keymap data model satisfies FR-008/FR-014 (it is bindable and
listed) without rewriting the gesture handler.

**Alternatives considered**:
- *A general chord state machine in the resolver* — over-engineering for one
  chord; the contextual handler already exists and works.
- *Treating `Esc Esc` as two unrelated `Esc` bindings* — loses the gesture
  identity `/keys` needs to display.

---

## R3 — Default map as data vs the static `BINDINGS` table

**Decision**: Convert the existing `const BINDINGS: &[(KeyCode, KeyModifiers,
Action)]` into the **constructor of the default `Keymap`**. The default map is
built once from this same data, so the zero-config effective map is byte-identical
to today's behavior (FR-002). The default map additionally binds the two copy
variants (R4) and ships the newline-insertion alternate (R5). `action::resolve()`
is reframed as `Keymap::resolve(&self, chord)`; the free `resolve()` against the
static table is removed once `App` holds a `Keymap`.

**Rationale**: Reusing the existing binding data as the default-map source is the
smallest change that guarantees identical defaults and keeps a single source of
truth. The registry was explicitly built as this seam (its module docs say so).

**Alternatives considered**:
- *Keep the static table and layer config on top at lookup time* — two lookup
  paths (static + overrides) is more complex than building one effective map.

---

## R4 — Binding the copy variants (`copy_block_without_command`, `copy_current_line`)

**Decision**: Add `Action::CopyBlockWithoutCommand` and `Action::CopyCurrentLine`
to the registry and give them default bindings in the default map. The handler
methods already exist on `App` (`copy_block_without_command(row)`,
`copy_current_line(screen_row)`), but they are **row/screen-targeted** — the mouse
click used to supply the target. With no mouse position, a keyboard binding needs
a deterministic target: both actions act on the **bottom-most transcript output**
(the newest content). `copy_block_without_command` resolves the block at the
newest stable row (the most recently completed command block) and copies its
output; `copy_current_line` copies the last (bottom) visible transcript line. Both
reuse the existing methods verbatim with a computed "bottom" row — no new copy
behavior. Default chords: `Ctrl+Y` → copy current line, `Alt+Y` → copy block
without command (subject to conflict check; finalized in data-model).

**Rationale**: FR-005 requires these get default bindings. They are currently
reachable only via mouse/right-click context paths from 004; giving them keyboard
actions is the explicit ask. Choosing chords not already in the default map avoids
a self-conflict.

**Alternatives considered**:
- *Leave them mouse-only and just list them as unbound in `/keys`* — violates
  FR-005 (they MUST have default bindings).
- *Gate them on an active transcript selection* — overlaps with `Ctrl+C` (which
  already copies an active selection); rejected. The bottom-of-transcript target
  is independent of selection and always well-defined.
- *Bind to keys that need a row argument the keyboard can't supply* — resolved by
  targeting the newest transcript row, reusing the existing methods unchanged.

---

## R5 — Newline insertion as a named, alternate-bearing action

**Decision**: Promote newline insertion to a named `Action::InsertNewline` bound
in the default map to **`["Shift+Enter", "Alt+Enter"]`** (primary + alternate,
FR-004). Today `App::on_key` matches `Enter + SHIFT` and `Enter + ALT` directly;
those two arms are replaced by the action resolving through the keymap. Plain
`Enter` (submit) stays a contextual arm (FR-018) — it is not a keymap action
because its meaning (submit vs. newline) is buffer-contextual and it must always
win when no modifier is present.

**Rationale**: This is the canonical demonstration of the primary/alternate slot
(the pre-plan's worked example) and the one default the spec calls out (FR-004).
Modeling it as one action with two keys is exactly what the alternate slot is for.

**Alternatives considered**:
- *Keep newline insertion hardcoded* — would leave the headline alternate example
  unconfigurable, contradicting FR-003/FR-004.
- *Make plain `Enter` a keymap action too* — its submit-vs-newline meaning is
  contextual; FR-018 keeps it in the event loop.

---

## R6 — Retaining the config path for `/reload-config`

**Decision**: `App` gains a `config_path: Option<PathBuf>` field holding the
**resolved** path the run loaded from (the `--config` argument, or the resolved
XDG default when present). `lib.rs` resolves the path once and passes it to
`App::new(config, config_path)`. `/reload-config` calls `Config::load(path)`
again, rebuilds the `Keymaps`, and swaps it in **only on success**; on error it
keeps the current keymap and surfaces the error in a synthetic block (FR-016).
The shell-override (`--shell`) is applied to the reloaded config the same way it
was at startup so reload does not drop it.

**Rationale**: `App::new` currently takes only `Config` and discards how it was
loaded, so reload has nothing to re-read. Storing the resolved path is the minimal
addition. Swapping only on success satisfies FR-016 (malformed reload keeps the
prior map); reload touching only `config`/`keymaps` (not `input`) satisfies FR-017
(in-progress buffer preserved).

**Alternatives considered**:
- *Re-derive the XDG default at reload time* — fails if the user started with
  `--config`; must reuse the exact path.
- *File watching* — explicitly rejected in pre-planning (Q4); on-demand only.

---

## R7 — `[keymap]` config shape, per-mode sections, and conflict policy

**Decision**: The TOML surface is a `[keymap]` table whose top level is the
**default mode**, with optional per-mode subtables. Each entry maps an action name
to either a string or an array of strings:

```toml
[keymap]
word_move_left  = "Ctrl+Left"               # primary only
insert_newline  = ["Shift+Enter", "Alt+Enter"]  # primary + alternate
kill_to_line_start = ""                       # cleared / disabled

[keymap.laat]                                  # per-mode override (sprint 008)
scroll_line_up = "Ctrl+P"
```

`into_config` parses this into `Keymaps { default: Keymap, modes: Map<String,
Keymap> }`, where each mode keymap is the default map overlaid by that mode's
overrides (FR-012). Building the effective map: start from the data-derived
default map, apply each configured override (parse keys → bind; empty value →
unbind, FR-011). **Conflict detection** runs over the resulting bindings: if two
distinct actions resolve from the same `KeySpec`, emit a `tracing::warn!` naming
both and keep the **last-declared** binding (FR-010, warn + last-wins, by config
declaration order). Unknown action names and unknown mode subtables emit a
`tracing::warn!` and are ignored (FR-013). Unparseable key strings warn and are
skipped (FR-009). The `[keymap]` key is added to `TOP_LEVEL_KEYS`; per-action
unknown-key warnings are handled by the engine (the action-name space is dynamic),
not the static `warn_unknown_keys` table.

**Rationale**: This mirrors the established config pattern (`[status]`,
`[divider]`, `[scroll]` as Raw structs + `into_config` + warn loop) while
accommodating the dynamic action-name keyspace. The array form is the pre-plan's
chosen alternate syntax. Last-declared-wins is well-defined because TOML preserves
table order within a section, and the default map is applied first so user entries
always win over (and can clear) defaults.

**Alternatives considered**:
- *Explicit `alt =` field per action* — rejected in pre-planning (Q1) in favor of
  the array form, which is terser and matches the "two keys for one action" mental
  model.
- *Hard-error on conflict* — rejected in pre-planning (Q2); warn + last-wins keeps
  kapollo always startable (FR-010, SC-004).
- *Per-action entries in the static `warn_unknown_keys` allow-list* — impossible;
  action names are an open set relative to that table, so the engine validates
  them and warns on unknowns instead.

---

## Summary of decisions

| # | Decision | Drives |
|---|----------|--------|
| R1 | Hand-rolled key-string tokenizer; canonical short modifiers; lowercased tokens; fixed key-name table; no new dep | FR-006, FR-007 |
| R2 | `KeySpec { Single, Chord }`; `Esc Esc` listed/bindable but gesture stays contextual | FR-008, FR-014, FR-018 |
| R3 | Existing `BINDINGS` data becomes the default-`Keymap` constructor; `resolve` becomes a `Keymap` method | FR-002 |
| R4 | Add `CopyBlockWithoutCommand` + `CopyCurrentLine` actions targeting the bottom-most transcript output (newest block / last line) | FR-005 |
| R5 | `InsertNewline` action default-bound `["Shift+Enter","Alt+Enter"]`; plain Enter stays contextual | FR-003, FR-004, FR-018 |
| R6 | `App` retains resolved config path; `/reload-config` swaps keymap only on success; input preserved | FR-015, FR-016, FR-017 |
| R7 | `[keymap]` table (default mode + per-mode subtables); array alternates; empty = unbind; warn + last-wins; unknowns warned | FR-009–FR-013 |

No new runtime dependencies. All decisions stay within Constitution VII
(simplicity): the engine is the minimum that satisfies the spec, reusing the
005 registry and the established config-parsing pattern.
