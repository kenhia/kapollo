# Implementation Plan: Configurable Keymap Engine

**Branch**: `006-keymap-engine` | **Date**: 2026-06-06 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/006-keymap-engine/spec.md`

## Summary

Turn kapollo's hardcoded key bindings into a **config-driven keymap engine**.
Sprint 005 shipped every behavior behind a named-action registry
([src/action/mod.rs](../../src/action/mod.rs)) precisely so this sprint can make
the bindings configurable without touching any action's behavior. This feature
adds a human-writable **key-string parser** (`Ctrl+Left`, `shift+pageup`,
`Alt+Enter`, the `Esc Esc` chord), a `[keymap]` config surface with a
**primary + optional alternate** binding per action (array form), **per-mode**
keymap sections, **warn + last-wins** conflict detection, **disable-by-clearing**,
default bindings for the two previously-unbound 004 copy variants, a live `/keys`
listing, and an on-demand `/reload-config` slash command (no file watching).

The out-of-the-box keymap stays **identical** to today: the default map is the
former `BINDINGS` table expressed as data, so a user who configures nothing sees
no change. This is **plumbing for existing behaviors** — no new editing actions,
no mouse-binding config.

The work is additive and in-place (the 004/005 pattern). The stable layers — PTY,
grid, block store, status bar, input router, slash registry — carry over. This
feature grows the action layer (`src/action`) into a keymap (default map + parser
+ effective-map resolution + conflict reporting), adds a `[keymap]` table to
`src/config.rs`, retains the resolved config path on `App` so `/reload-config`
can re-read it, replaces `App::on_key`'s static `action::resolve` call with a
lookup against the effective keymap, binds the copy variants, and adds the
`/reload-config` slash command. Realizes the resolved pre-plan decisions for
sprint 006.

## Technical Context

**Language/Version**: Rust 1.96.0 (edition 2021; CI `@stable` resolves to 1.96.0,
local toolchain aligned — carried over from 004/005).

**Primary Dependencies**: No new crates expected. Existing stack carries the work:
- `crossterm 0.29` — the event source; `KeyCode`/`KeyModifiers` are the parse
  target for key strings (the engine maps strings ↔ the existing `KeyChord`).
- `serde` + `toml` — the `[keymap]` table deserializes as a map of action name →
  binding (string or array of strings), per-mode sections nest under named tables.
  The existing `RawConfig`/`into_config`/`warn_unknown_keys` pattern is extended.
- `ratatui 0.30`, `wezterm-term` (git-pinned) — unchanged; the keymap does not
  touch rendering or the grid.
- Retained: `tracing` (the warn/conflict diagnostics sink), `anyhow`/`thiserror`
  (config errors). No `unicode`/parser crate is needed — the key-string grammar
  is a small hand-rolled tokenizer (`+`-split modifiers + a key-name table).

**Storage**: None new. The effective keymap lives in `App` (replacing the static
`BINDINGS` lookup). `/reload-config` re-reads the same config path the run was
started with, so `App` must **retain that path** (it currently does not — see
research R6). No persistence.

**Testing**: `cargo test` (unit + integration), TDD per Constitution III. The
feature is **almost entirely pure logic** and is unit-tested first: key-string
parsing (case-insensitive, modifier-order-tolerant, `Esc Esc` chord), the
default-map → effective-map overlay, primary/alternate resolution,
disable-by-clearing, per-mode override + inheritance, and conflict detection
(warn + last-wins). The `[keymap]` config parsing + diagnostics are unit-tested
against in-memory TOML (the existing `from_toml` seam). Only the live-TTY
round-trip (a rebound key actually firing, `/reload-config` applying without
disrupting an in-progress buffer) uses the documented Constitution III
integration/manual exception, validated by a manual quickstart mapped to
SC-001…006.

**Target Platform**: Linux-first; built directly on the 004/005 chrome.

**Project Type**: Single Rust binary crate (TUI terminal app) — `kapollo`/`kap`.

**Performance Goals**: Interactive feel only. Key resolution is a small map lookup
per keystroke; building the effective keymap happens once at startup and on
`/reload-config`. No throughput target.

**Constraints**: Constitution VII (simplicity) is the governing gate — the engine
must be the minimum that satisfies the spec, not a speculative framework. Per-mode
support is built structurally (the default mode is the only populated map this
sprint) only because the spec (FR-012) and sprint 008 require it. Constitution VI:
a malformed `/reload-config` must never leave the session with a broken keymap
(FR-016) and must not discard an in-progress input buffer (FR-017).

**Scale/Scope**: Additive. Touches `src/action` (grows into the keymap engine),
`src/config.rs` (`[keymap]` table + Raw structs + warn loop), `src/app.rs`
(effective-keymap field, retained config path, `on_key` lookup, `/reload-config`
+ live `/keys`), `src/slash` (`ReloadConfig` command), and `src/lib.rs` (thread
the resolved config path into `App`). The bulk is new pure logic in `src/action`
(parser + keymap model) and config parsing; no module is rewritten.

### Open clarifications

None blocking. All four pre-plan open questions were resolved by the user before
specification ([pre-plan-006-keymap-engine.md](../planning/pre-plan-006-keymap-engine.md))
and encoded in the spec with zero `[NEEDS CLARIFICATION]` markers:
per-mode keymaps; alternates as an array `["Shift+Enter", "Alt+Enter"]`;
warn + last-wins conflicts; canonical short modifier names, case-insensitive;
`Esc Esc`-only chord; on-demand `/reload-config` (no file watcher). The design
choices with options (config-path retention strategy; how `Esc Esc` is modeled
alongside single-key chords) are resolved in [research.md](research.md).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. Spec-Driven Development** | ✅ | Fully specced ([spec.md](spec.md)); `docs/specification.md` updated in the polish phase. |
| **II. Architecture First** | ✅ | No architectural reversal. The named-action registry from 005 was designed as the seam for exactly this; the engine grows it into a keymap. `docs/architecture.md` gains a "keymap engine (006)" section in the polish phase; no pre-implementation architecture change is needed. |
| **III. Test-Driven Development** | ✅ | The feature is overwhelmingly pure logic (parser, default→effective overlay, primary/alternate, disable-by-clearing, per-mode inheritance, conflict detection) — TDD'd first. Live-TTY rebinding/reload uses the documented integration/manual exception. No coverage decrease. |
| **IV. Code Standards Gate** | ✅ | `cargo fmt --check` + `cargo clippy --all-targets --all-features -- -D warnings` + `cargo test` clean on 1.96.0. |
| **V. Documentation** | ✅ | README (`[keymap]` config + `/reload-config`), `docs/usage.md` (binding syntax, alternates, disable-by-clearing, per-mode, `/keys`/`/reload-config`), `docs/setup.md` (new config keys), `docs/architecture.md`, `docs/specification.md` updated as definition-of-done. A generated, **fully-populated** example keymap (`docs/keymap-defaults.toml`) ships every action's default binding, kept honest by a sync test (see Structure). |
| **VI. Quality & Observability (TUI)** | ✅ | A malformed `/reload-config` reports the error and keeps the prior keymap (FR-016); reload never discards an in-progress buffer (FR-017); invalid/conflicting bindings warn via `tracing` and never block startup (FR-009/FR-010). |
| **VII. Simplicity & Intentional Design** | ✅ | The governing gate. No speculative framework: per-mode is built minimally because FR-012 requires it; the parser is a hand-rolled tokenizer (no new dep); chords are limited to `Esc Esc` per the spec. No template engine, no file watcher, no mouse-binding config. |

**Gate result: PASS.** No unjustified violations; Complexity Tracking not required.
The one forward-looking structure — per-mode keymap sections — is explicitly
required by the spec (FR-012) for sprint 008 and is built as the minimum (default
mode is the only populated map this sprint).

## Project Structure

### Documentation (this feature)

```text
specs/006-keymap-engine/
├── plan.md              # This file
├── research.md          # Phase 0 output — decisions (parser grammar, path retention, conflict policy)
├── data-model.md        # Phase 1 output — Action/KeyChord/KeySpec/Binding/Keymap/Keymaps
├── quickstart.md        # Phase 1 output — manual interactive validation script (SC-001…006)
├── contracts/           # Phase 1 output — internal interface contracts
│   ├── key-string.md    #   grammar + parse/format rules (case-insensitive, Esc Esc chord)
│   ├── keymap-config.md #   [keymap] table shape, per-mode sections, alternate array, clearing
│   ├── keymap-engine.md #   default map, effective-map overlay, resolve(), conflict reporting
│   └── slash-commands.md#   /reload-config + live /keys listing
└── checklists/
    └── requirements.md  # Spec quality checklist (already complete)
```

### Source Code (repository root)

```text
src/
├── action/
│   └── mod.rs           # 🔧 grows: Action set (+ copy variants, + insert_newline action),
│                        #    KeyChord (kept), NEW KeySpec/key-string parser, NEW Keymap +
│                        #    Keymaps (default map as data; effective-map overlay; resolve;
│                        #    conflict detection). The static BINDINGS table becomes the
│                        #    default-map constructor.
├── config.rs            # 🔧 [keymap] table: RawKeymap (map of mode → action → binding),
│                        #    into_config builds Keymaps; warn loop + diagnostics.
├── app.rs               # 🔧 holds effective Keymaps + retained config path; on_key resolves
│                        #    against the effective keymap; /reload-config re-reads + re-applies;
│                        #    /keys lists the live map; binds copy variants.
├── slash/
│   ├── mod.rs           # 🔧 SlashCommand::ReloadConfig + dispatch("reload-config").
│   └── builtins.rs      # 🔧 help_text lists /reload-config.
└── lib.rs               # 🔧 thread the resolved config path into App::new.

docs/
└── keymap-defaults.toml # ✨ generated example: a [keymap] table fully populated with EVERY
                         #    action's default binding (commented, copy-paste-ready). Kept in
                         #    sync with Keymap::default_map() by tests/keymap_defaults_doc.rs.

tests/
├── keymap_parse.rs      # ✨ key-string grammar: case-insensitive, modifier order, Esc Esc,
│                        #    bad strings rejected, round-trip format.
├── keymap_config.rs     # ✨ [keymap] parsing: primary/alternate array, per-mode override +
│                        #    inheritance, disable-by-clearing, unknown action/mode warned.
├── keymap_engine.rs     # ✨ default == legacy defaults; effective overlay; conflict last-wins;
│                        #    copy variants bound; resolve() matches.
├── keymap_defaults_doc.rs # ✨ loads docs/keymap-defaults.toml and asserts it builds a Keymap
│                        #    equal to Keymap::default_map() — the example can never drift.
└── config.rs            # 🔧 +keymap default + unknown-key cases (extends existing suite).
```

**Structure Decision**: Single Rust binary crate (unchanged). The keymap engine
lives in the existing `src/action` module — the registry it extends — rather than
a new top-level module, keeping the action set and its bindings in one place. The
parser, default map, effective-map overlay, and conflict detection are all pure
functions/types in `src/action`; config parsing stays in `src/config.rs`
consistent with how `[status]`/`[divider]`/`[scroll]` were added; `App` holds the
resolved `Keymaps` and the config path. New test files mirror the three concern
boundaries (parse / config / engine). A generated example
(`docs/keymap-defaults.toml`) documents the full default map and is kept in sync
with `Keymap::default_map()` by a dedicated test.

## Complexity Tracking

> No constitution violations require justification. Complexity Tracking not
> required for this feature.
