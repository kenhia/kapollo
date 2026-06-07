# Tasks: LAAT Mode, `/save`, `/filter`, and `/load`

**Input**: Design documents from `/specs/007-laat-mode/`
**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/)

**Tests**: REQUIRED. Constitution III (TDD) is mandatory; the live-TTY pieces (mode
label render, LAAT highlight, `/save` prompt key-handling, `/filter` shell
round-trip) use the documented integration/manual exception and are validated by
[quickstart.md](quickstart.md), not automated tests.

**Organization**: Tasks are grouped by user story (spec.md): US1 LAAT step-through
(P1), US2 `Mult` editing (P1), US3 `/save`+`/filter` (P2), US4 push/pop (P2),
US5 `/load` (P3).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1–US5 for user-story tasks; Setup/Foundational/Polish carry no label
- Exact file paths are included in each task

## Path Conventions

Single Rust binary crate (`kapollo`): sources under `src/`, integration tests under
`tests/`, docs under `docs/`. Paths below are repository-relative.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Establish a green baseline before any change.

- [X] T001 Confirm the baseline gate is green so all later work starts from green: run `cd /home/ken/src/tools/kapollo && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The shared `InputMode` state, status-bar wiring, vertical caret motion,
and the `Ctrl+Alt+1` mode toggle — required by both P1 stories before either can be
implemented.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T002 [P] Add the `InputMode` enum (`Norm`/`Mult`/`Laat`) with `label()` returning `"norm"`/`"Mult"`/`"1T"`, plus unit tests for the labels, in src/input/mod.rs (per [contracts/input-modes.md](contracts/input-modes.md) §1 and [data-model.md](data-model.md))
- [X] T003 [P] Add vertical caret motion to `InputPad` — `caret_line_up`, `caret_line_down` (column-preserving, clamped, selection-collapsing) and `caret_on_first_line`/`caret_on_last_line` — with unit tests, in src/input/mod.rs (per [contracts/input-modes.md](contracts/input-modes.md) §2)
- [X] T004 Add a `mode: InputMode` field to `App` (initialized to `Norm`) in src/app.rs
- [X] T005 Pass `app.mode.label()` into `status::fit` instead of the hardcoded `DEFAULT_MODE` literal in src/ui/status.rs
- [X] T006 ⚠️ [Foundational test — write first, must fail] Write the mode-toggle and mode-aware-navigation tests — C2 (`Ctrl+Alt+1` from an empty `Norm` buffer enters `Mult`), C3 (`Ctrl+Alt+1` toggles `Mult ↔ Laat` when multi-line), C4 (`Up` in `Mult` with the caret below line 1 moves the caret with no history recall) from [contracts/input-modes.md](contracts/input-modes.md) §4 — in tests/input_modes.rs
- [X] T007 Register the `ToggleMultLaat` action (stable name `toggle_mult_laat`, default `Ctrl+Alt+1`) — add the `Action` variant, wire `name`/`from_name`/`default_map`, and add a `dispatch_action` arm calling `App::toggle_mult_laat()` — in src/action/mod.rs and src/app.rs (per [contracts/keymap-actions.md](contracts/keymap-actions.md))
- [X] T008 Implement `App::toggle_mult_laat()` mode transitions on the mode enum — `Norm → Mult` (even empty/single line), and `Mult ↔ Laat` only when multi-line — in src/app.rs (FR-015/FR-016; [contracts/input-modes.md](contracts/input-modes.md) §1) — makes C2/C3 pass
- [X] T009 Make `Up`/`Down` in `App::on_key` mode-aware — `Norm` recalls history as today; `Mult`/`Laat` call `caret_line_up`/`caret_line_down` (basic caret motion, clamped at edges) — in src/app.rs (FR-009; [contracts/input-modes.md](contracts/input-modes.md) §2) — makes C4 pass
- [X] T010 [P] Add the `toggle_mult_laat` default binding to docs/keymap-defaults.toml and extend the keymap sync/listing tests so the example stays equal to `default_map()` and `/keys` lists the action, in tests/keymap_defaults_doc.rs and tests/keymap_engine.rs (per [contracts/keymap-actions.md](contracts/keymap-actions.md) K1–K5)

**Checkpoint**: Modes exist, the status bar shows them, caret motion works, and
`Ctrl+Alt+1` enters/toggles modes (T006 tests green) — US1 and US2 can now proceed.

---

## Phase 3: User Story 1 - Step through a sequence of commands one at a time (Priority: P1) 🎯 MVP

**Goal**: LAAT mode — a highlight steps line-by-line; `Enter` submits the
highlighted line; the highlight advances only on exit `0`, and a non-zero exit
flags a probable failure.

**Independent Test**: Type three lines, `Ctrl+Alt+1` twice to reach `Laat` (mode
field `1T`, line 1 highlighted); submit a succeeding line and confirm the highlight
advances; submit `false` and confirm the highlight stays and the line is flagged.

### Tests for User Story 1 ⚠️ (write first, must fail)

- [X] T011 [P] [US1] Write LAAT stepping + exit-code gating tests (L1–L6 from [contracts/laat-engine.md](contracts/laat-engine.md) §6: enter-highlights-line-0, exit-0 advances, non-zero flags, Down+Enter advances past failure, re-run clears flag, `Esc Esc` clears the LAAT buffer) in tests/laat_engine.rs

### Implementation for User Story 1

- [X] T012 [US1] Add the `LaatState { highlight, failed_lines, pending }` type with a pure `apply_exit_code(pending, exit) -> advance|flag` gating function and module unit tests, in src/input/mod.rs (per [data-model.md](data-model.md) `LaatState` and [contracts/laat-engine.md](contracts/laat-engine.md) §1/§3)
- [X] T013 [US1] Add a `laat: Option<LaatState>` field to `App` and extend `App::toggle_mult_laat()` (T008) to initialize it (`highlight = 0`, empty flags) on entering `Laat` and clear it on leaving, in src/app.rs
- [X] T014 [US1] Make the highlight track the caret line during `Up`/`Down` while in `Laat` (extend the mode-aware navigation from T009) in src/app.rs (per [contracts/laat-engine.md](contracts/laat-engine.md) §2)
- [X] T015 [US1] In `Laat`, make `Enter` submit the highlighted line as a normal single-line submission and set `pending` to that line index; when a multi-line selection is active, `Enter` instead submits the selection as one combined submission (selection overrides the highlight, FR-003/FR-017), in src/app.rs (FR-003/FR-005; [contracts/laat-engine.md](contracts/laat-engine.md) §3)
- [X] T016 [US1] Hook the existing `Boundary::CommandEnd { exit_code }` observation (the path that updates `App.last_exit`) so that, when a LAAT submission is `pending`, exit `0` advances the highlight (clearing that line's flag) and non-zero sets the probable-failure flag and keeps the highlight, in src/app.rs (FR-004; research R5)
- [X] T017 [US1] Implement failure recovery (re-run with `Enter`, `Down`+`Enter` to advance past, `Esc Esc` to abort) and make leaving `Laat` clear the LAAT buffer, in src/app.rs (FR-006/FR-007; [contracts/laat-engine.md](contracts/laat-engine.md) §4)
- [X] T018 [US1] Render the LAAT highlight (background on the highlighted line) and the probable-failure background (flagged lines) when `mode == Laat`, honoring `color_enabled()`, in src/ui/input_pad.rs (FR-002/FR-004; [contracts/laat-engine.md](contracts/laat-engine.md) §5)

**Checkpoint**: LAAT is fully functional — step, gate, flag, recover — independently
demonstrable. **This is the MVP.**

---

## Phase 4: User Story 2 - Edit multi-line input naturally in `Mult` mode (Priority: P1)

**Goal**: In `Mult`, `Up`/`Down` move the caret between lines (fixing the 005
buffer-loss edge), with chat-style edge recall that stashes the draft on `Up` at
the top and restores it on `Down`.

**Independent Test**: Type a line, `Alt+Enter` (mode `Mult`), type a second line,
`Up` moves the caret up (no history recall); from the first line `Up` stashes the
draft and recalls history, `Down` restores the stashed draft; deleting back to one
line returns to `Norm`.

### Tests for User Story 2 ⚠️ (write first, must fail)

- [X] T019 [P] [US2] Write the US2-specific mode and edge-recall tests — C1 (`Alt+Enter` enters `Mult`), C5 (edge stash on `Up` at line 1 + restore on `Down` past newest), C6 (delete back to one line returns to `Norm`), C7 (plain `Enter` combined submit), C8 (multi-line selection + `Enter` is one submission) from [contracts/input-modes.md](contracts/input-modes.md) §4 (C2/C3/C4 are covered by the Foundational T006) in tests/input_modes.rs

### Implementation for User Story 2

- [X] T020 [US2] Enter `Mult` automatically when an edit grows the buffer to a second line via the newline action (`Alt+Enter`/`InsertNewline`), in src/app.rs (FR-008; [contracts/input-modes.md](contracts/input-modes.md) §1)
- [X] T021 [US2] Transition back to `Norm` when a `Mult` buffer is deleted down to a single line, in src/app.rs (FR-012)
- [X] T022 [US2] Add the stashed-draft field to `InputHistory` and the edge-recall entry points (stash the passed-in draft on the first older-step; return the stash when stepping newer past the newest entry; `Down` never recalls older), with unit tests, in src/input/mod.rs (FR-010/FR-011; research R3; [contracts/input-modes.md](contracts/input-modes.md) §3)
- [X] T023 [US2] Extend the mode-aware `Up`/`Down` (T009) so that in `Mult`/`Laat`, `Up` on the first line performs chat-style edge recall (stash draft + recall older) and `Down` past the newest entry restores the stash, in src/app.rs (FR-010; [contracts/input-modes.md](contracts/input-modes.md) §2/§3)
- [X] T024 [US2] Ensure leaving `Mult` via `Esc Esc`, submit, or push returns to `Norm`, and that plain `Enter` (and a multi-line selection + `Enter`) submits the whole buffer as one combined submission, in src/app.rs (FR-013/FR-014/FR-017)

**Checkpoint**: `Mult` editing is natural and lossless; US1 and US2 both work
independently.

---

## Phase 5: User Story 3 - `/save` and `/filter` the previous command's output (Priority: P2)

**Goal**: `/save <file>` writes the previous block's exact output (with a no-path
message, an overwrite prompt, and a missing-block message); `/filter <cmd>` pipes
the previous output through `<cmd>` via the shell and chains.

**Independent Test**: Run a command, `/save out.txt` (file equals the output),
`/save out.txt` again (prompt `[O]verwrite, [A]ppend, [C]ancel`); run multi-line
output, `/filter rg <pat>` (new `/filter rg <pat>` block of matches), then a second
`/filter` chained on the first; a no-match `/filter` shows the non-zero exit.

### Tests for User Story 3 ⚠️ (write first, must fail)

- [X] T025 [P] [US3] Write slash dispatch + path-resolution + save-content tests (S1–S7 from [contracts/slash-commands.md](contracts/slash-commands.md) §6: `/save` no-arg message, `/save` writes exact bytes, existing-file prompt path, missing-block message, `/filter` raw payload preserved, chained result becomes previous output, non-zero exit surfaced) in tests/slash_filter_save.rs

### Implementation for User Story 3

- [X] T026 [US3] Add argument-bearing `SlashCommand::Save(String)` and `Filter(String)` variants and parse their payloads (verb match + trimmed remainder; empty payload still dispatches) in src/slash/mod.rs (research R6; [contracts/slash-commands.md](contracts/slash-commands.md) §1)
- [X] T027 [US3] Implement the `/save` handler — resolve the path against `App.cwd` with `~` expansion, write the previous block's exact stored output (`BlockStore::text` of the most recent sealed block); on no path show `'/save' requires path` without clearing the buffer; on missing/evicted block show `Save failed, previous buffer not found` — in src/app.rs (FR-021/FR-022/FR-024; [contracts/slash-commands.md](contracts/slash-commands.md) §2)
- [X] T028 [US3] Add the `PendingPrompt { path, bytes }` state and make `App::on_key` consume the next key first when a prompt is pending — `o/O` overwrite, `a/A` append, `c/C`/`Esc` cancel, other keys keep the prompt — for the `/save` existing-file case, in src/app.rs (FR-023; research R9; [contracts/slash-commands.md](contracts/slash-commands.md) §2)
- [X] T029 [US3] Implement the `/filter` handler — materialize the previous output to a uniquely-named temp file, submit `cat <temp> | <cmd>` to the shell with the transcript block titled `{leader}filter <cmd>` (so the result becomes the new previous output and chains), surface a non-zero exit via `last_exit` plus a `filter non-zero exit` message, and remove the temp file best-effort — in src/app.rs (FR-025/FR-026/FR-027; research R8; [contracts/slash-commands.md](contracts/slash-commands.md) §3)
- [X] T030 [US3] Add `/save` and `/filter` to `help_text` with one-line descriptions, and add a help-text test, in src/slash/builtins.rs (FR-030)

**Checkpoint**: Output save/filter (with chaining and the overwrite prompt) works
independently of the modal-input stories.

---

## Phase 6: User Story 4 - Push/pop the input buffer to run an ad-hoc command (Priority: P2)

**Goal**: A one-item stack pushes the buffer+mode (dropping to `Norm`), then pops
to restore them (including the stash and LAAT state) on the next submit.

**Independent Test**: Compose a `Mult`/`Laat` buffer, push (`Ctrl+Alt+Enter`) →
`Norm`, empty pad; run an ad-hoc command → next submit restores the buffer and mode
exactly; a second push while already pushed is a no-op.

### Tests for User Story 4 ⚠️ (write first, must fail)

- [X] T031 [P] [US4] Write push/pop snapshot tests (P1–P5 from [contracts/push-pop-stack.md](contracts/push-pop-stack.md) §5: push saves buffer+mode and drops to `Norm`, next submit — shell or slash alike — restores, second push is a no-op, `Laat` highlight/flags restored, stashed draft restored) in tests/push_pop.rs

### Implementation for User Story 4

- [X] T032 [US4] Register the `PushInput` action (stable name `push_input`, default `Ctrl+Alt+Enter`) — `Action` variant, `name`/`from_name`/`default_map`, `dispatch_action` arm, and the default in docs/keymap-defaults.toml — in src/action/mod.rs, src/app.rs, and docs/keymap-defaults.toml (per [contracts/keymap-actions.md](contracts/keymap-actions.md))
- [X] T033 [US4] Add the `InputSnapshot { buffer, cursor, mode, stash, laat }` type and an `Option<InputSnapshot>` stack field on `App` in src/app.rs (per [data-model.md](data-model.md) and [contracts/push-pop-stack.md](contracts/push-pop-stack.md) §1)
- [X] T034 [US4] Implement `App::push_input()` — save the snapshot, reset the pad to empty, set `mode = Norm`; if the slot is already occupied, no-op (one-item semantics) — in src/app.rs (FR-018/FR-020; [contracts/push-pop-stack.md](contracts/push-pop-stack.md) §2)
- [X] T035 [US4] Pop on the next `submit` after a push — **any** submitted line pops (shell command or slash command alike); run the submitted line, then restore buffer/cursor/mode/stash/laat and clear the slot — in src/app.rs (FR-019; [contracts/push-pop-stack.md](contracts/push-pop-stack.md) §3)

**Checkpoint**: Push/pop completes LAAT's failure-recovery story and works
independently.

---

## Phase 7: User Story 5 - Load a script into LAAT mode (Priority: P3)

**Goal**: `/load <file>` reads a script's lines into the LAAT buffer (one command
per line) and enters `Laat` with the first line highlighted.

**Independent Test**: Create a file with several command lines, `/load <file>` →
each line a buffer line, mode `1T`, line 1 highlighted; `/load <missing>` reports a
status message and does not enter `Laat` partially.

### Tests for User Story 5 ⚠️ (write first, must fail)

- [X] T036 [P] [US5] Write `/load` dispatch + buffer-load + enter-`Laat` tests (S8 from [contracts/slash-commands.md](contracts/slash-commands.md) §6: each line a buffer line, mode `Laat`, highlight on line 0; missing-file does not partially enter) in tests/slash_filter_save.rs

### Implementation for User Story 5

- [X] T037 [US5] Add the `SlashCommand::Load(String)` variant and parse its payload in src/slash/mod.rs (research R6; [contracts/slash-commands.md](contracts/slash-commands.md) §1)
- [X] T038 [US5] Implement the `/load` handler — resolve the path against `App.cwd` with `~` expansion, read the file's lines into the input buffer (one command per line), enter `Laat` with the first line highlighted; on a missing/unreadable file show a status message and do not enter `Laat` with a partial buffer — in src/app.rs (FR-028; [contracts/slash-commands.md](contracts/slash-commands.md) §4)
- [X] T039 [US5] Add `/load` to `help_text` and the help-text test in src/slash/builtins.rs (FR-030)

**Checkpoint**: All five user stories are independently functional.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Documentation (Constitution V, definition-of-done) and final validation.

- [X] T040 [P] Update README.md with the input modes (`norm`/`Mult`/`1T`), the `Ctrl+Alt+1`/`Ctrl+Alt+Enter` defaults, and the `/save`, `/filter`, `/load` commands
- [X] T041 [P] Update docs/usage.md with the mode labels, LAAT stepping + failure recovery, push/pop, and the three slash commands (with the `/save` overwrite prompt)
- [X] T042 [P] Update docs/setup.md with the two new keymap actions (`toggle_mult_laat`, `push_input`) and their defaults
- [X] T043 Add an "input modes & LAAT (007)" section to docs/architecture.md describing `InputMode`, `LaatState`, the push/pop snapshot, and the exit-code gating hook
- [X] T044 Update docs/specification.md with the FR-001…FR-031 behaviors (modes, LAAT gating, push/pop, `/save`/`/filter`/`/load`)
- [X] T045 Run the [quickstart.md](quickstart.md) manual validation (SC-001…007) and the final gate `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately.
- **Foundational (Phase 2)**: Depends on Setup — **BLOCKS all user stories**.
- **User Stories (Phases 3–7)**: All depend on Foundational. US1 and US2 (both P1)
  can proceed in parallel; US3, US4, US5 are independent of each other and of US1/US2.
- **Polish (Phase 8)**: Depends on the desired user stories being complete.

### User Story Dependencies

- **US1 (LAAT, P1)**: After Foundational. Enters `Laat` via the Foundational
  `Ctrl+Alt+1` toggle; does not require US2.
- **US2 (`Mult`, P1)**: After Foundational. Independent of US1 (the edge-recall
  stash is US2-only; LAAT reuses Foundational caret motion).
- **US3 (`/save`+`/filter`, P2)**: After Foundational. Independent of the modal
  stories (operates on the existing `BlockStore`).
- **US4 (push/pop, P2)**: After Foundational. Restores US1/US2 state if present but
  is independently testable on a `Mult`/`Norm` buffer.
- **US5 (`/load`, P3)**: After Foundational; enters `Laat` (reuses US1's LAAT state
  init from T013, otherwise independent).

### Within Each User Story

- The test task (T011/T019/T025/T031/T036) — and the Foundational test T006 — is
  written first and MUST fail before the implementation it covers.
- `src/app.rs` tasks within a story are sequential (same file); cross-file tasks
  (e.g. `src/slash/mod.rs` vs `src/app.rs` vs `src/ui/input_pad.rs`) may overlap.

### Parallel Opportunities

- Foundational T002 and T003 are `[P]` (different concerns in src/input/mod.rs and
  pure additions) and T010 is `[P]` (test/doc files); T004–T009 touch src/app.rs
  (and the status render) and are sequential, with the test T006 preceding T007–T009.
- Each story's test task is `[P]` and can be written while the previous story's
  implementation finishes.
- Polish T040–T042 are `[P]` (separate doc files); T043–T045 are sequential.

---

## Parallel Example: Foundational Phase

```bash
# T002 and T003 add independent pure logic to src/input/mod.rs concerns:
Task: "Add InputMode enum + label() with unit tests (T002)"
Task: "Add InputPad vertical caret motion with unit tests (T003)"
# T010 extends test/doc files independently:
Task: "Add toggle_mult_laat default + keymap sync/listing tests (T010)"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Phase 1: Setup (green baseline).
2. Phase 2: Foundational (modes, status label, caret motion, `Ctrl+Alt+1`).
3. Phase 3: US1 LAAT — step, gate, flag, recover, render.
4. **STOP and VALIDATE**: step through a multi-line buffer with exit-code gating.
5. Demo the headline LAAT flow.

### Incremental Delivery

1. Setup + Foundational → modes exist and are visible.
2. US1 (LAAT) → headline stepping (**MVP**).
3. US2 (`Mult`) → lossless multi-line editing + edge recall.
4. US3 (`/save`+`/filter`) → work with prior output, chaining.
5. US4 (push/pop) → duck-out-and-return; completes LAAT recovery.
6. US5 (`/load`) → load scripts into LAAT.
7. Polish → docs + quickstart + gate.

Each story adds value without breaking the previous ones; run the gate at every
checkpoint.

---

## Notes

- **Tests requested**: yes (Constitution III). New suites: tests/input_modes.rs,
  tests/laat_engine.rs, tests/push_pop.rs, tests/slash_filter_save.rs; the
  tests/keymap_*.rs suites are extended for the two new defaults.
- **No new dependencies**: the 006 key-string parser already accepts `Ctrl+Alt+1`
  and `Ctrl+Alt+Enter`; `/filter` uses `std::env::temp_dir` + the wrapped shell.
- **Out of scope** (do not implement): per-mode `[keymap.laat]`/`[keymap.mult]`
  config sections; cross-session LAAT-buffer persistence.
- **Total**: 45 tasks — Setup 1, Foundational 9, US1 8, US2 6, US3 6, US4 5, US5 4,
  Polish 6.
