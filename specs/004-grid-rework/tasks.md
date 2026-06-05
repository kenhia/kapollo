# Tasks: Grid Rework — Native Terminal Grid, Mouse Selection & Block Store

**Input**: Design documents from `/specs/004-grid-rework/`
**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/)

**Tests**: INCLUDED. Constitution III mandates TDD (Red-Green-Refactor). Pure helpers
(block-store eviction, selection FSM, coord math, clipboard framing, grid-render mapping)
are unit/contract-tested first; live-shell/TTY paths use the documented integration-test
exception and are validated by the manual [quickstart.md](quickstart.md).

**Organization**: Tasks are grouped by user story (US1 P1 → US2 P2 → US3 P3) for
independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1 / US2 / US3 (Setup, Foundational, Polish have no story label)
- Exact file paths are included in each task

## Path Conventions

Single Rust crate: `src/`, `tests/` at repository root. Module tree per [plan.md](plan.md)
("Source Code") — 🔧 reworked, ✨ new, unmarked kept.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Architecture-first record + dependency wiring before the core diverges.

- [X] T001 Update [docs/architecture.md](../../docs/architecture.md) to record the grid backend (D25/D27), block-as-annotation-over-grid (D29), retained block-store text superseding D29's reconstruct lean (R3), and mouse/alt-screen routing (D28) — Constitution II gate, MUST land before implementation diverges
- [X] T002 Add the decisions-log entry recording R3 **supersedes D29** (retained block-store text is canonical) in the project decisions ledger
- [X] T003 Add dependencies in [Cargo.toml](../../Cargo.toml): `wezterm-term` git-pinned `rev = "577474d89ee61aef4a48145cdec82a638d874751"`, bump `ratatui` 0.29→0.30, `crossterm` 0.28→0.29, `portable-pty` 0.8→0.9, add `base64 0.22` and `arboard 3.6`; keep `vte 0.13` for the OSC 133/7 side-tap
- [X] T004 Run `cargo build` and resolve any `termwiz`/`unicode-width`/`bitflags` version conflicts from the wezterm tree; commit the updated [Cargo.lock](../../Cargo.lock) (reproducibility rests on the pinned rev + lockfile)
- [X] T005 [P] Confirm code-standards gate is green after the dep bump: `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test` on Rust 1.96.0

**Checkpoint**: Architecture recorded, deps resolved, gate green — core work can begin.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Module scaffolding and the promoted spike helpers that ALL stories build on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T006 [P] Create new module skeletons and wire into [src/lib.rs](../../src/lib.rs): `src/grid/{mod.rs,render.rs}`, `src/selection/{mod.rs,coords.rs}`, `src/clipboard.rs`, `src/session/store.rs`
- [X] T007 [P] Promote the spike coord helpers from `spike-support::coords` into [src/selection/coords.rs](../../src/selection/coords.rs) (content↔screen mapping, wide-cell aware), keeping their unit tests
- [X] T008 [P] Promote the spike clipboard helpers from `spike-support::clipboard` into [src/clipboard.rs](../../src/clipboard.rs) (`osc52_frame`, local `arboard` fallback, configurable order), keeping their unit tests
- [X] T009 [P] Promote the spike `modes` mode-detection tap from `spike-support::modes` into the output side-tap path in [src/output/parser.rs](../../src/output/parser.rs) (`?1049`, `?1000/1002/1003/1006` h/l), keeping its unit tests
- [X] T010 Extend config in [src/config.rs](../../src/config.rs) for the new surface (mouse enable, selection/copy behavior, clipboard path + fallback order, scroll/scrollback) WITHOUT breaking existing keys (FR-026)

**Checkpoint**: Foundation ready — user stories can proceed (US1 first as MVP).

---

## Phase 3: User Story 1 - Native terminal rendering of the main screen (Priority: P1) 🎯 MVP

**Goal**: Render the shell's main screen as a real terminal grid (in-place redraws, inline
color/attrs, wide/combining chars, bounded scrollback, alt-screen) inside the transcript
pane, keeping the input-pad-at-bottom chrome.

**Independent Test**: Run a `\r` progress counter, a colorized command, a wide/CJK output,
and launch `htop`/`vim` — progress updates one line, colors/attrs render, columns don't
drift, alt-screen renders and restores cleanly (quickstart steps 1–3, 10).

### Tests for User Story 1 (write first, ensure they FAIL) ⚠️

- [X] T011 [P] [US1] Grid-render contract tests in [tests/grid_render.rs](../../tests/grid_render.rs): `"a\rb"` yields one row `b…` (in-place CR), SGR → ratatui `Modifier`/`Color`, wide char advances 2 cols with empty continuation, per [contracts/grid-render.md](contracts/grid-render.md) (FR-002/003, SC-002)
- [X] T012 [P] [US1] Scrollback-window + alt-screen tests in [tests/grid_render_scroll.rs](../../tests/grid_render_scroll.rs): `viewport_rows(n)` window + clamp at top, enter/leave alt-screen restores prior viewport, `changed_rows()` reports a 1-row range after a single update

### Implementation for User Story 1

- [X] T013 [US1] Implement the `Grid` wrapper over `wezterm-term` in [src/grid/mod.rs](../../src/grid/mod.rs): `advance_bytes`, `resize`, `stable_row_at`, `changed_rows`, `is_alt_screen_active`, `is_mouse_grabbed`, `cursor` (data-model "Grid"/"Scrollback"/"Cell")
- [X] T014 [US1] Implement grid→ratatui span mapping in [src/grid/render.rs](../../src/grid/render.rs): `rows_to_lines` mapping Cell fg/bg/attrs → `Style`, wide-cell handling, scrollback windowing (FR-002/003/004)
- [X] T015 [US1] Re-point the output feed in [src/output/mod.rs](../../src/output/mod.rs) and [src/output/parser.rs](../../src/output/parser.rs) to drive `Grid::advance_bytes`, reducing the hand-rolled escape application to the OSC 133/7 + mode side-tap only (FR-001, R7)
- [X] T016 [US1] Replace the append-only render in [src/ui/transcript.rs](../../src/ui/transcript.rs) with grid rendering + incremental redraw via `changed_rows` (FR-001/004, SC-001/003)
- [X] T017 [US1] Wire scrollback navigation (scroll offset, follow-tail) and resize→`Grid::resize` into the event loop in [src/app.rs](../../src/app.rs), keeping input-pad/status chrome (FR-004/006)
- [X] T018 [US1] Implement alt-screen switch rendering + clean restore in [src/ui/transcript.rs](../../src/ui/transcript.rs) and [src/app.rs](../../src/app.rs), ensuring no alt-screen content leaks into scrollback (FR-005, SC-003)
- [X] T019 [US1] Verify TUI integrity for the render path in [src/app.rs](../../src/app.rs): off-screen logs, panic boundary restores terminal, clean teardown on exit (FR-027, SC-009)

**Checkpoint**: US1 is independently demonstrable — kapollo renders like a native terminal.

---

## Phase 4: User Story 2 - Mouse selection, copy & scroll with app hand-over (Priority: P2)

**Goal**: Click-drag selection with a content-anchored highlight, OSC 52 copy with local
fallback, wheel/PageUp-Down scrollback, correct mouse hand-over to full-screen apps, and
Shift bypass — clearing the selection on command submit.

**Independent Test**: Click-drag selects with a real-time highlight; copy round-trips
exactly (incl. over SSH and under flood); wheel scrolls history; `vim` mouse mode receives
clicks; Shift-drag uses host native selection (quickstart steps 4–13).

### Tests for User Story 2 (write first, ensure they FAIL) ⚠️

- [X] T020 [P] [US2] Selection FSM + coord tests in [tests/selection_coords.rs](../../tests/selection_coords.rs): down→drag→up yields an `Active` range, bare click yields no selection, `on_command_submit` clears (FR-010/017), stable-row anchoring no-drift, exact char-for-char selected range (SC-004), per [contracts/mouse-selection.md](contracts/mouse-selection.md)
- [X] T021 [P] [US2] Routing-table + clipboard tests in [tests/selection_routing.rs](../../tests/selection_routing.rs): each (shift, alt_screen, child_mouse) combo → expected `Routed`; `detect_mode` flips child_mouse/alt_screen; `osc52_frame` round-trips; copy tries primary then fallback; multi-row join has no off-by-one (SC-004)

### Implementation for User Story 2

- [X] T022 [US2] Implement the `SelectionController` FSM in [src/selection/mod.rs](../../src/selection/mod.rs) (idle→dragging→active→idle) anchored to `StableRowIndex`, with `on_command_submit` clear (FR-007/008/010/017, R6)
- [X] T023 [US2] Implement mouse routing in [src/input/router.rs](../../src/input/router.rs): Shift→bypass, alt_screen/child_mouse→ToChild, else→Consumed (selection/scroll), per the routing table (FR-015/016)
- [X] T024 [US2] Render the selection highlight overlay in [src/ui/transcript.rs](../../src/ui/transcript.rs) aligned to anchored rows (FR-007), and auto-scroll-extend on edge drag (FR-009)
- [X] T025 [US2] Wire wheel + PageUp/PageDown scrollback navigation and selection-survives-scroll into [src/app.rs](../../src/app.rs) (FR-014)
- [X] T026 [US2] Implement copy-on-selection in [src/app.rs](../../src/app.rs) using [src/clipboard.rs](../../src/clipboard.rs): right-click/Ctrl-C with active selection copies + clears; Ctrl-C with no selection sends SIGINT; visible notice on copy failure (FR-011/012/013, SC-005)
- [X] T027 [US2] Fold [src/ui/passthrough.rs](../../src/ui/passthrough.rs) into the routing path so full-screen/inner-mouse apps receive forwarded input and kapollo selection suspends/resumes cleanly (FR-015, SC-003)
- [X] T028 [US2] Clear active selection on command submit at the submit site in [src/app.rs](../../src/app.rs) (FR-017 — resolves the spike flood-overrun caveat, SC-006)

**Checkpoint**: US1 + US2 both work — native rendering plus mouse selection/copy/scroll.

---

## Phase 5: User Story 3 - Block store over the grid (Priority: P3)

**Goal**: Capture each command as a block (command + output + exit code) anchored to grid
rows, retained in an in-memory store behind a single text accessor (DB-ready seam),
preserving `/save`/`/filter`/exit-code chrome and adding block-aware copy.

**Independent Test**: Run commands, `/save` the last (output matches exactly), `/filter`
honors boundaries/exit codes, right-click offers copy-with/without-command; evicted blocks
report "unavailable" (quickstart steps 14–15).

### Tests for User Story 3 (write first, ensure they FAIL) ⚠️

- [X] T029 [P] [US3] Block-store contract tests in [tests/block_store.rs](../../tests/block_store.rs): begin→set_start_row→seal yields Finished block w/ exit + row range; `text_with_command` == command+"\n"+text; `duration` is `None` before seal and `Some(ended_at−started_at)` after; `cap+1` evicts oldest (id → `None` everywhere); `block_at_row` maps/`None`, per [contracts/block-store.md](contracts/block-store.md)
- [X] T030 [P] [US3] Seam + survival tests in [tests/block_store_seam.rs](../../tests/block_store_seam.rs): a stub `BlockText` impl passes the same accessor tests (proves SC-010); a block whose grid rows are evicted still returns `text` (store eviction ⊥ grid eviction, R3)

### Implementation for User Story 3

- [X] T031 [US3] Implement the `BlockText` accessor seam + `Block` fields (`row_range`, `cwd`, `started_at`, `ended_at`, `state`, `available`, `text()`, `text_with_command()`, `duration()`) in [src/session/block.rs](../../src/session/block.rs), reusing `OutputBuffer` as the bounded retainer (FR-018/019, data-model "Block")
- [X] T032 [US3] Implement `BlockStore` in [src/session/store.rs](../../src/session/store.rs): `begin`/`set_start_row` (stamps `started_at`)/`seal` (stamps `ended_at`)/`get`/`block_at_row`/`text`/`text_with_command`/`duration`, bounded eviction, `index_by_row`, designed-for `SecondaryStore` seam (FR-019/020/025, SC-010)
- [X] T033 [US3] Drive block boundaries from the OSC 133 `A/B/C/D` + OSC 7 side-tap (with sentinel fallback) into the store in [src/session/mod.rs](../../src/session/mod.rs), anchoring row ranges to `StableRowIndex`: `B`→`begin`, `C`→`set_start_row` (stamps `started_at`), `D`→`seal` (stamps `ended_at`) (FR-018/029, R7)
- [~] T034 [US3] **DEFERRED → kwi WI #43** (user decision 2026-06-04): `/save` was never implemented (declared post-MVP in 001; dispatcher only has help/clear/quit), so there is nothing to "re-point". The store seam (`BlockStore::text`/`text_with_command`/`block_at_row`) is ready; `/save` is net-new and underspecified (no file-target design). Re-point `/save` to the store accessor in [src/slash/builtins.rs](../../src/slash/builtins.rs): saved content = `block.text()` (faithful, not re-scrape); evicted → explicit "unavailable" (FR-021/025, SC-007)
- [~] T035 [US3] **DEFERRED → kwi WI #44** (user decision 2026-06-04): `/filter` was never implemented (declared post-MVP in 001) and its interactive UX is unspecified — likely depends on the wanted-later popup UI. Re-point `/filter` to operate over store block boundaries + exit codes with no regression in [src/slash/builtins.rs](../../src/slash/builtins.rs) (FR-022, SC-008)
- [X] T036 [US3] Reflect block exit status (and optionally elapsed `duration()`) in transcript chrome over the grid in [src/ui/transcript.rs](../../src/ui/transcript.rs) / [src/ui/status.rs](../../src/ui/status.rs) (FR-023, SC-008)
- [X] T037 [US3] Add block-aware copy affordances in [src/app.rs](../../src/app.rs) using `block_at_row`: copy block output with command, without command, and current line (FR-024, data-model US3-5)

**Checkpoint**: All three stories work — grid + selection + block store, MVP complete.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Regression-proofing, docs, and final integrity verification.

- [X] T038 [P] Update [README.md](../../README.md), [docs/setup.md](../../docs/setup.md), and [docs/usage.md](../../docs/usage.md) with new mouse/selection/clipboard/scroll keybindings and config keys (Constitution V)
    - user addition: Create a `licenses` folder and include copy of the wezterm license (`https://github.com/wezterm/wezterm/blob/main/LICENSE.md`) — done: [licenses/wezterm-LICENSE.md](../../licenses/wezterm-LICENSE.md)
- [X] T039 [P] Update [docs/specification.md](../../docs/specification.md) to reflect the grid model and block store (Constitution I)
- [X] T040 Confirm no regression in existing slash commands and shell-wrapping; ensure [tests/shell_parity.rs](../../tests/shell_parity.rs) still passes (FR-028) — verified green in the final gate
- [X] T041 **MANUAL (live TTY)** Ran [quickstart.md](quickstart.md) — steps 1–13 PASS (no flicker/drift/off-by-one/stuck-capture; `/quit` restored the terminal cleanly; `/help` prompt redraw aligned). Steps 14–15 deferred with `/save` (kwi WI #43); the store/seam they target is covered by `tests/block_store_seam.rs`. Record in PR description.
- [X] T042 Final code-standards gate: `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test` green on 1.96.0; verify CI runs green (fish-install fix already on branch) — local gate green (69 lib + all integration suites)

---

## Dependencies & Execution Order

```text
Phase 1 (Setup) ─▶ Phase 2 (Foundational) ─▶ Phase 3 (US1/P1, MVP)
                                                   │
                                                   ├─▶ Phase 4 (US2/P2) ─┐
                                                   └─▶ Phase 5 (US3/P3) ─┤
                                                                         ▼
                                                              Phase 6 (Polish)
```

- **Setup (T001–T005)**: T001 (architecture) gates everything (Constitution II). T003→T004→T005 sequential (deps→lock→gate).
- **Foundational (T006–T010)**: T006 before T007/T008/T009 (skeletons first); T007/T008/T009 are `[P]` (different files). Blocks all stories.
- **US1 (T011–T019)**: depends on Foundational. The MVP. Tests T011/T012 first. T013 before T014–T018; T019 last.
- **US2 (T020–T028)**: depends on US1's grid (`StableRowIndex`, render). Tests T020/T021 first. T022 before T023–T028.
- **US3 (T029–T037)**: depends on US1's grid rows for anchoring; complements US2. Tests T029/T030 first. T031 before T032; T033 before T034–T037.
- **Polish (T038–T042)**: after the stories it documents/verifies.

### Story independence

- **US1** is the standalone MVP: native rendering needs nothing from US2/US3.
- **US2** needs US1's grid (cells + stable rows) but not US3.
- **US3** needs US1's grid rows to anchor blocks; uses US2's `block_at_row` only for the
  block-aware copy affordance (T037) — the store/`/save`/`/filter` work stands without US2.

---

## Parallel Execution Examples

- **Setup**: T005 runs after T004; T001/T002 (docs/ledger) can run alongside T003 dep work.
- **Foundational**: after T006, run T007 + T008 + T009 in parallel (separate files), then T010.
- **US1 tests**: T011 + T012 in parallel (separate files `tests/grid_render.rs` + `tests/grid_render_scroll.rs`) before implementation.
- **US2 tests**: T020 + T021 in parallel (`tests/selection_coords.rs` + `tests/selection_routing.rs`).
- **US3 tests**: T029 + T030 in parallel (`tests/block_store.rs` + `tests/block_store_seam.rs`).
- **Polish**: T038 + T039 (separate doc files) in parallel.

---

## Implementation Strategy

1. **MVP first**: Complete Phase 1 → 2 → 3 (US1). Ship/demo the native-terminal rendering;
   it delivers the core "feels like a real terminal" value alone.
2. **Increment 2**: Add Phase 4 (US2) — mouse selection/copy/scroll, the headline UX upgrade.
3. **Increment 3**: Add Phase 5 (US3) — re-home the block store with the DB-ready seam,
   restoring `/save`/`/filter`/exit chrome with no regression.
4. **Harden**: Phase 6 — docs, regression check, manual quickstart, final gate.

**Total tasks**: 42 — Setup 5, Foundational 5, US1 9 (2 test + 7 impl), US2 9 (2 test +
7 impl), US3 9 (2 test + 7 impl), Polish 5.
