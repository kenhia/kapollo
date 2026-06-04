# Tasks: Terminal-Grid Spike

**Input**: Design documents from `specs/003-grid-spike/`
**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: Per [plan.md](plan.md) Constitution Check, TDD is relaxed for the
interactive slice (manual host-terminal validation). Automated tests are written
**only** for the pure helpers in `spike-support` (selection coordinate math, mode
detection, OSC 52 framing). Those test tasks are included below and are REQUIRED.

**Organization**: Tasks are grouped by user story (S1 `vt100`, S2 `alacritty_terminal`,
S3 `wezterm-term`, and the recommendation synthesis). Stages run sequentially per the
spike plan, but each story is an independently demonstrable increment.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no incomplete dependencies)
- **[Story]**: US1=S1 vt100, US2=S2 alacritty, US3=S3 wezterm, US4=recommendation

## Path Conventions

All spike code lives in the isolated `delos/` Cargo workspace (FR-002/FR-003). The
shipping `kapollo`/`kap` crate at the repo root is **not** modified except for the
one-line workspace `exclude`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Stand up the isolated `delos/` workspace; guarantee shipping-crate isolation.

- [x] T001 Add `[workspace] exclude = ["delos"]` to the root [Cargo.toml](../../Cargo.toml) so `cargo build`/`cargo test` at the repo root never descend into the spike (FR-003/FR-004); confirm root `cargo build && cargo test` still pass.
- [x] T002 Create `delos/Cargo.toml` as a standalone workspace (`resolver = "2"`, members `spike-support`, `spike-vt100`, `spike-alacritty`, `spike-wezterm`; `[workspace.dependencies]` ratatui 0.30, crossterm 0.29, portable-pty 0.9, arboard 3.6) per [quickstart.md](quickstart.md) §1.
- [x] T003 Scaffold the four crates in `delos/`: `cargo new --lib spike-support`, `cargo new --bin spike-vt100`, `cargo new --bin spike-alacritty`, `cargo new --bin spike-wezterm`.
- [x] T004 [P] Add `delos/README.md` describing what delos is, how to run each stage, and linking back to [spec.md](spec.md) and [03-spike-plan.md](../planning/grid-pivot/03-spike-plan.md).
- [x] T005 [P] Create the empty deliverable docs under `delos/docs/`: `scorecard.md` (seed with the rubric template from [contracts/scorecard.md](contracts/scorecard.md)), `s1-vt100.md`, `s2-alacritty.md`, `s3-wezterm.md`, `recommendation.md`.

**Checkpoint**: `cd delos && cargo build` succeeds (empty crates); root crate still green and `cargo tree` at root shows no spike deps.

---

## Phase 2: Foundational (Blocking Prerequisites) — `spike-support`

**Purpose**: The shared, crate-agnostic plumbing + the unit-tested pure helpers every
stage reuses (data-model.md §Validation rules). **No stage slice can be built until
this is done.**

**⚠️ TDD REQUIRED here**: write the failing test first, then implement, for each pure helper.

- [x] T006 Configure `delos/spike-support/Cargo.toml`: depend on `portable-pty.workspace = true`, `arboard.workspace = true`, and `base64 = "0.22"` (for OSC 52 framing in T011).
- [x] T007 [P] Selection coordinate math (TDD) in `delos/spike-support/src/coords.rs`: write failing unit tests then implement `screen_to_content(top_row, screen_y) -> abs_row` + inverse with output-region clamping (FR-008/FR-009, R5).
- [x] T008 [P] Auto-scroll math (TDD) in `delos/spike-support/src/coords.rs`: failing tests then `auto_scroll(top_row, drag_y, height, history_len) -> top_row'` — up when `drag_y < 0`, down when `drag_y >= height`, clamp `[0, history_len-height]` (FR-010, R5).
- [x] T009 [P] Selection normalization (TDD) in `delos/spike-support/src/coords.rs`: failing tests then `normalize(anchor, end) -> (start, end)` yielding document order for copy (FR-016).
- [x] T010 [P] Mode detector (TDD) in `delos/spike-support/src/modes.rs`: failing tests then `detect_mode(&[u8]) -> Vec<ModeEvent>` recognizing alt-screen `?1049h/l` (+ legacy `?47`,`?1047`) and child mouse modes `?1000/1002/1003/1006 h/l` (FR-013/FR-014, R4).
- [x] T011 [P] OSC 52 framing (TDD) in `delos/spike-support/src/clipboard.rs`: failing tests then `osc52_frame(&[u8]) -> String` producing `ESC ] 52 ; c ; <base64> ST` (FR-020, R3); plus an optional `arboard` fallback `copy_local(&str)` (FR-021).
- [x] T012 PTY plumbing in `delos/spike-support/src/pty.rs`: `spawn_shell_pty(shell) -> PtyShell` using portable-pty (reader thread → byte channel, writer handle, resize propagation), mirroring kapollo's approach but self-contained (R7). No unit test (I/O boundary); smoke-validated via the first stage.
- [x] T013 Re-export the public surface in `delos/spike-support/src/lib.rs` (`coords`, `modes`, `clipboard`, `pty`) and run the `spike-support` gate: `cargo fmt --check && cargo clippy -- -D warnings && cargo test` (all helper tests green).

**Checkpoint**: `spike-support` compiles, all pure-helper unit tests pass, gate clean. User-story stages can now begin.

---

## Phase 3: User Story 1 — S1 vertical slice on `vt100` (Priority: P1) 🎯 MVP

**Goal**: Prove the core feel (grid render + content-coord selection + scroll +
alt-screen handover + explicit copy) on `vt100`; fill the first scorecard column.

**Independent Test**: Run `cargo run -p spike-vt100` in Windows Terminal Preview; per
[quickstart.md](quickstart.md) §4 confirm styled-grid render, drag-select in content
coords, wheel scroll, `vi` handover + clean restore, right-press/Ctrl-C copy on an
active selection, and SIGINT when no selection. Fill the S1 scorecard column.

- [x] T014 [US1] Add S1 deps to `delos/spike-vt100/Cargo.toml`: `spike-support` (path), `ratatui.workspace`, `crossterm.workspace`, `vt100 = "0.16"`, `tui-term = "0.3"` (optional accelerator, R6). **First** verify `tui-term` resolves against `ratatui` 0.30 (`cargo update -p spike-vt100` / build); if it conflicts, drop `tui-term` and use the raw `vt100` API per R6.
- [x] T015 [US1] Terminal lifecycle in `delos/spike-vt100/src/main.rs`: raw mode + mouse capture + alt-screen enter on start; **panic-safe** restore on exit/Ctrl-Q (leave raw mode, disable mouse, leave alt screen) per [contracts/spike-binary-cli.md](contracts/spike-binary-cli.md).
- [x] T016 [US1] Wire `spike_support::pty::spawn_shell_pty` and feed child bytes into a `vt100::Parser`; pump keystrokes to the PTY verbatim (FR-005/FR-006).
- [x] T017 [US1] Render the `vt100` main screen as ratatui styled spans (SGR colors/attrs, wide chars), main screen only, with scrollback windowing via `top_row` (FR-007). Decide tui-term vs raw `vt100` here and note it in `delos/docs/s1-vt100.md` (R6).
- [x] T018 [US1] Routing override using `spike_support::modes::detect_mode`: while alt-screen active OR child mouse mode enabled, forward all input/mouse to the child and suspend selection/scroll; resume on reset/exit (FR-013/FR-014).
- [x] T019 [US1] SelectionController in `delos/spike-vt100/src/selection.rs` implementing the data-model state machine using `spike_support::coords`: left-press→Dragging, drag-extend with auto-scroll past edge, release→Active (no copy), second-left/ESC→cancel, Shift→forward to child (FR-008/009/010/011/017/018).
- [x] T020 [US1] Copy triggers + context menu: right-press/Ctrl-C on Active selection → `osc52_frame` copy then deselect; right-press/Ctrl-C with no selection → "Hello, World." menu / SIGINT respectively; render the selection highlight and the trivial menu (FR-015/016/019).
- [x] T021 [US1] Scroll input: mouse wheel + PgUp/PgDn adjust `top_row` (selection survives via content coords) (FR-012); add `--clipboard=arboard` flag to exercise the fallback (FR-021).
- [x] T022 [US1] Run the manual validation script ([quickstart.md](quickstart.md) §4) for S1; write the nuts-and-bolts writeup in `delos/docs/s1-vt100.md` and fill the **S1 column** of `delos/docs/scorecard.md` (all 12 criteria) (SC-001/SC-002).
- [x] T023 [US1] Isolation check ([quickstart.md](quickstart.md) §6): at repo root `cargo tree | grep -E 'vt100|alacritty_terminal|wezterm-term|termwiz'` returns nothing and root `cargo build && cargo test` stay green (SC-007).

**Checkpoint**: S1 slice runs; selection + alt-screen handover demonstrated on at least one crate (satisfies SC-004/SC-005 minimally); S1 scorecard column filled.

---

## Phase 4: User Story 2 — S2 vertical slice on `alacritty_terminal` (Priority: P2)

**Goal**: Rebuild the *identical* slice on the proven baseline crate for apples-to-apples comparison (scrollback, damage tracking, selection primitives).

**Independent Test**: Run `cargo run -p spike-alacritty` through the same
[quickstart.md](quickstart.md) §4 script; confirm identical observable behaviors;
fill the S2 scorecard column + writeup.

- [x] T024 [US2] Add S2 deps to `delos/spike-alacritty/Cargo.toml`: `spike-support` (path), `ratatui.workspace`, `crossterm.workspace`, `alacritty_terminal = "0.26"`.
- [x] T025 [US2] Port the S1 binary shell into `delos/spike-alacritty/src/main.rs` + `selection.rs`: reuse `spike-support` helpers and the same terminal lifecycle/routing/selection/copy/menu/scroll wiring (T015–T021), swapping the grid model to `alacritty_terminal`'s `Term`/`Grid` parser and its scrollback (FR-005…FR-021).
- [x] T026 [US2] Map `alacritty_terminal` cells → ratatui styled spans; cross-check its native alt-screen flag against `spike_support::modes` and note discrepancies (R4).
- [x] T027 [US2] Under output flood ([quickstart.md](quickstart.md) §4 step 10) record `alacritty_terminal` damage/dirty-tracking behavior and responsiveness (scorecard criterion 8).
- [x] T028 [US2] Run the manual script for S2; write `delos/docs/s2-alacritty.md` (noting surprises vs S1) and fill the **S2 column** of `delos/docs/scorecard.md` (SC-001/SC-002).
- [x] T029 [US2] Isolation check at repo root (as T023); confirm shipping graph still has zero spike deps and stays green (SC-007).

**Checkpoint**: S1 and S2 slices both runnable independently; two scorecard columns filled.

---

## Phase 5: User Story 3 — S3 vertical slice on `wezterm-term` (Priority: P3)

**Goal**: Rebuild the identical slice on the maximum-fidelity crate (graphemes, OSC 8
hyperlinks, optional images) and weigh its extra fidelity against its weight/API cost.

**Independent Test**: Run `cargo run -p spike-wezterm` through the same script; probe
grapheme segmentation + OSC 8; image forwarding as a cherry; fill the S3 column.

- [x] T030 [US3] Pin `wezterm-term` as a git dep in `delos/spike-wezterm/Cargo.toml` (`git = "https://github.com/wezterm/wezterm.git", rev = "<latest green tag>"`); record the exact `rev` in `delos/docs/s3-wezterm.md` (R1). Add `spike-support`, `ratatui.workspace`, `crossterm.workspace`.
- [x] T031 [US3] Port the slice into `delos/spike-wezterm/src/main.rs` + `selection.rs` reusing `spike-support`, swapping the grid model to `wezterm-term`'s `Terminal`/`Screen` (termwiz cells) (FR-005…FR-021).
- [x] T032 [US3] Map `wezterm-term` cells → ratatui styled spans; record grapheme/Unicode segmentation and wide/combining-char fidelity (scorecard criteria 1–2).
- [x] T033 [US3] Probe OSC 8 hyperlink parsing/exposure (criterion 6) and record build-weight/compile-time (criterion 10) given the large wezterm transitive tree.
- [x] T034 [US3] **Cherry (easy-cut, FR-028)**: time-boxed probe of whether an image protocol (sixel/kitty/iTerm) can be forwarded through the owned grid; record achievable/not without spending multiple days; MUST NOT gate the decision.
- [x] T035 [US3] Run the manual script for S3; write `delos/docs/s3-wezterm.md` and fill the **S3 column** of `delos/docs/scorecard.md` (SC-001/SC-002).
- [x] T036 [US3] Isolation check at repo root (as T023); confirm the heavy wezterm/termwiz tree never entered the shipping graph (SC-007).

**Checkpoint**: All three scorecard columns filled; three writeups complete.

---

## Phase 6: User Story 4 — Crate recommendation (Priority: P1)

**Goal**: Synthesize the three columns into one production-crate recommendation with
rationale + feasibility confirmation — the spike's reason for existing.

**Independent Test**: Review `recommendation.md`; confirm it names exactly one crate,
cites weighted-rubric evidence, and states selection + alt-screen feasibility.

- [x] T037 [US4] Verify scorecard completeness: every one of the 12 criteria has an entry in all three columns; fill any gaps with explicit `-`/`n/a` + note (SC-001).
- [x] T038 [US4] Write `delos/docs/recommendation.md`: name exactly one production crate with rationale weighted by the rubric (SC-003); explicitly confirm or refute that the content-coord selection model and alt-screen handover are achievable (SC-004/SC-005); state how the outputs feed D25–D30 and the in-place rework spec (FR-027).

**Checkpoint**: Recommendation written; spike's decision output ready to promote.

---

## Phase 7: Polish & Cross-Cutting

**Purpose**: Cross-stage validation and final hygiene.

- [x] T039 Complete the host-terminal matrix ([quickstart.md](quickstart.md) §5): run the slice in Windows Terminal Preview (primary) + at least one of GNOME Terminal/Konsole (secondary); record per-terminal OSC 52 honor in `delos/docs/scorecard.md` or the writeups (SC-006/FR-026).
- [x] T040 [P] Final isolation + green gate: at repo root confirm `cargo tree` shows zero spike deps and `cargo build && cargo test` pass; in `delos/` run `cargo fmt --check && cargo clippy -- -D warnings && cargo test` clean (SC-007).
- [x] T041 [P] Update `delos/README.md` with final run instructions and a one-line pointer to `recommendation.md`; ensure the planning docs' decks-clearing/next-step section reflects that the spike is complete.

---

## Dependencies & Execution Order

- **Phase 1 (Setup)** → **Phase 2 (spike-support)** are strictly blocking; nothing else starts until T013 is green.
- **US1 (P1)** is the MVP and the foundation pattern every later stage ports. Build it first.
- **US2 (P2)** and **US3 (P3)** depend on the slice pattern established in US1 (they port `selection.rs`/`main.rs`). They run **sequentially** per the spike plan (S1→S2→S3), though their crates are independent.
- **US4 (P1)** depends on all three scorecard columns (T022, T028, T035).
- **Polish** depends on US4 (or runs alongside the final isolation checks).

### Story dependency graph

```text
Setup (T001-T005)
   └─ Foundational spike-support (T006-T013)  ← BLOCKS all stories
        ├─ US1 S1 vt100 (T014-T023)  🎯 MVP, establishes the slice pattern
        │     └─ US2 S2 alacritty (T024-T029)   [ports US1 pattern]
        │           └─ US3 S3 wezterm (T030-T036)   [ports US1 pattern]
        └────────────────────────────────────────────┐
                                                       └─ US4 recommendation (T037-T038)
                                                            └─ Polish (T039-T041)
```

## Parallel Opportunities

- **Setup**: T004, T005 are `[P]` (different files) once T003 scaffolds the crates.
- **Foundational**: T007, T008, T009, T010, T011 are `[P]` — independent pure-helper modules, each TDD'd in its own file (`coords.rs`, `modes.rs`, `clipboard.rs`). T012 (PTY) and T013 (lib wire-up) follow.
- **Within a stage**: tasks are mostly sequential (same `main.rs`/`selection.rs`).
- **Polish**: T040, T041 are `[P]`.
- **Across stages**: although S1→S2→S3 are run sequentially by design, the S2/S3 crates are independent, so a second contributor *could* port them in parallel after US1 lands — not recommended for a single maintainer.

## Implementation Strategy

- **MVP = US1 only.** A complete S1 slice already answers the central feasibility
  question (is the selection model + alt-screen handover achievable?) and produces the
  first scorecard column. If time/appetite ran out after US1, the spike still has value.
- **Incremental delivery**: US1 → US2 → US3 each add a comparable column; US4 converts
  the columns into the decision. Stop-after-any-stage still yields a partial but usable
  comparison.
- **Throwaway discipline**: correctness matters only insofar as it makes the rubric
  fillable and the feel demonstrable; do not gold-plate the spike binaries.
