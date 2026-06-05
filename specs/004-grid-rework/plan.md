# Implementation Plan: Grid Rework — Native Terminal Grid, Mouse Selection & Block Store

**Branch**: `004-grid-rework` | **Date**: 2026-06-04 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/004-grid-rework/spec.md`

## Summary

Replace kapollo's append-only, style-stripped transcript core with a **real terminal
grid** (rows × columns of styled cells + scrollback) driven by the `wezterm-term`
emulator selected in the 003 spike. On top of that grid, add **mouse-driven selection,
OSC 52 copy with local fallback, and scroll-wheel/scrollback** with correct hand-over to
full-screen applications, and re-home the **block model** (`command + output + exit code`)
as an annotation layer backed by an **in-memory block store** structured so a database
backing can be added later without reshaping callers.

This is an **in-place rework (Path A)**: the stable layers — PTY, config, slash registry,
input router, chrome, shell OSC 133/7 hooks — carry over; the output→grid feed, transcript
render, block model, and event loop are reworked. The 003 spike's `spike-support` helpers
(coords/modes/clipboard) and `selection.rs` state machine are promoted into `src/` as the
design seed. Realizes decisions **D25–D30**.

## Technical Context

**Language/Version**: Rust 1.96.0 (edition 2021; CI `@stable` resolves to 1.96.0 — local
toolchain now aligned).

**Primary Dependencies**:
- `wezterm-term` (git-pinned `rev = 577474d89ee61aef4a48145cdec82a638d874751`) + its
  `termwiz`/`wezterm-*` tree — the production grid engine (D27). `alacritty_terminal 0.26`
  is the named fallback, not added unless the wezterm dep proves untenable.
- `ratatui` (bump 0.29 → 0.30), `crossterm` (bump 0.28 → 0.29), `portable-pty`
  (bump 0.8 → 0.9) — align with the spike-proven versions.
- `base64 0.22` (OSC 52 framing), `arboard 3.6` (local clipboard fallback).
- Retained: `serde`, `toml`, `tracing`(+subscriber/appender), `anyhow`, `thiserror`,
  `directories`, `libc`. `vte 0.13` is retained only for the OSC 133/7 + alt-screen
  side-channel tap (the grid engine owns the main escape parse).

**Storage**: In-memory only. The **block store** retains each block's output text as the
canonical `/save`/`/filter` source (supersedes D29's reconstruct-from-grid lean), behind a
single `block.text()` / `block.text_with_command()` accessor so a future DB backing is a
drop-in (SC-010, FR-019/FR-020). No persistence ships in this MVP.

**Testing**: `cargo test` (unit + integration). TDD per Constitution III; pure helpers
(coords/modes/clipboard, block-store eviction, selection state machine) are unit-tested
first. PTY/grid/clipboard behavior that needs a live shell or TTY uses the documented
Constitution III integration-test exception (e.g. the existing `tests/shell_parity.rs`,
now fish-guarded). Interactive mouse/render *feel* is validated via a manual quickstart
(the spike pattern), recorded against the spec's success criteria.

**Target Platform**: Linux-first (D9); the chosen engine keeps the cross-platform door
open but cross-platform is not a goal of this MVP.

**Project Type**: Single Rust binary crate (TUI terminal app) — `kapollo`/`kap`.

**Performance Goals**: Smooth interactive feel — no visible flicker or dropped frames
under output flood; selection highlight tracks the drag in real time; a `\r` progress bar
renders as a single updating line (SC-001). No hard numeric throughput target; the grid
engine's damage tracking (`get_changed_stable_rows`) bounds redraw cost.

**Constraints**: Constitution VI (TUI integrity) — logs off-screen, panic boundary
restores the terminal, clean teardown of raw mode + mouse capture + alternate screen on
every exit path (FR-027, SC-009). Memory: the in-memory block store is bounded by existing
`Caps` (D14); roughly a second copy of scrollback-scale text, acceptable per the
deep-history analysis.

**Scale/Scope**: Reworks ~40–50% of the existing ~2,857 LOC (output→grid, transcript
render, session/block, app event loop per [02-rework-vs-rewrite.md](../planning/grid-pivot/02-rework-vs-rewrite.md) §2);
keeps the other ~50–60% (PTY, config, slash, input router, chrome, hooks) and its tests.

### Open clarifications

None blocking. All NEEDS-CLARIFICATION candidates were resolved by the grid-pivot planning
docs (D25–D30) and the 003 spike recommendation; resolutions are captured in
[research.md](research.md).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. Spec-Driven Development** | ✅ | This work is fully specced ([spec.md](spec.md)); `docs/specification.md` updated in the polish phase. |
| **II. Architecture First** | ⚠️→✅ | A foundational change: it **reverses D4** (no grid model). `docs/architecture.md` MUST be updated **before** implementation diverges, recording the grid backend (D25/D27), block-as-annotation (D29), and mouse routing (D28). Tracked as the first task of Phase 1 design and a polish-phase gate. |
| **III. Test-Driven Development** | ✅ | Pure helpers (coords/modes/clipboard, block-store eviction, selection FSM) are TDD'd first. Live-shell/grid/clipboard paths use the documented integration-test exception. No coverage decrease. |
| **IV. Code Standards Gate** | ✅ | `cargo fmt --check` + `cargo clippy --all-targets --all-features -- -D warnings` + `cargo test` clean on 1.96.0. The CI fish-install fix is already in. |
| **V. Documentation** | ✅ | README, `docs/architecture.md`, `docs/setup.md`, `docs/usage.md` (new mouse/selection/clipboard/scroll keybindings + config) updated as definition-of-done. |
| **VI. Quality & Observability (TUI)** | ✅ | The hardest gate for this feature. Mouse + clipboard + alt-screen handover must preserve no-flicker, no-lost-output, clean teardown, panic boundary, off-screen logs. Explicit success criteria SC-001/003/009 and FR-027 enforce it. |
| **VII. Simplicity & Intentional Design** | ✅ | No speculative abstraction beyond the **one** deliberate seam the user asked for: the block-store text accessor (FR-019/020) enabling a future DB backing. Persistence, multi-block range-select, image rendering, and OSC 8 UX are explicitly deferred. |

**Gate result: PASS** (with the Architecture-First action item: update
`docs/architecture.md` as the first design step, not a follow-up). No unjustified
violations; Complexity Tracking not required.

## Project Structure

### Documentation (this feature)

```text
specs/004-grid-rework/
├── plan.md              # This file
├── research.md          # Phase 0 output — decisions D25–D30 + version/dep resolutions
├── data-model.md        # Phase 1 output — Grid/Cell/Scrollback/Block/BlockStore/Selection
├── quickstart.md        # Phase 1 output — manual interactive validation script
├── contracts/           # Phase 1 output — internal interface contracts
│   ├── block-store.md   #   block store + text accessor (DB-ready seam)
│   ├── grid-render.md   #   grid → ratatui render + scrollback windowing
│   └── mouse-selection.md  # mouse routing, selection FSM, clipboard
├── checklists/
│   └── requirements.md  # spec quality checklist (already ✔)
└── tasks.md             # Phase 2 output (/speckit.tasks — NOT created here)
```

### Source Code (repository root)

Single Rust crate. **Reworked** modules marked 🔧, **new** marked ✨, **kept** unmarked
(per the reuse map in [02-rework-vs-rewrite.md](../planning/grid-pivot/02-rework-vs-rewrite.md) §2):

```text
src/
├── main.rs                  # kapollo binary entry
├── lib.rs
├── app.rs               🔧  # event loop grows mouse routing, selection, scrollback, grid feed
├── config.rs            🔧  # + mouse/selection/clipboard/scroll keys (keep existing keys)
├── error.rs                 # keep
├── logging.rs               # keep (off-screen sink)
├── grid/                ✨  # NEW: owns the emulated screen + scrollback (wezterm-term)
│   ├── mod.rs           ✨  #   Grid wrapper: advance_bytes, viewport, stable-row anchoring
│   └── render.rs        ✨  #   grid cells → ratatui styled spans; scrollback windowing
├── selection/           ✨  # NEW: promoted from spike selection.rs + coords
│   ├── mod.rs           ✨  #   SelectionController FSM (idle→dragging→active)
│   └── coords.rs        ✨  #   content↔screen coord math (from spike-support::coords)
├── clipboard.rs         ✨  # NEW: OSC 52 framing + arboard fallback (from spike-support)
├── input/               🔧  # keep router; grow key/mouse handling for selection/scroll
│   ├── mod.rs
│   └── router.rs
├── output/              🔧  # re-point at grid feed; keep OSC 133/7 + alt-screen side tap
│   ├── mod.rs           🔧
│   ├── parser.rs        🔧  # mostly replaced by grid engine; retain side-channel tap
│   └── sentinel.rs          # keep in spirit (boundary fallback)
├── session/             🔧  # block becomes annotation-over-grid + block store
│   ├── mod.rs           🔧  #   Transcript → grid + block index
│   ├── block.rs         🔧  #   Block: command + output + exit + row range; text accessor
│   └── store.rs         ✨  #   BlockStore: in-memory canonical text, bounded eviction, DB-ready seam
├── slash/                   # keep (model-agnostic)
│   ├── mod.rs
│   └── builtins.rs
├── pty/                     # keep (spawn, resize, signals, shell hooks)
│   ├── mod.rs
│   └── shell.rs
├── ui/                  🔧  # transcript render replaced; chrome largely kept
│   ├── mod.rs           🔧
│   ├── transcript.rs    🔧  # render grid + selection highlight (replaces append render)
│   ├── passthrough.rs   🔧  # folded into mouse/keyboard routing on alt-screen/inner-mouse
│   ├── input_pad.rs         # keep (layout tweaks only)
│   └── status.rs            # keep
└── bin/
    └── kap.rs               # keep

tests/
├── shell_parity.rs          # keep (fish-guarded)
├── block_store.rs       ✨  # eviction, text accessor, duration, begin/set_start_row/seal
├── block_store_seam.rs  ✨  # swappable BlockText stub (SC-010), store⊥grid eviction
├── selection_coords.rs  ✨  # coord math + selection FSM (ported spike tests)
├── selection_routing.rs ✨  # mouse routing table + clipboard framing
├── grid_render.rs       ✨  # grid → spans (CR/SGR/wide)
└── grid_render_scroll.rs ✨ # scrollback windowing, alt-screen switch, damage
```

**Structure Decision**: Single-crate in-place rework. `ringbuf.rs`'s `OutputBuffer` is
retained inside the block store as the bounded byte/text retainer (it already enforces
`Caps`), so the store is an evolution of today's `Block.output`, not a greenfield system.
The grid engine replaces the hand-rolled `output/parser.rs` escape application, but the
OSC 133/7 + alt-screen detection side-tap survives (block boundaries still come from shell
integration marks, not the grid).

## Complexity Tracking

No constitution violations require justification. The single intentional seam (block-store
text accessor for a future DB backing) is explicitly requested by the spec (FR-019/020,
SC-010) and is the minimum abstraction for that goal — it is not speculative.

## Phase 0 — Research

See [research.md](research.md). All technical unknowns resolved from the grid-pivot
planning decisions (D25–D30) and the 003 spike; the file consolidates: engine choice +
git-pin rationale, dependency version bumps, the block-store-vs-reconstruction decision
(superseding D29), clipboard strategy, mouse/alt-screen routing model, and the
content-stable row anchoring approach.

## Phase 1 — Design & Contracts

- [data-model.md](data-model.md): Grid, Cell, Scrollback, Block, BlockStore, Selection,
  Clipboard Target — fields, relationships, state transitions, validation rules.
- [contracts/](contracts/): three internal interface contracts (block-store, grid-render,
  mouse-selection) — the seams TDD targets first.
- [quickstart.md](quickstart.md): manual interactive validation script mapped to SC-001…010.
- **Architecture-First action**: `docs/architecture.md` is updated to record the grid
  backend, block-as-annotation, and mouse routing **before** implementation diverges
  (Constitution II).

## Notes / carry-ins

- **CI**: the 003 PR's red CI was a missing `/usr/bin/fish` on the runner; fixed on this
  branch (workflow installs fish; the parity test skips gracefully when a shell is absent).
  No outstanding CI debt; the 004 PR should run green.
- **Toolchain**: local stable aligned to 1.96.0 to match CI `@stable`, closing version skew.
