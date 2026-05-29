# Implementation Plan: kapollo MVP — Split-Pad Shell REPL

**Branch**: `001-mvp-repl` | **Date**: 2026-05-29 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/001-mvp-repl/spec.md`

## Summary

kapollo is a Rust terminal application that wraps the user's real shell
(fish/bash) in a PTY and presents an Apollo-DM-style split UI: an input pad
at the bottom, an output (transcript) pad above showing each command and its
output as a discrete **block**. Normal commands stream into append-mostly
blocks; full-screen (alt-screen) programs are handed to the host terminal via
passthrough. A slash-command layer (`/quit`, `/clear`, `/help`) provides
features beyond a plain shell wrapper. Block boundaries and exit codes are
detected via auto-injected OSC 133 shell hooks (sentinel fallback). The
technical approach follows the committed architecture in
[docs/architecture.md](../../docs/architecture.md) (decisions D1–D21).

## Technical Context

**Language/Version**: Rust (stable, edition 2021; pin via `rust-toolchain.toml`)  
**Primary Dependencies**: `ratatui` + `crossterm` (TUI/events), `portable-pty` (PTY), `vte` (ANSI/OSC 133 parsing), `serde` + `toml` (config), `tracing` + `tracing-subscriber` + `tracing-appender` (file logging), `anyhow` (app errors) + `thiserror` (library errors), `directories` (XDG paths)  
**Storage**: In-memory transcript (ring-buffered block output); config file at `~/.config/kapollo/config.toml`; log file under XDG state/cache dir. No database in MVP.  
**Testing**: `cargo test` (unit + integration); PTY/terminal behavior covered by integration tests against a headless PTY harness (constitution III exception, documented here)  
**Target Platform**: Linux only for MVP (D9)  
**Project Type**: Single binary crate (CLI/desktop-terminal app); binary names `kap` and `kapollo`  
**Performance Goals**: Responsive interactive UI — input-to-render latency imperceptible (<50 ms typical); sustain high-volume command output (e.g. `yes`/multi-MB) without UI stall, by capping per-block and transcript retention. NOTE: performance is observed/measured during validation, but dedicated performance optimization work is out of scope for the MVP (revisit post-MVP if measured need arises).  
**Constraints**: Bounded memory via ring-buffer caps (per-block default 1 MiB / 50k lines, hard max 64 MiB; transcript default 128 MiB / 1000 blocks); TUI integrity — logs never to screen, panics caught at event-loop boundary, terminal always restored; honor `NO_COLOR`; no TUI when stdout is not a TTY  
**Scale/Scope**: Single-user interactive session; MVP scope = 33 functional requirements (FR-001–FR-033) across 4 user stories; ~10 internal modules per architecture §7

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Spec-Driven Development | PASS | spec.md authored before implementation; this plan derives from it. `docs/specification.md` to be created during polish phase. |
| II. Architecture First | PASS | `docs/architecture.md` is the authoritative reference (D1–D21); plan conforms. Will be updated during polish. |
| III. Test-Driven Development | PASS (with documented exception) | TDD followed; PTY/terminal interactive behavior covered by integration/smoke tests against a headless PTY harness where unit isolation is impractical — explicitly permitted by Principle III and recorded in research.md. |
| IV. Code Standards Gate | PASS | Rust gate: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`. Wired into CI per polish tasks. |
| V. Documentation | PASS | README, docs/setup.md, docs/usage.md, and architecture updates are explicit polish-phase deliverables (see Release & Repo Readiness). |
| VI. Quality & Observability | PASS | File-sink logging via tracing; panic boundary restores terminal; structured errors; `NO_COLOR`; non-TTY handling — all captured as requirements. |
| VII. Simplicity & Intentional Design | PASS | No grid model (D4); wrap real shell rather than reimplement (D1); post-MVP features explicitly deferred. Defensive coding at boundaries (PTY, config, input) only. |

**Result**: PASS. No violations; Complexity Tracking not required.

## Project Structure

### Documentation (this feature)

```text
specs/001-mvp-repl/
├── plan.md              # This file (/speckit.plan command output)
├── spec.md              # Feature specification
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
│   ├── cli.md           # CLI invocation & flags contract
│   ├── config.md        # config.toml schema contract
│   ├── slash-commands.md # built-in slash command contract
│   └── shell-hooks.md   # OSC 133 hook / sentinel contract
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created here)
```

### Source Code (repository root)

```text
Cargo.toml               # package metadata (license=MIT, description, repo), bins: kap + kapollo
rust-toolchain.toml      # pinned stable toolchain
src/
├── main.rs              # arg parse, config load, terminal setup/teardown, panic boundary wiring
├── app.rs               # App, State, event loop
├── config.rs            # TOML config (serde), defaults, XDG paths
├── logging.rs           # tracing → file appender
├── pty/
│   ├── mod.rs           # PTY spawn, write stdin, read output, resize, signals
│   └── shell.rs         # shell detection, hook installation (fish/bash), KAPOLLO_ACTIVE
├── output/
│   ├── mod.rs           # output processor orchestration
│   ├── parser.rs        # vte parsing, OSC 133, alt-screen detection
│   └── sentinel.rs      # fallback boundary detection
├── session/
│   ├── mod.rs           # transcript model
│   ├── block.rs         # Block, flags, exit code, timestamps
│   └── ringbuf.rs       # capped output storage + caps enforcement
├── input/
│   ├── mod.rs           # input pad model, multiline, history
│   └── router.rs        # slash vs pass-through, leader/escape
├── slash/
│   ├── mod.rs           # slash command registry
│   └── builtins.rs      # /quit /clear /help
└── ui/
    ├── mod.rs           # layout orchestration
    ├── input_pad.rs
    ├── transcript.rs
    ├── status.rs
    └── passthrough.rs   # alt-screen handoff
tests/
├── config.rs            # config defaults, unknown-key tolerance, cap clamping, invalid TOML
├── pty_smoke.rs         # headless PTY: spawn shell, run command, capture output
├── block_boundaries.rs  # OSC 133 + sentinel block delimiting & exit codes
├── input_router.rs      # slash detection, doubled-leader escape
├── signals.rs           # SIGINT forwarding + clean teardown
├── input_pad.rs         # multiline compose + input-history recall
├── passthrough.rs       # alt-screen handoff & UI restore
└── caps.rs              # per-block + transcript ring-buffer caps & truncation
docs/
├── architecture.md      # already authored (authoritative)
├── setup.md             # build/install/run (polish)
├── usage.md             # keys, slash commands, config (polish)
└── specification.md     # combined spec (polish; constitution I)
README.md                # polish: overview, install, usage, screenshot, kap alias
LICENSE                  # polish: MIT
.gitignore               # Rust ignores
```

**Structure Decision**: Single binary crate (architecture §7). Internal
modules mirror the architecture's layers (pty, output, session, input, slash,
ui) so they could be promoted to crates later without redesign. Two binary
targets (`kap`, `kapollo`) share the same library code. Integration tests in
`tests/` exercise cross-component boundaries (PTY ↔ output ↔ session ↔ input
router) per Constitution III.

## Release & Repo Readiness (polish phase)

Per Constitution V (Documentation) and the user's request, the following are
in-scope deliverables for this milestone, executed as polish-phase tasks in
`tasks.md` (ordered late — the screenshot depends on a working TUI):

- **README.md** — project overview, what kapollo is (Apollo-DM inspiration),
  install/build instructions, basic usage, the `kap` alias, key bindings, a
  screenshot (or asciinema cast) of kapollo running, and license badge.
- **LICENSE** — MIT.
- **Cargo.toml metadata** — `license = "MIT"`, `description`, `repository`,
  `keywords`, `categories`, authors.
- **.gitignore** — Rust defaults (`/target`, etc.) plus local scratch dirs.
- **docs/setup.md** and **docs/usage.md** — build/install and usage/keys/config.
- **docs/specification.md** — combined specification (Constitution I).
- **CI workflow** — runs the Code Standards Gate (fmt --check, clippy -D
  warnings, test) on push/PR.
- **Pre-push verification** — Code Standards Gate clean (Constitution IV).
- **CHANGELOG.md** — initial entry for the MVP (optional but recommended).

## Complexity Tracking

> No Constitution violations. Section intentionally empty.
