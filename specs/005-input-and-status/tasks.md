# Tasks: Input Editing & Fixed Status Bar

**Input**: Design documents from `/specs/005-input-and-status/`
**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/)

**Tests**: INCLUDED — Constitution III (TDD) is mandatory for this repo. The pure
logic (motion/selection/kill, paste split, page-minus-context, status fit/truncate,
Esc/selection arbitration) is unit-tested **before** implementation. Live-TTY behavior
(paste round-trip, status render/resize/hide, key feel) is covered by the manual
[quickstart.md](quickstart.md) per the documented integration/manual exception.

**Organization**: Tasks are grouped by user story. US1 + US2 are both P1 (the MVP).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1–US5 for user-story tasks; Setup/Foundational/Polish carry no label
- All paths are repository-root-relative; gate = `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test`

## Path Conventions

Single Rust crate: `src/`, `tests/` at repository root (per [plan.md](plan.md) Project Structure).

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Confirm a green baseline and scaffold the new modules so the crate compiles before any logic lands.

- [X] T001 Confirm the baseline gate is green on `005-input-and-status` (run `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test`) so all later failures are attributable to this feature.
- [X] T002 Create empty module skeletons and register them so the crate compiles: `src/input/editing.rs`, `src/input/selection.rs`, `src/action/mod.rs` (add `mod editing;`/`mod selection;` to `src/input/mod.rs` and `pub mod action;` to `src/lib.rs`).

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The named-action registry, config surface, and slash-registry shape that multiple user stories bind against.

**⚠️ CRITICAL**: US1/US3/US5 resolve their key chords through the registry, and US3/US4 read the new config keys — these must exist first.

- [X] T003 Define the named-action model in `src/action/mod.rs`: an `Action` enum with one variant per mapped action (data-model §3 table) plus the reserved-but-unmapped `MultilineMoveStartBuffer`/`MultilineMoveEndBuffer` (FR-009), and a `KeyChord { code, mods }` descriptor.
- [X] T004 Implement the registry `default_bindings() -> &'static [(Action, KeyChord, &'static str)]`, `resolve(chord) -> Option<Action>`, and `listing() -> Vec<(String, String)>` in `src/action/mod.rs`, then write the registry unit tests FIRST (assert: every **mapped** action appears exactly once, no two share a chord, the two reserved actions **and** `ClearStatusMessage` have no entry and `resolve` never returns them, `listing()` is stable-ordered and includes `ClearStatusMessage`) — see [contracts/input-editing.md](contracts/input-editing.md) §4. (depends on T003)
- [X] T005 Add the config surface in `src/config.rs`: new `[status] enabled` (bool, default `true`) and `scroll.context_lines` (u16, default `3`); update `TOP_LEVEL_KEYS`/`SCROLL_KEYS` and add `STATUS_KEYS`, the `Status` struct + `Default`, and `Scroll.context_lines`; write unit tests FIRST asserting defaults, that existing keys still parse (FR-033), and unknown-key logging is unchanged — see [contracts/status-bar.md](contracts/status-bar.md#config).
- [X] T006 Extend the slash registry shape in `src/slash/mod.rs`: add `SlashCommand::{Status, Keys}` and map `"status"`/`"keys"` in `dispatch`; write a unit test FIRST asserting the two new strings resolve and existing `help`/`clear`/`quit`/`exit` are unchanged (FR-032) — see [contracts/slash-commands.md](contracts/slash-commands.md).

**Checkpoint**: Registry, config, and slash shape exist — user stories can begin.

---

## Phase 3: User Story 1 - Shell-grade line editing (Priority: P1) 🎯 MVP

**Goal**: Word/line motion, keyboard selection, and `Ctrl+U`/`Ctrl+K`/`Ctrl+W` kills in the input pad, current-line-scoped in both single- and multi-line buffers.

**Independent Test**: Type a line, exercise `Home`/`End`, `Ctrl+Left/Right`, `Shift[+Ctrl]+arrows`, `Ctrl+U/K/W`; add a newline and confirm identical behavior on the second line (quickstart US1, SC-001).

### Tests for User Story 1 (write first, ensure they FAIL)

- [X] T007 [P] [US1] Unit tests for motion + kill in `tests/input_editing.rs`: punctuation-aware `word_boundary_left/right` (stops at `.` in `foo.bar`), `line_move_start/end` on the current line, `kill_to_line_start/end` within the current line, `delete_word_before` (readline whitespace rule), each verified in BOTH single- and multi-line buffers (FR-001/002/005/006/007) — see [contracts/input-editing.md](contracts/input-editing.md) §1–2.
- [X] T008 [P] [US1] Unit tests for input selection in `tests/input_selection.rs`: `select_char_left/right` and `select_word_left/right` anchor then extend, `range()` normalizes, `selected_text()` returns the span, word-wise uses the punctuation-aware boundary (FR-003/FR-004) — see [contracts/input-editing.md](contracts/input-editing.md) §3.

### Implementation for User Story 1

- [X] T009 [P] [US1] Implement the pure scanners in `src/input/editing.rs`: `word_boundary_left/right` (whitespace/word/punctuation classes), `delete_word_before_start` (whitespace rule), `current_line_bounds` (FR-002/006/007).
- [X] T010 [P] [US1] Implement `InputSelection { anchor, caret }` with `range()`/`is_empty()` in `src/input/selection.rs` (FR-003/FR-004).
- [X] T011 [US1] Add the current-line ops to `InputPad` in `src/input/mod.rs`: `line_move_start/end`, `word_move_left/right`, `kill_to_line_start/end`, `delete_word_before`, `select_char_left/right`, `select_word_left/right`, `cancel_selection`, `selected_text`, holding the `selection: Option<InputSelection>` field (depends on T009, T010).
- [X] T012 [US1] Wire the US1 actions in `src/app.rs` `on_key` via `action::resolve(chord)` (Home/End, Ctrl+Left/Right, Shift[+Ctrl]+arrows, Ctrl+U/K/W) (FR-008; depends on T011, T004).
- [X] T013 [US1] Render the input-pad selection highlight in `src/ui/input_pad.rs` (mirror the 004 transcript highlight).

**Checkpoint**: Input line editing is fully functional and unit-tested.

---

## Phase 4: User Story 2 - Multi-line paste lands as one buffer (Priority: P1) 🎯 MVP

**Goal**: A bracketed paste inserts as one editable multi-line buffer that never auto-submits; only `Enter` submits the whole buffer.

**Independent Test**: Paste a 3-line block — it lands as one buffer, nothing submits, caret at end, then `Enter` submits all of it (quickstart US2, SC-002).

### Tests for User Story 2 (write first, ensure they FAIL)

- [X] T014 [P] [US2] Unit tests in `tests/input_paste.rs` for `insert_paste`: splits on `\n` into buffer lines (normalizing `\r\n`/`\r`), caret rests at end of inserted content, splices correctly mid-line into a non-empty buffer, empty paste is a no-op, trailing-newline preserved (FR-010/FR-012) — see [contracts/input-paste.md](contracts/input-paste.md).

### Implementation for User Story 2

- [X] T015 [US2] Implement `InputPad::insert_paste(&str)` in `src/input/mod.rs` (split/normalize/splice, caret to end) (FR-010/FR-012; depends on T014).
- [X] T016 [US2] Enable bracketed paste in terminal setup and add `DisableBracketedPaste` to BOTH the normal exit path and the panic guard in `src/lib.rs`, alongside the existing raw-mode/mouse/alt-screen teardown (FR-010, Constitution VI) — see [contracts/input-paste.md](contracts/input-paste.md).
- [X] T017 [US2] Add the `Event::Paste(text) => self.input.insert_paste(&text)` arm to the `src/app.rs` event loop so pasted newlines never reach the submit arm (FR-011; depends on T015, T016).

**Checkpoint**: Multi-line paste is safe and editable; MVP (US1 + US2) complete.

---

## Phase 5: User Story 3 - Scrollback polish with retargeted keys (Priority: P2)

**Goal**: Context-preserving page scroll, line-granular `Shift+PageUp/Down`, `Shift+Home/End` jumps; `Home`/`End` now edit the input line.

**Independent Test**: With long output, `PageUp`/`PageDown` overlap by 3 context lines (≥1 on a short pad), `Shift+PageUp/Down` move one line, `Shift+Home/End` jump top/bottom, `Home`/`End` do not scroll (quickstart US3, SC-007).

### Tests for User Story 3 (write first, ensure they FAIL)

- [X] T018 [P] [US3] Unit tests in `tests/scrollback_context.rs`: `scroll_page_up/down(context)` advance by `(viewport - context).max(1)` clamped to `[0, max_scroll]`, the **≥1-line floor** on a short pad, line scroll, and top/bottom (FR-013/FR-014/FR-015/FR-016) — see [contracts/scrollback.md](contracts/scrollback.md).

### Implementation for User Story 3

- [X] T019 [US3] Extend the `Transcript` scroll API in `src/session/mod.rs`: `scroll_page_up/down(context_lines)` with the `(viewport - context).max(1)` formula and expose `scroll_line_up/down` aliases (FR-013/FR-014/FR-015; depends on T018).
- [X] T020 [US3] Retarget bindings in `src/app.rs` `on_key` via the registry: `Home`/`End` → input line motion (NOT scroll), `Shift+Home/End` → `scroll_to_top/bottom`, `PageUp/Down` → context page, `Shift+PageUp/Down` → one line (FR-013/FR-016/FR-017; depends on T019, T004, T012).

**Checkpoint**: Scrollback keys retargeted; input motion and scroll no longer conflict.

---

## Phase 6: User Story 4 - Fixed-format status bar (Priority: P2)

**Goal**: A fixed `mode | cwd<greedypad>| message | exit` bar beneath the input, on by default, `/status`-toggleable, auto-hidden below 10 rows.

**Independent Test**: The bar renders in the fixed layout; failing commands show the exit code; narrowing truncates message-then-cwd without wrapping; `/status` toggles; below 10 rows it hides and reappears at ≥10 (quickstart US4, SC-005).

### Tests for User Story 4 (write first, ensure they FAIL)

- [X] T021 [P] [US4] Unit tests in `tests/status_bar.rs` for the pure `fit(width, mode, cwd, message, exit) -> String`: the fixed `mode | cwd<greedypad>| message | exit` layout (greedy pad between cwd and the `|`, no `|` immediately after cwd), default mode label `norm`, message right-justified, truncation order message(ellipsis)→cwd(middle-ellipsis), `mode`/`exit` never broken and output never exceeds width / wraps; plus the `<10`-row hide predicate (FR-019/FR-020/FR-021/FR-024) — see [contracts/status-bar.md](contracts/status-bar.md).

### Implementation for User Story 4

- [X] T022 [US4] Implement `fit(...)` and grow `src/ui/status.rs` into the fixed below-input bar, folding the existing above-input status *rule*'s cwd/exit/message into it and removing the old rule (research R3/R4; FR-018/FR-019/FR-020/FR-023; depends on T021).
- [X] T023 [US4] Update layout in `src/ui/mod.rs` to reserve one row beneath the input pad for the bar and apply the visibility predicate `enabled && rows >= 10` (FR-018/FR-021; depends on T022, T005).
- [X] T024 [US4] Wire status state in `src/app.rs`: `status_enabled` (from `config.status.enabled`), the constant `norm` mode (4-char), `cwd`, and `exit` (most-recent completed command) feeding the bar (FR-020/FR-023; depends on T023).
- [X] T025 [US4] Handle `SlashCommand::Status` in `src/app.rs` to toggle `status_enabled` and post a confirmation message (FR-022; depends on T024, T006).

**Checkpoint**: Status bar renders, toggles, truncates, and hides correctly.

---

## Phase 6b: Chrome polish — dividing rule restoration & scroll fix (post-smoke-test)

**Goal**: Restore the cosmetic dividing rule (Apollo / Domain OS lineage) removed when the status bar replaced the above-input rule, made configurable; fix the line-scroll over-scroll found in the US3/US4 smoke test (walkthrough item 14).

- [X] T035 Restore the dividing rule between the output and input pads as a configurable element (default on): add `[divider] enabled` to `src/config.rs` (mirrors `[status]`), a `src/ui/divider.rs` renderer (full-width `─`), generalize `src/ui/mod.rs` layout into `chrome_layout(area, input_height, divider, status) -> ChromeLayout` and account for the divider row in `src/app.rs` `sync_size`; tests in `tests/config.rs` (default + parse + unknown key) and `tests/chrome.rs` (full chrome ordering transcript→divider→input→status; omitted when disabled). Purely cosmetic now (kwi #47 may fold the prompt into it later).
- [X] T036 Fix line-scroll over-scroll (smoke test item 14): clamp `Transcript::scroll_up` to `max_scroll` in `src/session/mod.rs` so scrolling up past the oldest line no longer inflates the offset (which previously required extra `Shift+PageDown` presses to unwind); regression test in `tests/scrollback_context.rs` (`line_scroll_up_clamps_at_the_top`).

**Checkpoint**: Dividing rule visible by default and toggleable via config; line scroll pins cleanly at the top.

---

## Phase 7: User Story 5 - Message lifetime & selection arbitration (Priority: P3)

**Goal**: At most one selection across both pads; `Ctrl+C` copies-or-interrupts; `Esc`/`Esc Esc` cancel→clear-line→clear-buffer; status message persists until submit or double-Esc (no timeout).

**Independent Test**: Selections in the two pads are mutually exclusive; `Ctrl+C` copies with a selection and SIGINTs without; `Esc`/`Esc Esc` follow the rule; a message persists across non-submits and clears on Enter or `Esc Esc` (quickstart US5, SC-003/SC-004/SC-006).

### Tests for User Story 5 (write first, ensure they FAIL)

- [X] T026 [P] [US5] Unit tests in `tests/input_selection.rs` (extend) for the `ActiveSelection` arbiter and gestures: starting a selection in one pad clears the other (FR-027); `Ctrl+C` copies+clears with a selection, SIGINT path with none (FR-028); `Esc` cancel→clear-line, multi-line `Esc Esc` clears the whole buffer (FR-029); message set→persists-across-non-submit→clears on submit and on `Esc Esc`, never on a timeout (FR-025/FR-026) — driven by key sequences (no timers).

### Implementation for User Story 5

- [X] T027 [US5] Introduce the `ActiveSelection { None, Input(InputSelection), Transcript(..) }` arbiter in `src/app.rs`, making "at most one selection" structural; starting a selection in either pad clears the other (FR-027; depends on T012, T013).
- [X] T028 [US5] Update `Ctrl+C` and `Esc` handling in `src/app.rs` `on_key`: `Ctrl+C` copy-and-clear with a selection else SIGINT (FR-028); `Esc` cancel-selection / clear-current-line, multi-line `Esc Esc` clears the whole buffer via an `esc_pending` keypress flag (no timer) (FR-029; depends on T027).
- [X] T029 [US5] Implement status-message lifetime in `src/app.rs` (reuse `App.notice` as `message`): cleared on `Enter` submit and on `Esc Esc` (`clear_status_message`), never on a timeout (FR-025/FR-026; depends on T028, T024).

**Checkpoint**: All five user stories independently functional.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Discoverability commands, documentation, and full validation.

- [X] T030 Handle `SlashCommand::Keys` in `src/app.rs` (render `action::listing()` into the transcript) and add the one-line `/keys` pointer to `help_text` in `src/slash/builtins.rs`; add `tests/slash_status_keys.rs` asserting `/keys` lists every mapped binding and `/help` contains the pointer (FR-030/FR-031; depends on T004, T006).
- [X] T034 (kwi #46) Strip **trailing** whitespace-only lines from a multiline buffer on submit (interior blanks preserved; single-line unchanged) in `src/app.rs` `submit()`/`InputPad::take_submit`; add `tests/input_submit_trim.rs` covering trailing-blank removal, interior-blank preservation, and single-line no-op. **Default behavior only** — the `suppress_multiline_whitespace` / `suppress_multiline_trailing_whitespace_lines` config knobs are deferred (kwi #46). Surfaced in walkthrough item 10 (`.scratch/005-us1-us2-walkthrough.md`).
- [X] T031 [P] Update documentation as definition-of-done (Constitution V): new keys + status bar in `README.md`; key table, `/status`, `/keys`, and the new config keys in `docs/usage.md`; `status.enabled`/`scroll.context_lines` in `docs/setup.md`; the input-editing + status-bar design in `docs/architecture.md`; and `docs/specification.md`.
- [X] T032 Run the full gate (`cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test`) and resolve any failures (FR-032, SC-009).
- [X] T033 Execute [quickstart.md](quickstart.md) on a live TTY (all 27 steps), confirming SC-001…009 and no Constitution VI failure (flicker/reflow/wrap/lost output/stranded terminal mode after exit or crash).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately.
- **Foundational (Phase 2)**: Depends on Setup — **blocks all user stories** (registry, config, slash shape).
- **User Stories (Phase 3–7)**: All depend on Foundational. US1 + US2 are the P1 MVP.
- **Polish (Phase 8)**: Depends on all desired user stories.

### User Story Dependencies

- **US1 (P1)**: After Foundational. Independent.
- **US2 (P1)**: After Foundational. Independent of US1 (different files: `lib.rs`/`app.rs` paste arm/`insert_paste`).
- **US3 (P2)**: After Foundational. Its `app.rs` binding retarget (T020) touches `on_key` and so should land after US1's `on_key` wiring (T012) to avoid conflicts.
- **US4 (P2)**: After Foundational. Largely independent (`ui/`, `config`), with `app.rs` status state.
- **US5 (P3)**: After US1 (needs the input selection from T012/T013) and US4 (message lifetime ties to the bar, T024).

> **Shared-file note**: `src/app.rs` `on_key` is touched by US1, US3, US4, US5. Those
> `app.rs` tasks are **not** parallel with each other — prefer sequential priority
> order (US1 → US2 → US3 → US4 → US5). The pure logic in `src/input`, `src/session`,
> `src/ui`, and the tests **are** parallelizable.

### Within Each User Story

- Tests are written FIRST and must FAIL before implementation (Constitution III).
- Pure helpers (`editing.rs`, `selection.rs`, `fit`) before the `InputPad`/`app.rs` wiring.
- `app.rs` `on_key` wiring last within a story.

### Parallel Opportunities

- T007/T008 (US1 tests), T009/T010 (US1 pure helpers) — different files, parallel.
- T014, T018, T021, T026 — story test files, parallel with their story's setup.
- T031 (docs) is `[P]` against any remaining code-free work.

---

## Parallel Example: User Story 1

```bash
# Tests first (different files, in parallel):
Task: "T007 Unit tests for motion + kill in tests/input_editing.rs"
Task: "T008 Unit tests for input selection in tests/input_selection.rs"

# Then the pure helpers (different files, in parallel):
Task: "T009 Pure scanners in src/input/editing.rs"
Task: "T010 InputSelection in src/input/selection.rs"

# Then serial: T011 (input/mod.rs) → T012 (app.rs on_key) → T013 (ui/input_pad.rs)
```

---

## Implementation Strategy

### MVP First (US1 + US2 — both P1)

1. Phase 1 Setup → Phase 2 Foundational.
2. Phase 3 (US1) → Phase 4 (US2).
3. **STOP and VALIDATE**: quickstart US1 + US2 (SC-001, SC-002) — the daily-driver input.

### Incremental Delivery

1. Setup + Foundational → foundation ready.
2. US1 + US2 → MVP (editing + safe paste).
3. US3 → scrollback polish.
4. US4 → fixed status bar.
5. US5 → message lifetime + selection arbitration.
6. Polish → `/keys`, docs, gate, quickstart.

Each story is independently testable; the registry/config/slash foundation keeps the
forward seam for the sprint-006 keymap engine without behavioral rewrite.
