# Implementation Plan: LAAT Mode, `/save`, `/filter`, and `/load`

**Branch**: `007-laat-mode` | **Date**: 2026-06-07 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/007-laat-mode/spec.md`

## Summary

Give kapollo its first real **modes**. Sprint 005 shipped a multi-line input
buffer and a fixed status bar with a reserved 4-column mode field; sprint 006
turned every binding into a **named, rebindable action**. This feature adds three
input modes — `Norm`, `Mult`, and `LaaT` — and the slash commands that work with
prior command output, under one mental model: *stepping through commands and
working with their output*.

- **`Mult` mode** fixes the 005 sharp edge where `Up`/`Down` in a multi-line
  buffer recall history and discard the draft: in `Mult` the arrows move the caret
  between lines, with **chat-style edge recall** (stash the draft on `Up` at the
  top, restore it on `Down`).
- **`LaaT` (Line-At-A-Time) mode** is "`Mult` + highlight + step + exit-code
  gating": a highlight steps line-by-line, `Enter` submits the highlighted line as
  a normal submission, and the highlight advances only on exit `0` while a
  non-zero exit flags a **probable** failure.
- A **one-item push/pop input stack** (`Ctrl+Alt+Enter`) saves the buffer+mode,
  drops to `Norm` for an ad-hoc command, and restores on the next submit.
- **`/save <file>`**, **`/filter <cmd>`**, and **`/load <file>`** write, pipe, and
  load prior output and scripts.

The work is **additive and in-place** (the 004/005/006 pattern). The stable layers
— PTY, grid, block store, status bar, input router, slash registry, keymap engine
— carry over. This feature grows `src/input` (vertical caret motion + the stashed
draft), adds an `InputMode` surfaced to the status bar, registers two new named
actions in the 006 keymap engine (`ToggleMultLaat` = `Ctrl+1`, `PushInput` =
`Ctrl+Alt+Enter`), adds a small LAAT stepping state gated on the existing
`CommandEnd` exit-code observation, extends the slash layer with three
argument-bearing commands plus a one-key `/save` overwrite prompt, and threads the
mode through `App::on_key`. Realizes the resolved pre-plan decisions for sprint 007
([pre-plan-007-laat-mode.md](../planning/pre-plan-007-laat-mode.md)) with zero
`[NEEDS CLARIFICATION]` markers.

## Technical Context

**Language/Version**: Rust 1.96.0 (edition 2021; CI `@stable` resolves to 1.96.0,
local toolchain aligned — carried over from 004/005/006).

**Primary Dependencies**: No new crates expected. Existing stack carries the work:
- `crossterm 0.29` — the event source; `KeyCode`/`KeyModifiers` for the new
  `Ctrl+1` / `Ctrl+Alt+Enter` chords (the 006 key-string parser already
  accepts both — verified in `parse_key`).
- `ratatui 0.30` — the input-pad renderer gains the LAAT highlight + probable-
  failure background; the status bar's mode field becomes dynamic.
- `serde` + `toml` — only the existing top-level `[keymap]` table is touched (two
  new action names ship defaults); **no** new config sections (per-mode keymap
  tables are out of scope).
- Retained: `tracing` (diagnostics), `anyhow`/`thiserror` (errors), `wezterm-term`
  (git-pinned, unchanged). `/filter` uses `std::env::temp_dir` + the wrapped shell
  — no new dependency.

**Storage**: None persisted. New state is in-memory interaction state on `App`:
`mode: InputMode`, an optional `LaatState`, an optional one-item `InputSnapshot`
stack, and an optional `PendingPrompt` for the `/save` overwrite confirmation.
`InputHistory` gains a stashed-draft field. `/save` and `/filter` read the existing
`BlockStore` (the canonical retained output, R3) and touch the filesystem only at
the save target and a `/filter` temp file.

**Testing**: `cargo test` (unit + integration), TDD per Constitution III. The
feature is **mostly pure logic** and is unit-tested first: the `InputMode`
transition rules, vertical caret motion + edge detection on `InputPad`, the
stashed-draft recall on `InputHistory`, the LAAT exit-code gating function
(`pending` + exit → advance/flag), the `InputSnapshot` save/restore, slash dispatch
of the three argument-bearing commands, and the two keymap defaults (in the
existing `keymap_*` suites). Only the live-TTY pieces — the rendered mode label and
LAAT highlight, the `/save` overwrite prompt key-handling, and the `/filter` shell
round-trip — use the documented Constitution III integration/manual exception,
validated by [quickstart.md](quickstart.md) mapped to SC-001…007.

**Target Platform**: Linux-first; built directly on the 004/005/006 chrome.

**Project Type**: Single Rust binary crate (TUI terminal app) — `kapollo`/`kap`.

**Performance Goals**: Interactive feel only. Mode resolution and caret motion are
per-keystroke O(lines); LAAT gating reacts to the already-observed `CommandEnd`
boundary. No throughput target.

**Constraints**: Constitution VII (simplicity) is the governing gate — a one-item
stack (not a vector), a one-key prompt (not a modal widget), two global named
actions (not per-mode config tables). Constitution VI: the `/save` prompt and
`/filter` errors must surface as status messages, never panics; no buffer is ever
silently lost (SC-002/003/006).

**Scale/Scope**: Additive. Touches `src/input` (vertical caret motion + stashed
draft), `src/action` (two new actions + defaults), `src/app.rs` (mode field,
mode-aware Up/Down, LAAT state + gating hook, push/pop, the three slash handlers +
prompt), `src/slash` (argument-bearing `Save`/`Filter`/`Load` + help), `src/ui`
(dynamic mode label, LAAT highlight render), and `docs/keymap-defaults.toml` (two
new defaults). No module is rewritten.

### Open clarifications

None blocking. All ten pre-plan open questions were resolved by the user before
specification ([pre-plan-007-laat-mode.md](../planning/pre-plan-007-laat-mode.md))
and encoded in the spec with zero `[NEEDS CLARIFICATION]` markers. The design
choices with options (where the mode lives; how the stashed draft extends
`InputHistory`; the LAAT gating hook; the `/save` prompt primitive; the `/filter`
temp-file model; argument-bearing slash dispatch; the one-item snapshot) are
resolved in [research.md](research.md) (R1–R10).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. Spec-Driven Development** | ✅ | Fully specced ([spec.md](spec.md)); `docs/specification.md` updated in the polish phase. |
| **II. Architecture First** | ✅ | No architectural reversal. Modes are the natural next layer the 005 status mode-field and the 006 named-action engine were built for. `docs/architecture.md` gains an "input modes / LAAT (007)" section in the polish phase; no pre-implementation architecture change is needed. |
| **III. Test-Driven Development** | ✅ | The bulk (mode transitions, caret motion, stashed-draft recall, LAAT gating function, snapshot save/restore, slash dispatch, keymap defaults) is pure logic, TDD'd first. Live-TTY mode label / highlight / `/save` prompt / `/filter` round-trip use the documented integration/manual exception. No coverage decrease. |
| **IV. Code Standards Gate** | ✅ | `cargo fmt --check` + `cargo clippy --all-targets --all-features -- -D warnings` + `cargo test` clean on 1.96.0. |
| **V. Documentation** | ✅ | README (modes + `/save`/`/filter`/`/load`), `docs/usage.md` (mode labels, `Ctrl+1`/`Ctrl+Alt+Enter`, LAAT stepping, the three slash commands), `docs/setup.md` (the two new keymap actions), `docs/architecture.md`, `docs/specification.md` updated as definition-of-done. `docs/keymap-defaults.toml` gains the two new defaults, kept honest by the existing sync test. |
| **VI. Quality & Observability (TUI)** | ✅ | The mode field always reflects the true mode; the LAAT highlight and probable-failure flag are rendered; no buffer is silently lost (SC-002/003/006); `/save` errors and the overwrite prompt and `/filter` non-zero exits surface as status messages, never panics. |
| **VII. Simplicity & Intentional Design** | ✅ | The governing gate. One-item stack (not a vector), one-key prompt (not a modal widget), two **global** named actions (per-mode config tables explicitly out of scope), `/filter` reuses the existing shell+capture pipeline via a temp file (no parallel output path). No cross-session LAAT persistence. |

**Gate result: PASS.** No unjustified violations; Complexity Tracking not required.
The forward-looking surface (modes as first-class state) is exactly what 005/006
were built to enable and is implemented as the minimum the spec requires.

## Project Structure

### Documentation (this feature)

```text
specs/007-laat-mode/
├── plan.md              # This file
├── research.md          # Phase 0 — decisions R1–R10 (mode location, stashed draft, LAAT gating, /save prompt, /filter temp-file, slash args, snapshot, keymap)
├── data-model.md        # Phase 1 — InputMode, InputPad/InputHistory extensions, LaatState, InputSnapshot, PendingPrompt, new Action/SlashCommand variants
├── quickstart.md        # Phase 1 — manual interactive validation script (SC-001…007)
├── contracts/           # Phase 1 — internal interface contracts
│   ├── input-modes.md   #   InputMode state machine + mode-aware Up/Down + edge recall
│   ├── laat-engine.md   #   highlight + step + exit-code gating
│   ├── push-pop-stack.md#   one-item buffer+mode snapshot
│   ├── slash-commands.md#   /save (+ overwrite prompt), /filter (+ chaining), /load
│   └── keymap-actions.md#   ToggleMultLaat + PushInput named actions & defaults
└── checklists/
    └── requirements.md  # Spec quality checklist (already complete)
```

### Source Code (repository root)

```text
src/
├── input/
│   └── mod.rs           # 🔧 InputPad: vertical caret motion (caret_line_up/down,
│                        #    caret_on_first/last_line). InputHistory: stashed-draft
│                        #    field + edge-recall entry points. NEW InputMode enum +
│                        #    label() (or in src/action).
├── action/
│   └── mod.rs           # 🔧 NEW Actions ToggleMultLaat (Ctrl+1) + PushInput
│                        #    (Ctrl+Alt+Enter): name/from_name/default_map. Parser
│                        #    already handles both chords (no parser change).
├── app.rs               # 🔧 mode: InputMode; mode-aware Up/Down in on_key; LaatState
│                        #    + exit-code gating on the existing CommandEnd hook;
│                        #    one-item InputSnapshot push/pop; /save /filter /load
│                        #    handlers + PendingPrompt (on_key consumes prompt keys
│                        #    first); dispatch arms for the two new actions.
├── slash/
│   ├── mod.rs           # 🔧 SlashCommand::Save/Filter/Load(String) + dispatch args.
│   └── builtins.rs      # 🔧 help_text lists /save, /filter, /load.
└── ui/
    ├── status.rs        # 🔧 render() passes app.mode.label() instead of DEFAULT_MODE.
    └── input_pad.rs     # 🔧 LAAT highlight + probable-failure background render.

docs/
└── keymap-defaults.toml # 🔧 add toggle_mult_laat + push_input defaults (sync test).

tests/
├── input_modes.rs       # ✨ mode transitions, vertical caret motion, edge recall stash.
├── laat_engine.rs       # ✨ highlight stepping + exit-code gating (advance/flag/recover).
├── push_pop.rs          # ✨ one-item snapshot save/restore (buffer+mode+stash+laat).
├── slash_filter_save.rs # ✨ /save /filter /load dispatch, path resolution, save bytes.
└── keymap_*.rs          # 🔧 extend: two new defaults + /keys listing + doc sync.
```

**Structure Decision**: Single Rust binary crate (unchanged). The mode and editing
primitives grow `src/input` (the pure buffer/history model) and a new `InputMode`;
the two new bindings are named actions in `src/action` (the 006 engine); the
interaction wiring — mode-aware keys, LAAT gating on the existing `CommandEnd`
observation, the push/pop snapshot, and the three slash handlers with the `/save`
prompt — lives in `App` alongside the existing `esc_pending`/`passthrough`/
`selection` state. The status bar and input-pad renderers gain the dynamic mode
label and the LAAT highlight. New test files mirror the four concern boundaries
(modes / LAAT / push-pop / slash), and the existing `keymap_*` suites are extended
for the two new defaults.

## Complexity Tracking

> No constitution violations require justification. Complexity Tracking not
> required for this feature.
