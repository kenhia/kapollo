# Tasks: Configurable Keymap Engine

**Input**: Design documents from `/specs/006-keymap-engine/`
**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/)

**Tests**: INCLUDED — Constitution III (TDD) is mandatory for this repo. This
feature is **almost entirely pure logic** (key-string parsing, the default →
effective overlay, primary/alternate resolution, disable-by-clearing, per-mode
inheritance, conflict detection), all unit-tested **before** implementation.
Live-TTY behavior (a rebound key firing, `/reload-config` applying without losing
the in-progress buffer, a malformed reload keeping the old map) is covered by the
manual [quickstart.md](quickstart.md) per the documented integration/manual
exception.

**Organization**: Tasks are grouped by user story. US1 + US2 are both P1 (the MVP).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1–US4 for user-story tasks; Setup/Foundational/Polish carry no label
- All paths are repository-root-relative; gate = `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test`

## Path Conventions

Single Rust crate: `src/`, `tests/` at repository root (per [plan.md](plan.md) Project Structure). The keymap engine grows the existing `src/action` module.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Confirm a green baseline and scaffold the new test files so the crate compiles before any logic lands.

- [X] T001 Confirm the baseline gate is green on `006-keymap-engine` (run `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test`) so all later failures are attributable to this feature.
- [X] T002 [P] Create empty integration-test files so new suites are discovered: `tests/keymap_parse.rs`, `tests/keymap_config.rs`, `tests/keymap_engine.rs`, `tests/keymap_defaults_doc.rs` (each with a single `#[test] fn placeholder() {}` to start, replaced in later tasks).

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The key-string parser, the extended action set, and the `Keymap`/`Keymaps` types that every user story binds and resolves against.

**⚠️ CRITICAL**: US1 resolves keys through `Keymap`, US2 needs the `Binding` primary/alternate model, US3 needs the parser + conflict detection, and US4 needs `Keymaps` per-mode — these types must exist first.

- [X] T003 Extend the `Action` enum in `src/action/mod.rs` with the three new bindable actions `InsertNewline`, `CopyCurrentLine`, `CopyBlockWithoutCommand`, including their `name()` strings (`insert_newline`, `copy_current_line`, `copy_block_without_command`) — see [data-model.md](data-model.md) §1.
- [X] T004 [P] Write the key-string parser tests FIRST in `tests/keymap_parse.rs` (replace the placeholder): case-insensitive + modifier-order equality, short-modifier-names-only (`Control+Left` rejects), unknown key rejects, empty rejects, `Esc Esc` parses as a chord, multi-key sequences other than `Esc Esc` reject, and `display()` round-trips through `parse()` — see [contracts/key-string.md](contracts/key-string.md).
- [X] T005 Implement `KeySpec { Single(KeyChord), Chord(KeyChord, KeyChord) }`, `KeyParseError`, `KeySpec::parse(&str)`, and `KeySpec::display()` in `src/action/mod.rs` (hand-rolled tokenizer: split on `+`, lowercase tokens, fixed key-name table, canonical short modifiers; `Esc Esc`-only chord) so the T004 tests pass (FR-006/FR-007/FR-008; depends on T004). Note: `Esc Esc` is parse-recognized and listed in `/keys` only — its dispatch stays the existing contextual handler this sprint (FR-018, research R2); no chord is wired into single-key resolution.
- [X] T006 Implement the `Binding { primary: Option<KeySpec>, alternate: Option<KeySpec> }` model in `src/action/mod.rs` (string → primary; array → primary+alternate; empty/`[]` → both `None`; >2 elements keep first two) — see [data-model.md](data-model.md) §5 (depends on T005).
- [X] T007 Implement the `Keymap` type in `src/action/mod.rs`: internal `KeySpec → Action` table, `resolve(&self, KeySpec) -> Option<Action>` (modifier-masked, replacing the free `resolve()`), `with_overrides(base, overrides)` (bind/rebind/unbind), `listing()`, and conflict detection (warn + last-declared-wins via `tracing::warn!`) — see [contracts/keymap-engine.md](contracts/keymap-engine.md) (depends on T006).
- [X] T008 Implement `Keymaps { default: Keymap, modes: BTreeMap<String, Keymap> }` in `src/action/mod.rs` with `for_mode(&str)` (named mode or default-inheritance) and `default()` — see [data-model.md](data-model.md) §7 (depends on T007).

**Checkpoint**: Parser + `Keymap`/`Keymaps` exist and compile — user stories can begin.

---

## Phase 3: User Story 1 - Rebind any action from config with identical defaults (Priority: P1) 🎯 MVP

**Goal**: Every action is bindable from `[keymap]`; a zero-config user gets exactly the sprint-004/005 defaults, including the two previously-unbound copy variants.

**Independent Test**: With no `[keymap]`, confirm every action resolves to its established default; rebind a couple of actions and confirm the new keys fire and the old ones do not (quickstart §0, §3; SC-001/SC-002).

### Tests for User Story 1 (write first, ensure they FAIL)

- [X] T009 [P] [US1] Write the default-map engine tests FIRST in `tests/keymap_engine.rs` (replace the placeholder): every former `BINDINGS` chord resolves to its action (`default_map_matches_legacy_bindings`), the two copy variants resolve from their default chords (`copy_variants_are_bound_by_default`), an override rebinds and the old key stops resolving, and `for_mode` inherits the default for unlisted actions (FR-002/FR-005) — see [contracts/keymap-engine.md](contracts/keymap-engine.md).
- [X] T010 [P] [US1] Write the config-parsing tests FIRST in `tests/keymap_config.rs` (replace the placeholder): `no_keymap_table_yields_default_map`, `string_binding_sets_primary_only`, and `unknown_action_name_is_warned_and_ignored` (other bindings still apply) — see [contracts/keymap-config.md](contracts/keymap-config.md).

### Implementation for User Story 1

- [X] T011 [US1] Implement `Keymap::default_map()` in `src/action/mod.rs` from the data-fied 005 `BINDINGS` plus the new copy-variant default bindings (`Ctrl+Y` → `CopyCurrentLine`, `Alt+Y` → `CopyBlockWithoutCommand`, validated to not conflict), so the T009 default/copy tests pass (FR-002/FR-005; depends on T007).
- [X] T012 [US1] Add the `[keymap]` config surface in `src/config.rs`: `RawKeymap` (default-mode `map<action-name, RawBinding>` + `map<mode, …>` subtables), `RawBinding` (string | array | empty), `into_config` building `Keymaps` over `default_map()` (warn + skip bad key strings, warn + ignore unknown action names), and add `"keymap"` to `TOP_LEVEL_KEYS` — see [contracts/keymap-config.md](contracts/keymap-config.md) and [data-model.md](data-model.md) §8 (depends on T008, T011).
- [X] T013 [US1] Thread the resolved config path and effective keymap into the app: store the resolved `config_path: Option<PathBuf>` in `src/lib.rs` and pass it plus the built `Keymaps` into `App::new` in `src/app.rs` (add `keymaps: Keymaps` + `config_path` fields), per research R6 — see [data-model.md](data-model.md) §9 (depends on T012).
- [X] T014 [US1] Replace the static lookup in `src/app.rs` `on_key`: resolve key events through `self.keymaps.default().resolve(KeySpec::Single(chord))` (the default/`norm` mode is the only populated map this sprint; a real mode selector is deferred to sprint 008) instead of the free `action::resolve`, keeping the contextual arms (plain `Enter`, `Ctrl+C`, `Esc`/`Esc Esc`) unchanged (FR-001/FR-018; depends on T013).
- [X] T015 [US1] Wire the copy-variant actions in `src/app.rs` `dispatch_action`: `CopyCurrentLine` and `CopyBlockWithoutCommand` arms targeting the **bottom-most transcript output** (newest visible line / most recently completed block), reusing the existing `copy_current_line(screen_row)`/`copy_block_without_command(row)` methods with the computed bottom row; add a test asserting the chosen target (FR-005; depends on T014).

**Checkpoint**: Actions are config-bindable, defaults are identical, copy variants are bound — MVP slice 1 works.

---

## Phase 4: User Story 2 - Primary and alternate bindings per action (Priority: P1) 🎯 MVP

**Goal**: An action can carry a primary and an alternate key (array form); the default map ships the `["Shift+Enter", "Alt+Enter"]` newline alternate.

**Independent Test**: Bind an action to a two-element array and confirm both keys fire; confirm a single string still works; confirm `Shift+Enter` and `Alt+Enter` both insert a newline out of the box (quickstart §4; FR-003/FR-004).

### Tests for User Story 2 (write first, ensure they FAIL)

- [X] T016 [P] [US2] Add engine tests in `tests/keymap_engine.rs`: `insert_newline_has_primary_and_alternate_by_default` (both `Shift+Enter` and `Alt+Enter` resolve to `InsertNewline`) (FR-004) — see [contracts/keymap-engine.md](contracts/keymap-engine.md).
- [X] T017 [P] [US2] Add config tests in `tests/keymap_config.rs`: `array_binding_sets_primary_and_alternate` (both keys resolve) and a one-element array behaves as primary-only (FR-003) — see [contracts/keymap-config.md](contracts/keymap-config.md).

### Implementation for User Story 2

- [X] T018 [US2] Bind `Action::InsertNewline` in `Keymap::default_map()` to primary `Shift+Enter` + alternate `Alt+Enter`, and ensure `resolve`/`with_overrides` resolve **both** a binding's primary and alternate to its action (FR-003/FR-004; depends on T011, T007).
- [X] T019 [US2] Replace the two hardcoded newline arms in `src/app.rs` `on_key` (`Enter`+SHIFT and `Enter`+ALT) with the `InsertNewline` action dispatched via the keymap, keeping plain `Enter` as the contextual submit arm (FR-004/FR-018; depends on T018, T014).

**Checkpoint**: Primary+alternate works end to end; newline insertion is keymap-driven with its alternate. MVP (US1+US2) complete.

---

## Phase 5: User Story 3 - Readable key strings, validated, conflicts reported (Priority: P2)

**Goal**: Forgiving grammar; unparseable bindings warn-and-skip; same-key conflicts warn and last-declared wins; disable-by-clearing works.

**Independent Test**: Mixed-case/short-vs-long spellings, an unparseable binding (kapollo still starts), and two actions on one key (conflict warning + last-wins) (quickstart §5–8; FR-009/FR-010/FR-011).

### Tests for User Story 3 (write first, ensure they FAIL)

- [X] T020 [P] [US3] Add config tests in `tests/keymap_config.rs`: `empty_value_clears_and_disables_action` (former default no longer resolves), `unparseable_key_is_skipped_and_others_apply`, and `conflicting_bindings_last_declared_wins` (FR-009/FR-010/FR-011) — see [contracts/keymap-config.md](contracts/keymap-config.md).
- [X] T021 [P] [US3] Add engine tests in `tests/keymap_engine.rs`: `cleared_action_resolves_from_no_chord` and `conflict_keeps_last_declared` (a self-collapsing primary==alternate is **not** a conflict) (FR-010/FR-011) — see [contracts/keymap-engine.md](contracts/keymap-engine.md).

### Implementation for User Story 3

- [X] T022 [US3] Implement disable-by-clearing in `src/action/mod.rs`/`src/config.rs`: an empty binding value produces a `Binding` with no primary/alternate, and `with_overrides` removes the action from the resolution table (FR-011; depends on T012, T007).
- [X] T023 [US3] Implement conflict detection + warn/skip diagnostics in `Keymap` construction (`src/action/mod.rs`): emit `tracing::warn!` naming both actions on a same-`KeySpec` collision and keep the last-declared binding; emit `tracing::warn!` naming the offending binding on a parse failure and skip it (FR-009/FR-010; depends on T012, T007).

**Checkpoint**: The keymap config surface is safe to edit — bad input warns, never blocks startup; conflicts are deterministic.

---

## Phase 6: User Story 4 - Per-mode keymaps and live `/keys` and `/reload-config` (Priority: P3)

**Goal**: Per-mode override+inheritance; `/keys` shows the live effective map; `/reload-config` re-reads on demand without restarting or losing input.

**Independent Test**: A `[keymap.<mode>]` section applies only in that mode; `/keys` reflects configured changes; editing config + `/reload-config` applies new bindings with the in-progress buffer intact; a malformed reload keeps the old map (quickstart §1, §9, §10; FR-012/FR-014/FR-015/FR-016/FR-017).

### Tests for User Story 4 (write first, ensure they FAIL)

- [X] T024 [P] [US4] Add config tests in `tests/keymap_config.rs`: `per_mode_section_overrides_only_listed_actions_and_inherits_rest` and `unknown_mode_section_is_warned_and_ignored` (FR-012/FR-013) — see [contracts/keymap-config.md](contracts/keymap-config.md).
- [X] T025 [P] [US4] Add engine test in `tests/keymap_engine.rs`: `listing_reflects_effective_map_including_unbound` (primary + alternate shown, reserved/cleared shown unbound, includes the `Esc Esc` gesture row) (FR-014) — see [contracts/keymap-engine.md](contracts/keymap-engine.md).
- [X] T026 [P] [US4] Add a slash-dispatch unit test in `src/slash/mod.rs` asserting `dispatch("reload-config")` resolves to `SlashCommand::ReloadConfig` and existing commands are unchanged — see [contracts/slash-commands.md](contracts/slash-commands.md).

### Implementation for User Story 4

- [X] T027 [US4] Implement per-mode parsing in `src/config.rs` `into_config`: build each `[keymap.<mode>]` subtable as the default map overlaid by that mode's listed actions; warn + ignore unknown mode subtables (FR-012/FR-013; depends on T012).
- [X] T028 [US4] Add `SlashCommand::ReloadConfig` to `src/slash/mod.rs` and map `"reload-config"` in `dispatch` (FR-015; depends on T026).
- [X] T029 [US4] Implement `/reload-config` in `src/app.rs` `run_slash`: re-`Config::load(self.config_path)`, re-apply the `--shell` override, rebuild `Keymaps`; on success swap `self.config` + `self.keymaps` and emit a confirm block; on failure emit an error block and keep the previous map; never touch `self.input` (FR-015/FR-016/FR-017; depends on T028, T013).
- [X] T030 [US4] Switch `/keys` in `src/app.rs` `run_slash` to list `self.keymaps.default().listing()` (the live effective map for the default/`norm` mode — the only populated map this sprint) instead of the static `action::listing()` (FR-014; depends on T029, T008).

**Checkpoint**: Per-mode maps, live `/keys`, and on-demand reload all work; reload is safe.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: The generated example config, documentation, and the final gate.

- [X] T031 [P] Generate the example config `docs/keymap-defaults.toml`: a `[keymap]` table fully populated with **every** action's default binding (commented, copy-paste-ready) — see [plan.md](plan.md) Project Structure and [contracts/keymap-config.md](contracts/keymap-config.md) (depends on T011, T018).
- [X] T032 Write the example-config sync test in `tests/keymap_defaults_doc.rs` (replace the placeholder): load `docs/keymap-defaults.toml` and assert it builds a `Keymap` equal to `Keymap::default_map()`, so the example can never drift (Constitution III + V; FR-019; depends on T031).
- [X] T033 [P] Update `src/slash/builtins.rs` `help_text` to list `/reload-config` and keep the `/keys` pointer; update the builtins unit test (`help_text_mentions_reload_config`) — see [contracts/slash-commands.md](contracts/slash-commands.md) (depends on T028).
- [X] T034 [P] Update user-facing docs (FR-019, Constitution V): `README.md` (`[keymap]` config + `/reload-config`), `docs/usage.md` (binding syntax, primary/alternate array, disable-by-clearing, per-mode sections, `/keys`/`/reload-config`, pointer to `docs/keymap-defaults.toml`), `docs/setup.md` (new `[keymap]` config keys), `docs/architecture.md` (a "keymap engine (006)" section), `docs/specification.md` (FR-S section for sprint 006).
- [X] T035 Run the full gate (`cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test`) and confirm green, including all new keymap suites.
- [X] T036 Execute the manual [quickstart.md](quickstart.md) on a live TTY (rebinding fires, `/keys` live, `/reload-config` preserves the in-progress buffer, malformed reload keeps the old map) and record pass/fail per step (SC-001…006). **Completed: walkthrough passed on a live TTY (2026-06-06).**

---

## Dependencies & completion order

```text
Setup (T001–T002)
  └─▶ Foundational (T003–T008): Action set, KeySpec parser, Binding, Keymap, Keymaps
        ├─▶ US1 (T009–T015)  P1 🎯  default map + [keymap] config + app wiring + copy variants
        │     └─▶ US2 (T016–T019)  P1 🎯  primary/alternate + InsertNewline (needs default_map + on_key)
        │            └─▶ US3 (T020–T023)  P2  disable-by-clearing + conflicts (needs config + Keymap)
        │                   └─▶ US4 (T024–T030)  P3  per-mode + /reload-config + live /keys
        └─────────────────────────▶ Polish (T031–T036): example config + sync test + docs + gate + quickstart
```

- **US1 → US2**: US2 binds `InsertNewline` in `default_map()` (T011) and replaces the newline arms in the same `on_key` US1 rewires (T014).
- **US2 → US3**: conflict/clear logic builds on the `Binding`/override path US1+US2 establish in config + `Keymap`.
- **US3 → US4**: per-mode overlay and reload reuse the same `into_config`/`Keymaps` plumbing.
- **Shared-file note**: `src/action/mod.rs`, `src/config.rs`, and `src/app.rs` are touched across US1–US4 — those tasks run **sequentially** in priority order. The `[P]` tasks are the per-suite test files (`tests/keymap_*.rs`) and the independent docs/example tasks, which touch distinct files.

## Parallel opportunities

- **Setup**: T002 creates four independent test files in one shot.
- **Within each story**: the test-writing tasks are `[P]` (distinct `tests/keymap_*.rs` files): T009+T010 (US1), T016+T017 (US2), T020+T021 (US3), T024+T025+T026 (US4).
- **Polish**: T031 (example config), T033 (builtins/help), T034 (docs) are `[P]` against each other; T032 depends on T031; T035/T036 are the closing gate + manual pass.

## Implementation strategy

- **MVP = US1 + US2** (both P1): every action configurable with identical defaults, the copy variants bound, and the primary/alternate slot working with the newline alternate. That alone delivers the sprint's headline value and is independently shippable.
- **Incremental delivery**: US3 (safe editing: warn/skip/clear/conflicts) and US4 (per-mode + live reload) layer on without reworking the MVP.
- **TDD throughout**: every implementation task is preceded by its `[P]` test task in the same story; the pure-logic core (parser, overlay, conflicts, inheritance) is fully covered by `cargo test`, with the live behaviors in the quickstart.

## Format validation

All tasks carry a checkbox, a sequential `T###` id, a `[P]` marker where parallelizable, a `[US#]` label for user-story tasks (Setup/Foundational/Polish unlabeled), and an explicit file path.
