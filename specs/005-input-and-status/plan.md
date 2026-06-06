# Implementation Plan: Input Editing & Fixed Status Bar

**Branch**: `005-input-and-status` | **Date**: 2026-06-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/005-input-and-status/spec.md`

## Summary

Turn kapollo's input pad into a real shell-grade **line editor** and add a small
**fixed-format status bar** beneath it. On top of the 004 grid/selection/chrome
foundation, this feature adds word/line motion, keyboard selection, and the
`Ctrl+U`/`Ctrl+K`/`Ctrl+W` kill commands to the input pad; reworks bracketed
paste so a multi-line paste lands as **one editable buffer** that never
auto-submits; retargets scrollback keys (context-preserving page scroll,
line-granular `Shift+PageUp/Down`, `Shift+Home/End`) now that `Home`/`End` belong
to line editing; and introduces a fixed `mode | cwd<greedypad>| message | exit`
status bar with `/status` and `/keys` slash commands.

Every behavior ships as a **named action** with a hardcoded default binding, so
the sprint 006 keymap engine can bind default + alternate per action with no
behavioral rewrite. The status bar uses a **fixed layout** (the template language
is sprint 008), reserves a 4-char mode field for future modes (LAAT, sprint 007),
and the single-selection-across-pads rule disambiguates `Ctrl+C` and `Esc`.

This is an **additive, in-place change** (the 004 pattern): the stable layers —
PTY, grid, block store, selection model, config, slash registry, input router —
carry over; this feature grows the input pad (`src/input`), the event loop's key
handling and paste handling (`src/app.rs`), the transcript scroll API
(`src/session/mod.rs`), the status chrome (`src/ui/status.rs`), and the slash
registry (`src/slash`). Realizes the resolved pre-plan decisions for sprint 005.

## Technical Context

**Language/Version**: Rust 1.96.0 (edition 2021; CI `@stable` resolves to 1.96.0,
local toolchain aligned — carried over from 004).

**Primary Dependencies**: No new crates expected. Existing stack carries the work:
- `crossterm 0.29` — already the event source; this feature **enables bracketed
  paste** (`EnableBracketedPaste` on setup, `DisableBracketedPaste` on teardown)
  and handles `Event::Paste(String)` (the one terminal-setup change).
- `ratatui 0.30` — status-bar chrome + input-pad render (existing widgets).
- `wezterm-term` (git-pinned) — the grid; unchanged, read-only for scroll metrics.
- Retained: `serde`/`toml` (config), `tracing`, `anyhow`/`thiserror`, `arboard`/
  `base64` (clipboard, unchanged). `unicode-segmentation` **may** be added only if
  punctuation-aware word motion needs word segmentation beyond a hand-rolled
  char-class scanner — default is **no new dependency** (see research).

**Storage**: None new. Status state (enabled flag, current message, mode) lives in
`App`; scrollback context-lines is a config value. No persistence.

**Testing**: `cargo test` (unit + integration), TDD per Constitution III. The pure
logic — word/line motion, selection ranges, kill commands, paste-to-buffer
splitting, page-minus-context clamping, status-layout fitting/truncation, `Esc`/
`Esc Esc` and single-selection arbitration — is unit-tested first and is the bulk
of the work. Live-TTY behavior (bracketed paste round-trip, status render across
resize, the `<10`-row hide) uses the documented Constitution III integration/manual
exception, validated by a manual quickstart mapped to SC-001…009.

**Target Platform**: Linux-first; built directly on the 004 grid/chrome.

**Project Type**: Single Rust binary crate (TUI terminal app) — `kapollo`/`kap`.

**Performance Goals**: Interactive feel only — edits, paste, and scroll are
sub-frame; no measurable throughput target. The status bar adds one render line and
must not introduce flicker or reflow (Constitution VI).

**Constraints**: Constitution VI (TUI integrity) — the status bar must not wrap to a
second row or break the `mode`/`exit` fields on narrow widths (FR-024), must
auto-hide cleanly across the 10-row threshold on resize (FR-021), and the
bracketed-paste teardown must be added to the existing panic/exit restore path so
no terminal is left in bracketed-paste mode (alongside raw mode + mouse capture +
alt-screen from 004).

**Scale/Scope**: Additive. Touches ~6 modules (`input`, `app`, `session`, `ui/status`,
`ui/input_pad`, `slash`) plus config; the bulk is new pure logic in `src/input`
(motion/selection/kill) and new status-bar formatting, with small edits to the event
loop and scroll API. No module is rewritten.

### Open clarifications

None blocking. All sprint-005 decisions were resolved in pre-planning
([pre-plan-005-input-and-status.md](../planning/pre-plan-005-input-and-status.md))
and encoded in the spec with zero `[NEEDS CLARIFICATION]` markers. The two design
choices with options (word-segmentation dependency; whether the new status bar
replaces or complements the existing above-input status *rule*) are resolved in
[research.md](research.md).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. Spec-Driven Development** | ✅ | Fully specced ([spec.md](spec.md)); `docs/specification.md` updated in the polish phase. |
| **II. Architecture First** | ✅ | No architectural reversal (004 did the big one). Additive: an input-editing action layer, paste handling, a status-bar chrome element. `docs/architecture.md` gains an "input editing + status bar (005)" section in the polish phase; no change needed before implementation since the grid/selection/chrome architecture is unchanged. |
| **III. Test-Driven Development** | ✅ | The feature is mostly pure logic (motion/selection/kill, paste split, page-minus-context, status fit/truncate, Esc/selection arbitration) — TDD'd first. Live-TTY paste/render uses the documented integration/manual exception. No coverage decrease. |
| **IV. Code Standards Gate** | ✅ | `cargo fmt --check` + `cargo clippy --all-targets --all-features -- -D warnings` + `cargo test` clean on 1.96.0. |
| **V. Documentation** | ✅ | README (new keys + status bar), `docs/usage.md` (key table, `/status`, `/keys`, config), `docs/setup.md` (new config keys), `docs/architecture.md`, `docs/specification.md` updated as definition-of-done. |
| **VI. Quality & Observability (TUI)** | ✅ | The key gate here: status bar must not flicker/reflow/wrap, must hide cleanly under 10 rows, and bracketed-paste teardown must join the panic/exit restore path. SC-005/SC-007 and FR-021/FR-024 enforce it. |
| **VII. Simplicity & Intentional Design** | ✅ | No speculative abstraction. Actions are **named** (a thin enum/registry) only because the spec requires it for 006; no keymap engine, no template engine, no new modes are built. Default to no new dependency. |

**Gate result: PASS.** No unjustified violations; Complexity Tracking not required.
The one forward-looking structure — named actions — is explicitly required by the
spec (FR-008) and is the minimum needed; it is not speculative.

## Project Structure

### Documentation (this feature)

```text
specs/005-input-and-status/
├── plan.md              # This file
├── research.md          # Phase 0 output — decisions + dependency/style resolutions
├── data-model.md        # Phase 1 output — InputBuffer/Selection/Action/StatusBar/ScrollView
├── quickstart.md        # Phase 1 output — manual interactive validation script (SC-001…009)
├── contracts/           # Phase 1 output — internal interface contracts
│   ├── input-editing.md #   motion/selection/kill actions + named-action registry
│   ├── input-paste.md   #   bracketed paste enable/handle/teardown
│   ├── scrollback.md    #   page-minus-context, line scroll, top/bottom; key retarget
│   ├── status-bar.md    #   fixed layout, fit/truncate, <10-row hide, message lifetime
│   └── slash-commands.md#   /status, /keys, /help pointer
├── checklists/
│   └── requirements.md  # spec quality checklist (already ✔)
└── tasks.md             # Phase 2 output (/speckit.tasks — NOT created here)
```

### Source Code (repository root)

Single Rust crate. **Reworked/grown** modules marked 🔧, **new** marked ✨, **kept**
unmarked. Built on the 004 layout:

```text
src/
├── main.rs                  # keep — entry; terminal setup gains Enable/DisableBracketedPaste
├── lib.rs               🔧  # terminal setup/teardown: bracketed paste + restore path
├── app.rs               🔧  # event loop: handle Event::Paste; on_key grows the new
│                        #     hardcoded bindings; status message lifetime; single-selection
│                        #     arbitration; /status + /keys dispatch; mode/status state
├── config.rs            🔧  # + [status] (enabled) and scroll.context_lines (keep existing keys)
├── input/               🔧  # the line-editor core (most new logic lives here)
│   ├── mod.rs           🔧  #   InputPad gains: line-aware Home/End, word motion,
│   │                    #     char/word selection, Ctrl+U/K/W kills, multi-line current-line ops
│   ├── editing.rs       ✨  #   pure motion/word-boundary/kill helpers (unit-test seam)
│   ├── selection.rs     ✨  #   input-pad selection range (anchor+caret) over the buffer
│   └── router.rs            # keep (leader routing)
├── action/              ✨  # NEW: named-action registry (FR-008) — Action enum + default
│   └── mod.rs           ✨  #     binding table; the unit /keys lists and 006 will bind
├── session/             🔧  # transcript scroll API
│   └── mod.rs           🔧  #   page_up/down take context-lines; add scroll_line_up/down aliases
├── slash/               🔧  # registry grows /status, /keys
│   ├── mod.rs           🔧  #   SlashCommand::{Status, Keys}; dispatch
│   └── builtins.rs      🔧  #   help text gains /keys pointer; /keys + /status text
├── ui/                  🔧
│   ├── mod.rs           🔧  #   layout: reserve the status-bar row beneath the input pad
│   ├── status.rs        🔧  #   the fixed mode|cwd<pad>|message|exit bar (below input);
│   │                    #     fit/truncate; <10-row hide; reconcile with the existing rule
│   ├── input_pad.rs     🔧  #   render selection highlight in the input pad
│   └── transcript.rs        # keep (selection highlight already from 004)
└── ...                      # pty/, grid/, selection/, clipboard.rs, error.rs, logging.rs — keep

tests/
├── input_editing.rs     ✨  # word/line motion, char/word selection, Ctrl+U/K/W, multi-line current-line
├── input_paste.rs       ✨  # bracketed paste → one buffer, no auto-submit, caret at end, edge cases
├── input_selection.rs   ✨  # input-pad selection range + single-selection-across-pads arbitration
├── scrollback_context.rs✨  # page-minus-context (default 3), at-least-one-line clamp, line scroll, top/bottom
├── status_bar.rs        ✨  # fixed layout fit/truncate, greedy pad, 4-char mode, <10-row hide, message lifetime
├── slash_status_keys.rs ✨  # /status toggle, /keys listing, /help pointer
└── ...                      # existing suites (block_store, selection_*, grid_render, shell_parity) — keep green
```

**Structure Decision**: Single-crate, additive. The line-editor logic is split into
pure helpers under `src/input` (`editing.rs`, `selection.rs`) so motion/word/kill/
selection are unit-testable without a terminal — the largest TDD surface. A small new
`src/action` module holds the **named-action registry** (an `Action` enum + a default
binding table) so `/keys` has something concrete to list and sprint 006 has the seam to
bind against; it deliberately does **not** implement binding/config (that is 006). The
status bar grows the existing `ui/status.rs` rather than adding a parallel module, with
the existing above-input "status rule" reconciled per research.

## Complexity Tracking

No constitution violations require justification. The one forward-looking structure —
the named-action registry (`src/action`) — is mandated by FR-008 and is the minimum
shape (an enum + a static default-binding table) needed for `/keys` today and the 006
keymap engine later. It builds no binding engine, no config parsing, and no template
logic, honoring Principle VII.

## Phase 0 — Research

See [research.md](research.md). Resolves: the bracketed-paste enable/teardown approach
and the restore-path addition; the word-boundary model (punctuation-aware motion vs.
readline whitespace kill) and the no-new-dependency decision; how the new fixed status
bar reconciles with the existing above-input status *rule*; the greedy-pad fit/truncate
algorithm; the named-action registry shape (minimum viable for FR-008/`/keys`); and the
`Esc`/`Esc Esc` + single-selection arbitration state model.

## Phase 1 — Design & Contracts

- [data-model.md](data-model.md): InputBuffer (multi-line, current-line ops), Input
  Selection, Named Action + Key Map, Status Bar + Status Message, Scroll View State —
  fields, relationships, state transitions, validation rules.
- [contracts/](contracts/): five internal interface contracts (input-editing,
  input-paste, scrollback, status-bar, slash-commands) — the seams TDD targets first.
- [quickstart.md](quickstart.md): manual interactive validation script mapped to
  SC-001…009 (the live-TTY behaviors: paste round-trip, status render/resize/hide,
  key feel).
- **Agent context**: `.github/copilot-instructions.md` is repointed to this plan.

## Notes / carry-ins

- **Bracketed paste is currently OFF**: today every pasted line arrives as `Event::Key`
  and embedded `\n`s submit — exactly the hazard US2 fixes. Enabling bracketed paste
  (and the `Event::Paste` handler) is the corrective change; the teardown must join the
  004 restore path (raw mode + mouse capture + alt-screen) so no terminal is stranded.
- **Two "status" surfaces**: 004 left a horizontal status *rule above* the input
  (`── cwd [exit N] (dur)` + notice). The 005 spec adds a fixed status *bar below* the
  input. research.md picks the reconciliation (fold the rule's cwd/exit/notice into the
  new bar vs. keep both); the spec's layout is authoritative for the new bar.
- **`notice` ⇒ `message`**: the existing `App.notice` is the natural backing for the
  status bar's `message` field; its lifetime changes to "until next submit / double Esc"
  (FR-025/FR-026), replacing the implicit 004 behavior.
- **No CI debt** carried in; 004 shipped green on this toolchain.
