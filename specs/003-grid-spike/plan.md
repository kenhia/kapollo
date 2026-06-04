# Implementation Plan: Terminal-Grid Spike

**Branch**: `003-grid-spike` | **Date**: 2026-06-01 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/003-grid-spike/spec.md`

## Summary

Build the **same vertical slice** on three terminal-emulator crates, sequentially,
inside an isolated `delos/` Cargo workspace, and score them against one rubric to
choose the production grid crate. The slice: PTY-backed shell → crate grid/parser →
ratatui render (main screen) → content-coordinate mouse selection with
auto-scroll-on-drag-past-edge → OSC 52 copy → wheel scroll → alt-screen handover.
Stages: **S1 `vt100`** (optionally via `tui-term`) → **S2 `alacritty_terminal`** →
**S3 `wezterm-term`** (git dep; unpublished on crates.io). Output is knowledge: a
filled scorecard, per-stage writeups, and a single crate recommendation feeding
decisions D25–D30 and the in-place rework spec. This is throwaway research code,
contained so the shipping `kapollo`/`kap` crate gains **zero** spike dependencies.

## Technical Context

**Language/Version**: Rust 1.95.0 (edition 2021)
**Primary Dependencies** (spike workspace, current versions per maintainer):
- Grid/parser candidates: `vt100` 0.16.2 (+ optional `tui-term` 0.3.4),
  `alacritty_terminal` 0.26.0, `wezterm-term` (git pin — see research.md) with
  `termwiz` 0.23.3 along for the ride.
- Render: `ratatui` 0.30.0 (spike uses current; kapollo ships 0.29).
- Terminal backend / input: `crossterm` 0.29.0 (kapollo ships 0.28).
- PTY: `portable-pty` 0.9.0 (kapollo ships 0.8).
- Clipboard fallback candidate: `arboard` 3.6.1 (OSC 52 is the default path,
  hand-emitted; `arboard` only evaluated as the local fallback).

**Storage**: N/A (throwaway binaries; deliverables are Markdown under `delos/`).  
**Testing**: `cargo test` for the small pure helpers (selection coordinate math,
alt-screen/mouse-mode escape detection); manual host-terminal validation for the
interactive slice (the feel is the point — see Constitution Check).  
**Target Platform**: Linux (primary dev) + Windows Terminal Preview (primary
validation); GNOME Terminal / Konsole on Ubuntu (secondary). macOS out of scope.  
**Project Type**: Throwaway research spike — a Cargo workspace of CLI binaries.  
**Performance Goals**: Slice must stay interactive under an output flood (no
unbounded render stalls); damage/dirty-tracking behavior recorded per crate.  
**Constraints**: `delos/` is its **own Cargo workspace**; spike deps MUST NOT enter
the shipping `kapollo`/`kap` build, lockfile graph, or feature-gated bins. Shipping
crate stays green and dependency-clean throughout.  
**Scale/Scope**: Single maintainer/operator. Three throwaway binary crates + a
shared spike-support crate + Markdown deliverables. No productized UX.  

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Spec-Driven Development | PASS | Spec exists ([spec.md](spec.md)); planning under `specs/planning/grid-pivot/`. This plan precedes spike code. |
| II. Architecture First | PASS (scoped) | The spike *informs* `docs/architecture.md` (the grid pivot reverses D4) but does not change it yet. Architecture update is part of the **rework** spec, not this spike. Decisions captured in research.md + the planning docs. |
| III. Test-Driven Development | DEVIATION (justified) | The spike's value is manual *feel* evaluation of interactive PTY/mouse/render behavior that cannot be meaningfully unit-tested. Per Constitution III's documented exception, interactive behavior is covered by **manual host-terminal validation** ([quickstart.md](quickstart.md)); only the pure helpers (selection coordinate math, escape-sequence detection) get unit tests. See Complexity Tracking. |
| IV. Code Standards Gate | PASS | `delos/` workspace passes `cargo fmt --check` / `clippy -D warnings` / `cargo test` independently. Shipping crate gate unchanged and must stay green. |
| V. Documentation | PASS (scoped) | Spike deliverables are its docs (scorecard + writeups + recommendation under `delos/docs/`). README/usage/setup of the shipping product are untouched (no shipped feature). Folded into base docs during the rework. |
| VI. Quality & Observability | PASS (scoped) | Throwaway binaries; logging kept minimal. The slice must not corrupt the host TUI (raw-mode restore on exit, panic-safe terminal teardown). |
| VII. Simplicity & Intentional Design | PASS | Smallest slice that makes the rubric fillable; throwaway by design; no abstractions beyond a thin shared `spike-support` crate for the genuinely-identical plumbing. |

**Gate result**: PASS with one justified deviation (TDD relaxed for interactive
spike code, manual validation substituted per the constitution's own exception).

## Project Structure

### Documentation (this feature)

```text
specs/003-grid-spike/
├── plan.md              # This file
├── research.md          # Phase 0 — crate decisions, wezterm git pin, clipboard, mouse routing
├── data-model.md        # Phase 1 — slice/stage/scorecard/selection entities + selection state machine
├── quickstart.md        # Phase 1 — workspace setup + manual test script + host-terminal matrix
├── contracts/
│   ├── spike-binary-cli.md   # Runtime contract every spike binary obeys (args, keys, mouse, exit)
│   └── scorecard.md          # Shared rubric schema (criteria, weights, columns)
└── tasks.md             # Phase 2 (/speckit.tasks — NOT created here)
```

### Source Code (repository root)

The spike is an **isolated Cargo workspace** under `delos/`. It is NOT a member of
the root `kapollo` package and shares no dependency graph with it.

```text
delos/                          # OWN Cargo workspace (own Cargo.toml [workspace], own Cargo.lock)
├── Cargo.toml                  # [workspace] members = the crates below; resolver = "2"
├── Cargo.lock                  # separate lock — spike deps never touch kapollo's
├── README.md                   # what delos is, how to run, link back to the spec
├── docs/
│   ├── scorecard.md            # the shared rubric, three columns (filled across S1->S3)
│   ├── s1-vt100.md             # nuts-and-bolts writeup (per stage)
│   ├── s2-alacritty.md
│   ├── s3-wezterm.md
│   └── recommendation.md       # final synthesis -> feeds D25-D30 + rework spec
├── spike-support/              # thin shared crate: identical plumbing only
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs              # re-exports coords, modes, clipboard, pty
│       ├── coords.rs           # selection coord math + auto-scroll + normalize (UNIT-TESTED)
│       ├── modes.rs            # alt-screen + child mouse-mode escape detection (UNIT-TESTED)
│       ├── clipboard.rs        # OSC 52 framing + optional arboard fallback (UNIT-TESTED)
│       └── pty.rs              # portable-pty shell spawn (I/O boundary, smoke-validated)
├── spike-vt100/                # S1 binary crate (vt100 [+ optional tui-term])
│   ├── Cargo.toml
│   └── src/main.rs
├── spike-alacritty/            # S2 binary crate (alacritty_terminal)
│   ├── Cargo.toml
│   └── src/main.rs
└── spike-wezterm/              # S3 binary crate (wezterm-term via git pin)
    ├── Cargo.toml
    └── src/main.rs

# Shipping crate (UNCHANGED by this spike):
src/                            # kapollo/kap — must stay green, zero spike deps
tests/                          # existing suite continues to pass
Cargo.toml                      # MUST add exclude = ["delos"] ; delos is its own workspace
```

**Structure Decision**: A dedicated `delos/` Cargo workspace, **excluded** from the
root crate (the root `Cargo.toml` adds `exclude = ["delos"]` so `cargo build` at the
repo root never descends into it). Each stage is its own binary crate so heavy,
divergent deps (`alacritty_terminal`, `wezterm-term` git) are isolated per stage; a
single thin `spike-support` library holds only the genuinely-identical plumbing and
the unit-tested pure helpers. This satisfies FR-002/FR-003/FR-004 and SC-007 by
construction: the shipping dependency graph cannot gain a spike dependency because
the two workspaces are disjoint with separate lockfiles.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| TDD relaxed for interactive slice (Constitution III) | The spike evaluates *feel* — interactive PTY + mouse + render + alt-screen handover across real host terminals. This behavior is not meaningfully unit-testable and writing harnesses for throwaway code would cost more than the spike itself. | Full TDD rejected: a headless harness for mouse-drag/auto-scroll/alt-screen across three crates would dwarf the spike and still not validate the *feel* that is the actual deliverable. Mitigation: the pure, portable helpers (selection coordinate math, escape-sequence detection, OSC 52 framing) in `spike-support` ARE unit-tested; everything interactive is covered by the documented manual script in quickstart.md. |
| Separate `delos/` workspace + `spike-support` crate (vs. one inline example) | Required by FR-002/FR-003: spike deps must never enter the shipping graph; three crates with divergent heavy deps need isolation. | A single feature-gated example inside the kapollo crate was explicitly rejected (FR-003) — it would pull `wezterm-term`/`alacritty_terminal` into kapollo's lockfile and build. |
| Documentation MUST (Constitution V) relaxed for shipping-product docs (README/architecture/setup/usage) | The spike ships **no product feature**; updating product-facing docs now would document a renderer that the rework will replace. The spike's *own* deliverables (scorecard, per-stage writeups, recommendation under `delos/docs/`) satisfy the documentation intent for this iteration. | Updating `docs/architecture.md` et al. now rejected: it would pre-commit the architecture before the spike chooses a crate (premature, and likely rewritten). The MUST is honored at the **rework** spec, where the grid decision folds into the base docs. Tracked here so the relaxation is explicit, not silent. |
