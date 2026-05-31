# Tasks: kapollo MVP Hardening — Render, Chrome, Passthrough & Performance

**Input**: Design documents from `specs/002-mvp-hardening/`
**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/)

**Tests**: Included. The project mandates TDD (Constitution III); test tasks are
written first and must fail before the corresponding implementation.

**Organization**: Tasks are grouped by user story. Phases are ordered by the
recommended dependency work order **D → A+B → C → E** (performance first; then
render + chrome together since removing the transcript frame eliminates render-
corruption surface; then passthrough; then quick wins). All five stories remain
independently testable.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1–US5 (Setup/Foundational/Polish carry no story label)
- Exact file paths are included in each task

## Path Conventions

Single Rust crate at repository root: `src/`, `tests/`, `docs/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Confirm the working branch and a green baseline before changes.

- [X] T001 Confirm branch `002-mvp-hardening` is checked out and the tree is clean; run the gate baseline `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test` and record the current passing test count.
- [X] T002 Re-read [.scratch/kapollo-mvp-usertest.md](../../.scratch/kapollo-mvp-usertest.md) and [research.md](research.md) (R1–R8) to confirm the fix mechanisms before editing.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared plumbing that multiple stories depend on — the new
`Boundary::Cwd` variant and the `Config` keys. These are small, additive, and
must land before stories that consume them (US2 chrome color, US5 cwd).

**⚠️ CRITICAL**: No story that renders chrome color or cwd can complete until this phase is done.

- [X] T003 [P] Add `prompt_char: char` (default `'λ'`) and `prompt_color: Color` (default red) fields to `Config` with `Default` impl, in src/config.rs (FR-010/011/023).
- [X] T004 [P] Add `Boundary::Cwd(PathBuf)` variant to the boundary enum in src/output/parser.rs (no parsing yet — variant + match arms only) (FR-019).
- [X] T005 Add a `cwd: PathBuf` field to `App` initialized from `std::env::current_dir()` in src/app.rs (FR-019).

**Checkpoint**: Config and boundary plumbing compile; stories can now begin.

---

## Phase 3: User Story 4 - Stay responsive and interruptible under huge output (Priority: P1) 🎯 MVP

**Goal**: Fix the flood hang — amortized O(1) ring-buffer cap enforcement and a bounded, interruptible event loop so a 5M-line flood completes near shell-native time and Ctrl-C interrupts promptly.

**Independent Test**: Run `yes | head -n 5000000`; it completes in roughly shell-native time, the UI stays responsive, and Ctrl-C during the flood interrupts it.

### Tests for User Story 4 ⚠️ (write first, must fail)

- [X] T006 [P] [US4] Extend tests/caps.rs: assert incremental-`line_count` + bulk-trim parity with the prior implementation (retained bytes, `truncated()`, `byte_len()` equal) across mixed push sequences (FR-014).
- [X] T007 [P] [US4] Extend tests/caps.rs: tail fast-path test — a single `push` larger than `cap_bytes` retains only the last `cap_bytes`, sets `truncated`, and applies the line cap; add a flood-shaped input that asserts cap enforcement stays within a wall-clock budget (FR-014/016).

### Implementation for User Story 4

- [X] T008 [US4] Add `line_count: u64` to `OutputBuffer` and maintain it incrementally in `push` (count `\n` in the slice, never rescan) in src/session/ringbuf.rs (FR-014).
- [X] T009 [US4] Rewrite `enforce_caps()` for bulk trimming (single `drain(..overflow)` for the byte cap; bulk `drain` to the next `\n` for the line cap; adjust `line_count`; set `truncated`) in src/session/ringbuf.rs (FR-014).
- [X] T010 [US4] Add the tail fast-path to `push` (when `cap_bytes > 0` and `data.len() >= cap_bytes`, replace buffer with the last `cap_bytes`, recompute `line_count`, set `truncated`, apply line cap) in src/session/ringbuf.rs (FR-016).
- [X] T011 [US4] Bound the per-pass drain in the event loop (cap chunk count / byte budget per `drain_shell` pass, ~256 KiB / 64 chunks per research R5) so key input is serviced between passes, in src/app.rs (FR-015).
- [X] T012 [US4] Ensure a pending interrupt (Ctrl-C) is checked/serviced each loop iteration so it is not starved during a flood, in src/app.rs (FR-017).

**Checkpoint**: Flood completes near shell-native time; Ctrl-C interrupts promptly; caps tests green.

---

## Phase 4: User Story 1 - Read output that renders cleanly and correctly (Priority: P1)

**Goal**: Normalize captured output to printable text and make the renderer fully own/clear its surface so no control bytes leak and no stray characters bleed across rows.

**Independent Test**: Run `ls`, then repeatedly `echo $SHELL` past a full screen; no chrome is overwritten, no stray characters remain after scroll, every line intact; `cat` of a tabbed multi-line file has a correct first line.

### Tests for User Story 1 ⚠️ (write first, must fail)

- [X] T013 [P] [US1] Create tests/render_normalize.rs: feed bytes containing a bare `\r`, an OSC color-query response (`ESC]11;rgb:2020/2020/2020 ST`), `\r\n`, tabs, and residual CSI/SGR; assert the normalized block text contains only printable chars + `\n`/`\t` and none of the escapes/responses (FR-001/002).
- [X] T014 [P] [US1] Add a first-line-parity case to tests/render_normalize.rs: the first line of a block normalizes identically to subsequent lines (FR-004).

### Implementation for User Story 1

- [X] T015 [US1] Normalize the `Output` stream in the vte performer: map `\r\n`→`\n`, drop lone `\r` and other C0 controls, swallow all OSC payloads (133 handled, 7→`Cwd`, others discarded), consume CSI/ESC/DCS without emitting visible text — in src/output/parser.rs (FR-001).
- [X] T016 [US1] Update src/output/mod.rs so `OutputProcessor::apply` emits only normalized output and surfaces the new boundaries to `App` (FR-001).
- [X] T017 [US1] Update `output_lossy()`/rendered-text accessor in src/session/block.rs to reflect that stored output is already normalized (no re-stripping needed) (FR-001/004).
- [X] T018 [US1] Make the transcript renderer fully clear/own its rectangle each frame (no residual cells after scroll) and confine output strictly to the transcript area in src/ui/transcript.rs (FR-002/003).

**Checkpoint**: Rendered blocks are clean printable text; no stray characters after scroll; render_normalize tests green.

---

## Phase 5: User Story 2 - See a clean, informative chrome (Priority: P1)

**Goal**: Borderless transcript, a single status rule above the input carrying cwd and conditional non-zero exit code, a blank line between blocks, and a colorized `λ` prompt prefix.

**Independent Test**: No box around the transcript; one rule above the input shows cwd; exit 0 shows no code, `false` shows the code; two commands have a blank line between blocks; each command is echoed with a colorized `λ`.

### Tests for User Story 2 ⚠️ (write first, must fail)

- [X] T019 [P] [US2] Create tests/chrome.rs: assert the layout produces no transcript border and a single status-rule line above the input (FR-005/006).
- [X] T020 [P] [US2] Add to tests/chrome.rs: status rule shows cwd always; shows exit code only when non-zero (0 hidden); a blank line separates adjacent blocks; the command echo is prefixed with the configured `λ` and styled with `prompt_color` (FR-007/008/009/010/011).
- [X] T021 [P] [US2] Extend tests/config.rs: `prompt_char`/`prompt_color` defaults applied when absent, valid values parsed, multi-char `prompt_char` and unknown color warn-and-default (FR-023).

### Implementation for User Story 2

- [X] T022 [US2] Change the layout to `[Min(1) transcript, Length(1) status-rule, Length(input_height) input]` (rule above input; drop the `+2` border allowance) in src/ui/mod.rs (FR-005/006).
- [X] T023 [US2] Render the transcript borderless using styled `Line`/`Span`s, insert a blank line between blocks, and prefix each command echo with the configured `λ` styled in `prompt_color` (suppressed under `NO_COLOR`) in src/ui/transcript.rs (FR-005/009/010/011).
- [X] T024 [US2] Render the single status rule with the cwd (always) and the last exit code only when non-zero in src/ui/status.rs (FR-007/008).

**Checkpoint**: Chrome is borderless with a single informative rule and colorized `λ`; chrome/config tests green.

---

## Phase 6: User Story 3 - Run interactive programs without corruption or residue (Priority: P1)

**Goal**: Forward only the user's keystrokes to alt-screen programs (no injected OSC responses) and restore the terminal cleanly on every passthrough exit.

**Independent Test**: `vi test.txt` receives no spurious characters; `:q` restores the split-pad UI; `bpytop` exit restores the terminal cleanly every time.

### Tests for User Story 3 ⚠️ (write first, must fail)

- [X] T025 [P] [US3] Extend tests/passthrough.rs: assert that in passthrough, stdin is forwarded verbatim and a simulated terminal OSC 11 response on stdin is NOT delivered as encoded key input to the program (FR-012).
- [X] T026 [P] [US3] Extend tests/passthrough.rs: assert the restore sequence on alt-screen exit emits an explicit SGR/cursor reset and returns to normal mode with the prior transcript intact (FR-013).

### Implementation for User Story 3

- [X] T027 [US3] Route raw stdin verbatim to the child during passthrough (bypass `encode_key` mangling so OSC responses are not re-interpreted) in src/app.rs and src/ui/passthrough.rs (FR-012).
- [X] T028 [US3] On passthrough exit, emit an explicit SGR/cursor reset and ensure the RAII restore returns the terminal to a clean normal-mode state every time, in src/ui/passthrough.rs (FR-013).

**Checkpoint**: `vi`/`bpytop` run with no spurious input and clean restore on 100% of exits; passthrough tests green.

---

## Phase 7: User Story 5 - Use color, accurate cwd, scrolling, and `/exit` (Priority: P2)

**Goal**: Working chrome color + `NO_COLOR`, OSC 7 cwd tracking, `/exit` alias, and keyboard transcript scrolling documented in `/help`.

**Independent Test**: `λ` is red with color enabled and suppressed under `NO_COLOR`; `cd /tmp` updates the status cwd; `/exit` quits; PgUp/PgDn/Home/End scroll the transcript and are listed in `/help`.

### Tests for User Story 5 ⚠️ (write first, must fail)

- [X] T029 [P] [US5] Create tests/cwd_osc7.rs: an OSC 7 `ESC]7;file://host/tmp ST` sequence parses to `Boundary::Cwd(/tmp)` and updates `App.cwd` (FR-019).
- [X] T030 [P] [US5] Create tests/scrolling.rs: PgUp/PgDn change the scroll offset by a page (clamped at both ends), Home sets max offset (top), End sets offset 0 (bottom); submit resets to bottom (FR-021).
- [X] T031 [P] [US5] Extend the slash-command test (tests/input_router.rs or equivalent) to assert `/exit` dispatches to the same Quit action as `/quit`, and that `/help` text lists `/exit` and the scrolling keys (FR-020/022).

### Implementation for User Story 5

- [X] T032 [P] [US5] Parse OSC param 7 (`file://host/abs-path`) into `Boundary::Cwd(PathBuf)` in `osc_dispatch`, and handle `Boundary::Cwd` in `App` by updating `cwd` in src/output/parser.rs and src/app.rs (FR-019).
- [X] T033 [P] [US5] Emit OSC 7 from the prompt hooks: fish `--on-event fish_prompt` and bash `PROMPT_COMMAND` echo `ESC]7;file://$hostname$PWD ST`; sentinel-fallback shells get none, in src/pty/shell.rs (FR-019).
- [X] T034 [P] [US5] Map `/exit` to `SlashCommand::Quit` (alias of `/quit`) and update `/help` body to list `/exit` plus the scrolling keys (PageUp/PageDown, Home/End) in src/slash/builtins.rs (FR-020/022).
- [X] T035 [US5] Implement keyboard scrolling in the event loop / transcript: PgUp/PgDn scroll by a page, Home/End jump to top/bottom, clamped, with submit re-pinning to bottom, in src/app.rs and src/ui/transcript.rs (FR-021).
- [X] T036 [US5] Verify chrome color renders when enabled and is fully suppressed under `NO_COLOR` (no styling on the `λ` prompt or rule) — wire the `NO_COLOR` check through the chrome render path in src/ui/transcript.rs and src/ui/status.rs (FR-018).

**Checkpoint**: All five stories independently functional; cwd/scrolling/slash tests green.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, validation, and the code-standards gate.

- [X] T037 [P] Update docs/architecture.md with output-normalization, OSC 7 cwd, the bounded event loop, and the borderless chrome layout (Constitution II/V).
- [X] T038 [P] Update docs/usage.md (scrolling keys, `λ`/`prompt_color` config, `/exit`), docs/setup.md (config keys), and README.md (key bindings + chrome description) (Constitution V).
- [X] T039 [P] Refresh docs/specification.md to fold in the 002 hardening behavior (Constitution I).
- [X] T040 [P] Add a CHANGELOG.md entry for the 002 hardening sprint.
- [X] T041 Run the full gate: `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test` and resolve any failures (Constitution IV).
- [~] T042 **DEFERRED** (re-architecture pivot) — Execute [quickstart.md](quickstart.md) manual TTY validation for SC-001…SC-009 on a real terminal (flood timing, Ctrl-C, `vi`/`bpytop` restore, cwd follow, scrolling, `/exit`, `NO_COLOR`). Partial manual signoff was done; full re-walk is deferred because the planned terminal-grid re-architecture (see `specs/planning/grid-pivot/`) will reshape the render/passthrough paths these scenarios exercise. Re-validate against the reworked grid model rather than the soon-to-be-superseded MVP renderer.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately.
- **Foundational (Phase 2)**: Depends on Setup. Blocks US2 (chrome color) and US5 (cwd/`/exit`).
- **User Stories (Phases 3–7)**: Each depends on Foundational. Ordered D → A+B → C → E by the recommended work order; US4, US1, US3 need only Foundational; US2 and US5 consume the Foundational `Config`/`Boundary::Cwd` plumbing.
- **Polish (Phase 8)**: Depends on all targeted stories being complete.

### User Story Dependencies

- **US4 (P1, Phase 3)**: Only Foundational. Fully independent (ringbuf + event loop).
- **US1 (P1, Phase 4)**: Only Foundational. Independent (parser + transcript clear).
- **US2 (P1, Phase 5)**: Foundational (Config keys). Builds on US1's clean surface but is independently testable; the blank line / `λ` / borderless layout stand alone.
- **US3 (P1, Phase 6)**: Only Foundational. Independent (passthrough I/O).
- **US5 (P2, Phase 7)**: Foundational (`Config`, `Boundary::Cwd`, `App.cwd`). Independent; touches parser/hooks/slash/scroll.

### Within Each User Story

- Tests are written first and MUST fail before implementation.
- ringbuf/parser/config edits precede UI wiring; UI precedes integration.

### Parallel Opportunities

- Foundational T003/T004 are `[P]` (different files); T005 depends on nothing but App.
- Within each story, the `[P]` test tasks (different files) run in parallel before implementation.
- US5 implementation tasks T032/T033/T034 touch different files and are `[P]`.
- Polish docs tasks T037–T040 are `[P]`.
- If staffed in parallel after Foundational: US4, US1, US3 can proceed simultaneously; US2/US5 once Foundational lands.

---

## Parallel Example: User Story 4 tests

```bash
# Write both failing tests first (same file, but independent cases):
#   T006 caps parity test
#   T007 tail fast-path + flood-budget test
cargo test --test caps   # confirm RED before implementing T008–T012
```

---

## Implementation Strategy

### MVP scope

The trustworthiness MVP is **US4 (Phase 3)** — it removes the flood hang that
makes kapollo unusable under heavy output. Ship/verify that first.

### Incremental delivery (recommended order D → A+B → C → E)

1. **Phase 1–2** Setup + Foundational.
2. **Phase 3 (US4)** Performance — independently verifiable; the headline fix.
3. **Phase 4 (US1) + Phase 5 (US2)** Render + chrome together (removing the frame shrinks the render-corruption surface).
4. **Phase 6 (US3)** Passthrough robustness.
5. **Phase 7 (US5)** Quick wins (color/cwd/scroll/`/exit`).
6. **Phase 8** Polish, gate, and quickstart validation.

Each story is independently testable and can be demoed at its checkpoint.
