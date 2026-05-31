# Implementation Plan: kapollo MVP Hardening — Render, Chrome, Passthrough & Performance

**Branch**: `002-mvp-hardening` | **Date**: 2026-05-30 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/002-mvp-hardening/spec.md`

## Summary

A consolidation/hardening sprint that closes correctness bugs and Definition-of-Done
gaps found in the first user test ([.scratch/kapollo-mvp-usertest.md](../../.scratch/kapollo-mvp-usertest.md))
before Tier 1 work begins. Five fix clusters: (A) clean the render pipeline so
captured output is normalized to printable text and the renderer owns the whole
surface; (B) redesign the chrome — borderless transcript, a single status rule,
conditional exit code, blank line between blocks, a colorized `λ` prompt; (C)
make passthrough robust — no spurious control chars into interactive programs and
a clean terminal restore on every alt-screen exit; (D) fix the performance hang —
amortized O(1) ring-buffer cap enforcement and a bounded, interruptible event
loop; (E) quick DoD wins — working chrome color + `NO_COLOR`, OSC 7 cwd tracking,
`/exit` alias, and keyboard transcript scrolling documented in `/help`. The work
conforms to the committed architecture ([docs/architecture.md](../../docs/architecture.md))
and the brainstorm decisions, especially D22 (chrome color now, block ANSI later),
D23 (cwd via OSC 7 hook), and D24 (keyboard scrolling now, mouse opt-in later).

## Technical Context

**Language/Version**: Rust (stable, edition 2021; pinned via `rust-toolchain.toml`)  
**Primary Dependencies**: `ratatui` 0.29 + `crossterm` 0.28 (TUI/events), `portable-pty` 0.8 (PTY), `vte` 0.13 (ANSI/OSC parsing), `serde` + `toml` 0.8 (config), `tracing` + `tracing-subscriber` + `tracing-appender` (file logging), `anyhow`/`thiserror` (errors), `directories` 5 (XDG paths). No new runtime dependencies are anticipated; all fixes are internal.  
**Storage**: In-memory transcript (ring-buffered block output); config at `~/.config/kapollo/config.toml`; file log under XDG dir. No database.  
**Testing**: `cargo test` (unit + integration); render normalization, ring-buffer caps, and parsing are unit-tested; passthrough/cwd/interrupt behaviors are covered by integration/smoke tests against a headless PTY harness (Constitution III exception, carried over from 001 and recorded in research.md).
**Target Platform**: Linux only (D9).  
**Project Type**: Single binary crate (CLI/desktop-terminal app); binary names `kap` and `kapollo`.
**Performance Goals**: `yes | head -n 5000000` completes in roughly shell-native wall-clock time (same order of magnitude as the bare shell, not minutes); UI stays responsive throughout; Ctrl-C interrupts a flood within a small bounded delay (target ≤ ~100 ms perceived). Cap enforcement is amortized ~O(1) per byte.  
**Constraints**: Existing ring-buffer cap semantics unchanged (per-block 1 MiB / 50k lines, hard max 64 MiB; transcript 128 MiB / 1000 blocks) — only the enforcement *algorithm* changes. No terminal grid model (D4/§6): ANSI parsing is used solely for boundary/alt-screen detection, style stripping, and OSC 7/133 handling. Logs never to screen; panics caught at the event-loop boundary; terminal always restored; honor `NO_COLOR`.  
**Scale/Scope**: Single-user interactive session; this sprint = 23 functional requirements (FR-001–FR-023) across 5 user stories; touches the `output`, `session`, `ui`, `pty`, `input`, `slash`, and `config` modules (no new top-level modules expected).  

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Spec-Driven Development | PASS | spec.md authored before implementation; this plan derives from it. `docs/specification.md` updated in the polish phase. |
| II. Architecture First | PASS | Conforms to `docs/architecture.md`; changes to the output-normalization pipeline, OSC 7 cwd handling, bounded event loop, and chrome layout are recorded there during polish (D22–D24 already in the brainstorm decisions log). |
| III. Test-Driven Development | PASS (with documented exception) | TDD followed; render/ring-buffer/parser changes are unit-tested first. Interactive passthrough, OSC 7 cwd, and interrupt-under-flood are covered by integration/smoke tests against a headless PTY harness where unit isolation is impractical — permitted by Principle III and recorded in research.md. |
| IV. Code Standards Gate | PASS | Rust gate: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`. CI workflow already exists from 001. |
| V. Documentation | PASS | README (key bindings, chrome), docs/usage.md (scrolling keys, `λ`/color config, `/exit`), docs/setup.md, docs/architecture.md, and docs/specification.md updates are explicit polish-phase deliverables. |
| VI. Quality & Observability | PASS | File-sink logging unchanged; panic boundary + terminal restore reinforced by US3; structured errors; `NO_COLOR`; consistent rendering across resize/scroll is a first-class goal of US1/US2. |
| VII. Simplicity & Intentional Design | PASS | No new abstractions beyond what the fixes require; block ANSI color and mouse capture are explicitly deferred (D22/D24); defensive coding stays at boundaries (PTY I/O, parser, config). |

**Result**: PASS. No violations; Complexity Tracking not required.

## Project Structure

### Documentation (this feature)

```text
specs/002-mvp-hardening/
├── plan.md              # This file (/speckit.plan command output)
├── spec.md              # Feature specification
├── research.md          # Phase 0 output (decisions for the 5 fix clusters)
├── data-model.md        # Phase 1 output (entity deltas: OutputBuffer, Block, Config, ChromeState)
├── quickstart.md        # Phase 1 output (manual + automated validation steps)
├── contracts/           # Phase 1 output (deltas only)
│   ├── config.md        # new prompt_char / prompt_color keys
│   ├── shell-hooks.md   # OSC 7 cwd emission added to fish/bash hooks
│   ├── slash-commands.md # /exit alias; /help scrolling-keys content
│   └── keybindings.md   # transcript scrolling key map (PgUp/PgDn/Home/End)
├── checklists/
│   └── requirements.md  # spec quality checklist (already passing)
└── tasks.md             # Phase 2 output (/speckit.tasks - NOT created here)
```

### Source Code (repository root)

Existing single-crate layout (from 001); this sprint modifies files in place and
adds tests. No new top-level modules.

```text
src/
├── app.rs               # MODIFY: bounded drain (FR-015/017), passthrough input routing (FR-012),
│                        #         restore-on-return (FR-013), chrome wiring, scroll keys (FR-021)
├── config.rs            # MODIFY: add prompt_char + prompt_color keys + defaults (FR-010/011/023)
├── output/
│   ├── mod.rs           # MODIFY: surface OSC 7 cwd events to App (FR-019)
│   └── parser.rs        # MODIFY: normalize output (drop OSC responses/bare CR/residual escapes),
│                        #         add OSC 7 parsing (FR-001, FR-019)
├── session/
│   ├── block.rs         # MODIFY: rendered-text accessor returns normalized lines (FR-001/004)
│   └── ringbuf.rs       # MODIFY: incremental line count + bulk trim + truncated fast-path (FR-014/016)
├── pty/
│   └── shell.rs         # MODIFY: emit OSC 7 from fish/bash hooks (FR-019)
├── slash/
│   └── builtins.rs      # MODIFY: /exit alias text; /help lists scrolling keys (FR-020/022)
└── ui/
    ├── mod.rs           # MODIFY: layout w/o transcript border; single status rule (FR-005/006)
    ├── transcript.rs    # MODIFY: borderless, blank line between blocks, λ prefix, full-surface clear (FR-002/005/009/010)
    ├── status.rs        # MODIFY: cwd + conditional non-zero exit on the rule (FR-007/008)
    └── passthrough.rs   # MODIFY: raw stdin passthrough / response handling; restore (FR-012/013)
tests/
├── render_normalize.rs  # NEW: OSC-response/bare-CR/escape stripping; first-line parity (US1)
├── chrome.rs            # NEW: borderless layout, conditional exit, blank-line, λ prefix (US2)
├── caps.rs              # EXTEND: incremental-count correctness + bulk-trim + flood fast-path (US4)
├── passthrough.rs       # EXTEND: no spurious input; clean restore on alt-screen exit (US3)
├── cwd_osc7.rs          # NEW: OSC 7 parse → status cwd update (US5)
├── input_router.rs      # EXTEND (if needed) for /exit
└── scrolling.rs         # NEW: PgUp/PgDn/Home/End transcript scroll behavior (US5)
docs/
├── architecture.md      # polish: output-normalization, OSC 7, bounded loop, chrome
├── usage.md             # polish: scrolling keys, λ/color config, /exit
├── setup.md             # polish: any config changes
└── specification.md     # polish: combined spec refresh (Constitution I)
README.md                # polish: updated key bindings + chrome description
CHANGELOG.md             # polish: 002 hardening entry
```

**Structure Decision**: Reuse the single binary crate and its layer modules
(`pty`, `output`, `session`, `input`, `slash`, `ui`, `config`) established in 001.
All changes are in-place edits plus new/extended integration tests; introducing no
new modules keeps the change surface aligned with Principle VII and the existing
architecture.

## Complexity Tracking

> No Constitution violations. Section intentionally empty.
