# Tasks: kapollo MVP — Split-Pad Shell REPL

**Input**: Design documents from `/specs/001-mvp-repl/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Included. The project constitution (Principle III) mandates
Test-Driven Development; PTY/terminal interactive behavior is covered by
integration tests against a headless PTY harness (documented exception).

**Organization**: Tasks are grouped by user story (US1–US4) so each story
can be implemented and tested independently. MVP = US1 + US4 (both P1).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story the task belongs to (US1–US4)
- All paths are repository-relative (single binary crate per plan.md)

## Path Conventions

Single project: `src/`, `tests/`, `docs/` at repository root.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and toolchain.

- [X] T001 Create the crate directory structure per plan.md (`src/` with module folders `pty/`, `output/`, `session/`, `input/`, `slash/`, `ui/`; `tests/`; `docs/`) and placeholder `mod.rs` files
- [X] T002 Configure `Cargo.toml` with edition 2021, two binary targets (`kap` and `kapollo`) sharing a lib, and dependencies: `ratatui`, `crossterm`, `portable-pty`, `vte`, `serde`, `toml`, `tracing`, `tracing-subscriber`, `tracing-appender`, `anyhow`, `thiserror`, `directories`
- [X] T003 [P] Add `rust-toolchain.toml` pinning the stable toolchain
- [X] T004 [P] Add `rustfmt.toml` and confirm the Code Standards Gate runs locally (`cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure required before ANY user story can be built.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T005 [P] Define library error types in `src/lib.rs` (or `src/error.rs`) using `thiserror` for library errors and `anyhow` for app-level errors
- [X] T006 [P] Implement file-sink logging in `src/logging.rs` via `tracing` + `tracing-appender` (XDG state dir, quiet by default, opt-in verbose; never writes to the TUI) per FR-030
- [X] T007 [P] Config unit test in `tests/config.rs` ⚠️ (write first, must fail): defaults when file absent, missing-key defaults, unknown-key logged-and-ignored (not fatal), cap clamping to hard maxima, invalid-TOML actionable error per FR-028, FR-029, contracts/config.md, research R10
- [X] T008 Implement configuration loading in `src/config.rs` (`serde`/`toml`, defaults when absent, XDG path `~/.config/kapollo/config.toml`, unknown keys logged-and-ignored, caps clamped to hard maxima) per FR-028, FR-029, and data-model Configuration — makes T007 pass
- [X] T009 Implement the RAII terminal guard and panic hook in `src/ui/mod.rs` (enter/leave raw mode + alt-screen, restore cursor; panic hook restores terminal and logs) per FR-025, FR-026, research R8
- [X] T010 [P] Implement the `Block` entity in `src/session/block.rs` (id, command, timestamps, raw output bytes, truncated flag, exit_code, state enum `Running|Closed|Interactive`, reserved `private`/`save_output`) per FR-015 and data-model Block
- [X] T011 [P] Implement the capped output ring buffer in `src/session/ringbuf.rs` (retain tail raw bytes, drop head, set truncated; per-block + transcript cap enforcement) per FR-015, FR-016, research R6
- [X] T012 Implement the `Transcript`/session model in `src/session/mod.rs` (ordered blocks, total_bytes, scroll_offset, whole-transcript eviction) — depends on T010, T011
- [X] T013 [P] Implement the `InputPad` model in `src/input/mod.rs` (multiline buffer, cursor, internal scroll) per data-model InputPad
- [X] T014 Implement the UI layout skeleton in `src/ui/mod.rs` (split transcript pad / input pad / status line; honor terminal size) — depends on T009
- [X] T015 Implement the App and event loop skeleton in `src/app.rs` (single-threaded event loop, `mpsc`/crossbeam channel plumbing for PTY-reader thread → loop, key/resize event dispatch) per research R1
- [X] T016 Implement `src/main.rs` wiring: arg parsing (`--shell`, `--config`, `--verbose`, `--version`, `--help`), config load, logging init, non-TTY detection (no TUI when stdout is not a TTY), terminal setup/teardown, and the event-loop panic boundary per FR-032 and contracts/cli.md

**Checkpoint**: Foundation ready — user stories can now begin.

---

## Phase 3: User Story 1 - Run commands in a split-pad shell (Priority: P1) 🎯 MVP

**Goal**: Wrap the real shell in a PTY and present each command + its output as a discrete block in the transcript pad, with shell state persisting across commands.

**Independent Test**: Launch `kap`, run `echo hello` → `hello` appears as a block; run `cd /tmp` then `pwd` → reports `/tmp`; run `false` → status shows non-zero exit code.

### Tests for User Story 1 ⚠️ (write first, must fail)

- [X] T017 [P] [US1] Headless PTY smoke test in `tests/pty_smoke.rs` (spawn shell, send `echo hello`, assert captured output) per SC-001
- [X] T018 [P] [US1] Block boundary + exit code test in `tests/block_boundaries.rs` (OSC 133 marks delimit blocks; exit code captured; sentinel fallback path) per FR-005, FR-006
- [X] T019 [P] [US1] Output caps + truncation test in `tests/caps.rs` (per-block and transcript caps hold; raw bytes retained; truncation marker set) per FR-015, FR-016, SC-006

### Implementation for User Story 1

- [X] T020 [US1] Implement PTY spawn/read/write/resize in `src/pty/mod.rs` using `portable-pty` (PTY-reader thread feeding the event-loop channel) per FR-002, FR-003
- [X] T021 [US1] Implement shell detection and hook installation in `src/pty/shell.rs` (fish/bash OSC 133 injection, `shell_kind`, export `KAPOLLO_ACTIVE=1` + `KAPOLLO_VERSION`) per FR-007, FR-008, research R2, R12
- [X] T022 [P] [US1] Implement ANSI/OSC 133 + alt-screen parsing in `src/output/parser.rs` using `vte` (prompt marks, exit code, alt-screen enter/leave detection only — no grid model) per FR-005, research R2, R4
- [X] T023 [P] [US1] Implement sentinel fallback boundary detection in `src/output/sentinel.rs` per FR-005, research R3
- [X] T024 [US1] Implement the output processor orchestration in `src/output/mod.rs` (route PTY bytes → segments → append to the current block via ringbuf) — depends on T022, T023, T011
- [X] T025 [US1] Wire submit-non-slash-input → create block + send to shell stdin in `src/app.rs`, closing the block on the end mark and setting exit code per FR-004, FR-006, FR-009
- [X] T026 [P] [US1] Implement transcript rendering in `src/ui/transcript.rs` (render blocks with command + output) per FR-004
- [X] T027 [P] [US1] Implement input pad rendering in `src/ui/input_pad.rs` (render buffer, cursor; clear on submit)
- [X] T028 [P] [US1] Implement status line in `src/ui/status.rs` (current working directory + last exit code) per FR-033
- [X] T029 [US1] Handle wrapped-shell self-exit → clean termination and terminal restore in `src/app.rs` per FR-027

**Checkpoint**: US1 fully functional — the core run loop works on fish and bash (SC-002, SC-009).

---

## Phase 4: User Story 4 - Interrupt, control, and exit safely (Priority: P1)

**Goal**: Ctrl-C interrupts only the running command; slash commands (`/help`, `/clear`, `/quit`) work; terminal always restored on exit/error/panic.

**Independent Test**: Run `sleep 60`, press Ctrl-C → command interrupts, kapollo survives; `/quit` exits with terminal restored cleanly; `//foo` sends literal `/foo` to the shell.

### Tests for User Story 4 ⚠️ (write first, must fail)

- [X] T030 [P] [US4] Input router test in `tests/input_router.rs` (slash detection, doubled-leader `//` escape, non-slash passthrough) per FR-021, FR-022
- [X] T031 [P] [US4] SIGINT forwarding + clean-teardown test in `tests/signals.rs` (Ctrl-C interrupts command not kapollo; terminal restored on exit) per FR-024, SC-004, SC-005

### Implementation for User Story 4

- [X] T032 [US4] Implement the input router in `src/input/router.rs` (leader-char slash detection, doubled-leader escape to literal, else passthrough) per FR-021, FR-022
- [X] T033 [P] [US4] Implement the slash command registry in `src/slash/mod.rs` (exact-match dispatch, unknown-command error block suggesting `/help`)
- [X] T034 [US4] Implement built-in slash commands in `src/slash/builtins.rs` (`/help`, `/clear`, `/quit`) per FR-023 and contracts/slash-commands.md — depends on T033
- [X] T035 [US4] Implement Ctrl-C (SIGINT) forwarding to the foreground command's process group in `src/pty/mod.rs` / `src/app.rs` per FR-024, research R7
- [X] T036 [US4] Wire `/quit` and fatal-error/panic paths through the shared clean-teardown in `src/app.rs` per FR-025, FR-026

**Checkpoint**: MVP complete — US1 + US4 deliver a trustworthy core run loop.

---

## Phase 5: User Story 2 - Compose multiline commands and recall history (Priority: P2)

**Goal**: Shift+Enter/Alt+Enter insert newlines; Enter submits the whole multiline input; Up/Down recall prior inputs from kapollo's own history.

**Independent Test**: Compose a 3-line input with Shift+Enter, submit with Enter → runs as one unit; press Up → previous input recalled.

### Tests for User Story 2 ⚠️ (write first, must fail)

- [X] T037 [P] [US2] Multiline + history test in `tests/input_pad.rs` (Shift/Alt+Enter inserts newline without submitting; Enter submits multiline; Up/Down recall) per FR-010, FR-011, FR-013, SC-007

### Implementation for User Story 2

- [X] T038 [US2] Enable the Kitty keyboard protocol via `crossterm` in `src/ui/mod.rs` to disambiguate Shift+Enter, with Alt+Enter fallback per FR-010, research R5
- [X] T039 [US2] Implement multiline editing in `src/input/mod.rs` (Shift+Enter/Alt+Enter insert newline; Enter submits whole buffer; auto-grow to height cap then internal scroll) per FR-010, FR-011, FR-012
- [X] T040 [P] [US2] Implement `InputHistory` in `src/input/mod.rs` (append on submit, Up/Down recall, separate from shell history) per FR-013, data-model InputHistory
- [X] T041 [US2] Implement independent transcript scrolling in `src/ui/transcript.rs` and key handling in `src/app.rs` per FR-014

**Checkpoint**: US1 + US4 + US2 all work independently.

---

## Phase 6: User Story 3 - Run interactive full-screen programs (Priority: P2)

**Goal**: Detect alt-screen programs (`vim`, `less`, `top`), hand the terminal to them via passthrough, and restore the split-pad UI on exit.

**Independent Test**: Run `vim` → opens and is usable; `:q` → split-pad UI restored with transcript intact; resize during `top` reflows correctly.

### Tests for User Story 3 ⚠️ (write first, must fail)

- [X] T042 [P] [US3] Passthrough test in `tests/passthrough.rs` (alt-screen enter → passthrough; leave → UI restored; block marked `Interactive`) per FR-018, FR-020, SC-003

### Implementation for User Story 3

- [X] T043 [US3] Implement alt-screen handoff in `src/ui/passthrough.rs` (enter passthrough on alt-screen detect, forward terminal + keystrokes to the program, suspend capture) per FR-018
- [X] T044 [US3] Forward terminal resize to the wrapped program during passthrough in `src/pty/mod.rs` / `src/app.rs` per FR-019, SC-008
- [X] T045 [US3] Restore the split-pad UI with the prior transcript intact on passthrough exit, setting block `state` back from `Interactive` in `src/app.rs` per FR-020

**Checkpoint**: All user stories independently functional.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Cross-cutting requirements and release/repo readiness. The screenshot task is ordered last (depends on a working TUI).

- [X] T046 [P] Honor `NO_COLOR` for kapollo's own chrome in `src/ui/mod.rs` per FR-031
- [X] T047 [P] Graceful degradation when the terminal is too small in `src/ui/mod.rs` (minimal layout or clear message, no crash/corruption) per spec edge cases
- [X] T048 Forward terminal resize to the shell and reflow both pads without losing transcript content in `src/app.rs` / `src/ui/mod.rs` per FR-017
- [X] T049 [P] Update `docs/architecture.md` with any implementation-driven changes (Constitution II)
- [X] T050 [P] Write `docs/setup.md` (build/install/run on Linux)
- [X] T051 [P] Write `docs/usage.md` (keys, slash commands, config)
- [X] T052 [P] Write `docs/specification.md` (combined specification) per Constitution I
- [X] T053 [P] Add `LICENSE` (MIT)
- [X] T054 [P] Add `.gitignore` (Rust defaults: `/target`, scratch dirs)
- [X] T055 Complete `Cargo.toml` metadata (`license = "MIT"`, `description`, `repository`, `keywords`, `categories`, authors)
- [X] T056 [P] Add CI workflow in `.github/workflows/ci.yml` running the Code Standards Gate (`cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`) per Constitution IV
- [X] T057 [P] Add `CHANGELOG.md` with the initial MVP entry
- [X] T058 Run the fish-AND-bash core-run-loop parity check (run an identical command sequence under each shell; confirm blocks, exit codes, and shell-state persistence match) per SC-009 — automated in `tests/shell_parity.rs`
- [~] T059 Run the `quickstart.md` acceptance walkthrough and confirm SC-001 through SC-009 — SC-001/002/004/005/006/007/009 covered by the automated suite (pty_smoke, shell_parity, signals, caps, input_pad); SC-003 (vim/less/top) and SC-008 (live resize) need a manual TTY run
- [~] T060 Write `README.md` (overview, Apollo-DM inspiration, install/build, usage, `kap` alias, key bindings, license badge) and capture a screenshot/asciinema cast of kapollo running — README done; screenshot/cast requires a manual TTY capture (placeholder noted in README)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately.
- **Foundational (Phase 2)**: Depends on Setup — BLOCKS all user stories.
- **User Stories (Phases 3–6)**: All depend on Foundational completion.
  - **US1 (P1)** and **US4 (P1)** form the MVP. US4 routing/teardown builds on the US1 run loop, so within the MVP do US1 then US4 (or in parallel where files differ).
  - **US2 (P2)** and **US3 (P2)** can follow in either order; both are independently testable.
- **Polish (Phase 7)**: Depends on the desired user stories; T058/T059 validate, T060 (README + screenshot) is last.

### User Story Dependencies

- **US1 (P1)**: Foundational only.
- **US4 (P1)**: Foundational; shares the teardown path and event loop with US1.
- **US2 (P2)**: Foundational; extends the input pad — independent of US3/US4.
- **US3 (P2)**: Foundational; passthrough builds on US1's alt-screen detection.

### Within Each User Story

- Tests are written first and MUST fail before implementation.
- Models → parsing/processing → app wiring → UI rendering.

### Parallel Opportunities

- Setup: T003, T004 in parallel.
- Foundational: T005, T006, T007, T010, T011, T013 in parallel (different files); T008 after T007 (test first); T012 after T010/T011; T014 after T009; T015/T016 last.
- US1: tests T017–T019 in parallel; T022/T023 in parallel; UI tasks T026–T028 in parallel.
- US4: tests T030/T031 in parallel; T033 parallel with T032.
- Polish: most doc/repo tasks (T046, T047, T049–T054, T056, T057) in parallel.

---

## Implementation Strategy

1. **MVP first**: Complete Phase 1 → Phase 2 → US1 (Phase 3) → US4 (Phase 4). This yields a trustworthy, daily-driver-capable core: run commands as blocks, interrupt, slash commands, clean teardown.
2. **Incremental delivery**: Add US2 (multiline + history) and US3 (passthrough) — each is an independently testable increment.
3. **Polish & release**: Cross-cutting requirements (resize, NO_COLOR, small-terminal) then repo readiness (docs, LICENSE, CI, fish/bash parity check, README + screenshot) before pushing to GitHub.

---

## Summary

- **Total tasks**: 60
- **Setup**: 4 (T001–T004)
- **Foundational**: 12 (T005–T016) — 1 test (config)
- **US1 (P1, MVP)**: 13 (T017–T029) — 3 tests
- **US4 (P1, MVP)**: 7 (T030–T036) — 2 tests
- **US2 (P2)**: 5 (T037–T041) — 1 test
- **US3 (P2)**: 4 (T042–T045) — 1 test
- **Polish**: 15 (T046–T060)
- **Suggested MVP scope**: US1 + US4 (both P1).
