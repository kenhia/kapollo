<!-- Sync Impact Report
Version change: N/A → 1.0.0 (initial ratification)
Added principles:
  - I. Spec-Driven Development (SDD)
  - II. Architecture First
  - III. Test-Driven Development (TDD)
  - IV. Code Standards Gate
  - V. Documentation
  - VI. Quality & Observability (Terminal UX)
  - VII. Simplicity & Intentional Design
Added sections:
  - Core Principles
  - Directory Structure
  - Pre-Commit Checks
  - Development Workflow
  - Governance
Removed sections: none
Templates requiring updates:
  - .specify/templates/plan-template.md ✅ no changes needed
  - .specify/templates/spec-template.md ✅ no changes needed
  - .specify/templates/tasks-template.md ✅ no changes needed
Follow-up TODOs: none
-->

# kapollo Constitution

kapollo is a Rust-based terminal REPL that splits the terminal into a
command-input region and a command-output region (in the style of modern
agentic AI CLIs), wrapping the user's shell and layering additional
features on top.

## Core Principles

### I. Spec-Driven Development (SDD)

All changes MUST be documented in `/specs/` before implementation
begins. Iteration-scoped changes live in their spec directory
(e.g., `/specs/001-feature-name/spec.md`). Ad-hoc changes that fall
outside an active spec MUST be added to the current spec or to
`/specs/supplemental-spec.md`.

A combined specification at `/docs/specification.md` MUST be created
or updated during the polish phase of each spec and following any
ad-hoc changes. This serves as the canonical, up-to-date reference
for the full system.

- No code change without a corresponding spec entry.
- Specs define acceptance criteria before implementation starts.
- Spec updates are part of the definition of done for every iteration.

### II. Architecture First

An architecture document at `docs/architecture.md` MUST be maintained
as the authoritative technical reference for kapollo. Because kapollo
multiplexes a PTY, manages a split TUI, and integrates a feature layer
on top of an underlying shell, architectural clarity is essential.

- The architecture document MUST be updated during the polish phase
  of each spec and following ad-hoc changes.
- Supporting documents MUST remain consistent with the architecture
  document.
- Architectural decisions (terminal backend, PTY handling, event loop,
  shell integration strategy) MUST be recorded with rationale.
- Implementation MUST NOT diverge from the documented architecture
  without updating the architecture document first.

### III. Test-Driven Development (TDD)

TDD is mandatory for all new code changes. The Red-Green-Refactor
cycle MUST be followed:

1. Write a failing test that captures the requirement.
2. Implement the minimum code to make the test pass.
3. Refactor while keeping tests green.

- Tests MUST exist before or alongside the code they validate.
- Test coverage MUST NOT decrease with new changes.
- Integration tests are required for cross-component boundaries
  (PTY ↔ renderer, input router ↔ shell, feature layer ↔ core).
- Terminal/PTY behavior that cannot be unit-tested in isolation MAY
  be covered by integration or smoke tests against a headless terminal
  harness; this exception MUST be documented in the spec.

### IV. Code Standards Gate

All code MUST pass the following checks before commit:

1. **Formatted** — code is auto-formatted per ecosystem tooling.
2. **Linted** — no lint errors or warnings.
3. **Type-checked** — static type analysis passes.
4. **Unit tests** — all tests pass.

The CI variant of each check (strict/non-interactive) MUST pass
clean. This applies to both new and existing code — no broken
windows.

Use of `cargo clippy --fix` against dirty trees or any flag that
auto-applies semantically risky changes MUST be approved by the user
before execution.

See [Pre-Commit Checks](#pre-commit-checks) for specific tooling.

### V. Documentation

Each iteration (spec/sprint) MUST update user-facing documentation:

- **README.md** — project overview and getting started
- **Architecture** — `docs/architecture.md`
- **Setup** — `docs/setup.md`
- **Usage** — `docs/usage.md` (key bindings, command palette, config)

Documentation updates are part of the definition of done for every
iteration, not a follow-up task. If a feature changes how kapollo is
built, configured, or used, the docs MUST reflect that before the
iteration is complete.

### VI. Quality & Observability (Terminal UX)

kapollo is a terminal-first interactive tool; UX consistency and
debuggability are core requirements.

- **TUI rendering**: The split-region layout (input region, output
  region, status/chrome) MUST behave consistently across resizes,
  scroll, and focus changes. No flicker, no lost output, no orphaned
  cursor.
- **Wrapped shell fidelity**: The wrapped shell MUST receive an
  environment that preserves expected behavior (TTY semantics, signal
  forwarding, exit codes, working directory). Deviations MUST be
  documented and opt-in.
- **CLI/launch output**: Errors to stderr, structured/JSON output
  available for programmatic use when kapollo is invoked
  non-interactively. Honor `NO_COLOR` and detect non-TTY stdout.
- **Error messages**: Actionable — tell the user what went wrong and
  what they can do about it. Never expose raw stack traces or panics
  in release mode; panics MUST be caught at the event-loop boundary
  and surfaced as a recoverable error with logs.
- **Logging**: Structured, leveled (via `tracing` or equivalent), and
  written to a file sink by default so log output never corrupts the
  TUI. Default verbosity MUST be quiet; debug logging MUST be opt-in
  via flag or env var.

### VII. Simplicity & Intentional Design

Every addition MUST justify its complexity. YAGNI applies:

- Do not add features, abstractions, or configuration options for
  hypothetical future requirements.
- Prefer explicit over implicit behavior.
- Start with the simplest approach that meets the spec; refactor
  only when measured need arises.
- Defensive coding at system boundaries only (user input, PTY I/O,
  filesystem, config parsing). Trust internal code and framework
  guarantees.

## Directory Structure

| Directory | Purpose | Git tracked |
|-----------|---------|-------------|
| `.scratch-agent/` | Temporary workspace for agent use | No (`.gitignore`) |
| `.scratch/` | Temporary workspace for user use | No (`.gitignore`) |
| `docs/` | Project documentation (architecture, setup, usage) | Yes |
| `specs/` | Iteration and supplemental specifications (SDD) | Yes |
| `src/` | Rust source for the kapollo binary | Yes |
| `tests/` | Integration tests | Yes |

## Pre-Commit Checks

### Rust

```bash
# Standard
cargo fmt
cargo clippy --all-targets --all-features
cargo check
cargo test

# CI variant (must pass clean before commit)
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Development Workflow

1. **Spec** — Define or update the spec (`/specs/`).
2. **Plan** — Create implementation plan from spec.
3. **Implement** — Follow TDD; write tests first, then code.
4. **Check** — Run pre-commit checks (format, lint, type, test).
5. **Document** — Update `docs/` (architecture, setup, usage) as needed.
6. **Review** — Verify constitution compliance before commit.

Ad-hoc changes follow the same workflow but reference
`/specs/supplemental-spec.md` instead of a feature spec.

## Governance

This constitution supersedes all other development practices for the
kapollo project. All code changes, reviews, and architectural
decisions MUST verify compliance with these principles.

**Amendment procedure**:

1. Propose the change with rationale.
2. Document the amendment in this file.
3. Update the version number per semantic versioning:
   - **MAJOR**: Principle removal or backward-incompatible redefinition.
   - **MINOR**: New principle or materially expanded guidance.
   - **PATCH**: Clarifications, wording, or typo fixes.
4. Update `LAST_AMENDED_DATE`.
5. Propagate changes to dependent templates and documentation.

**Compliance review**: Every commit MUST pass the Code Standards Gate.
Architecture and spec alignment are verified during iteration polish.

**Version**: 1.0.0 | **Ratified**: 2026-05-29 | **Last Amended**: 2026-05-29
