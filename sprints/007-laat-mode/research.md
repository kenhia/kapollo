# Research: LAAT Mode, `/save`, `/filter`, and `/load`

**Feature**: 007-laat-mode | **Date**: 2026-06-07 | **Phase**: 0

All ten pre-plan open questions were resolved by the user before specification
([pre-plan-007-laat-mode.md](../planning/pre-plan-007-laat-mode.md)) and encoded
in [spec.md](spec.md) with zero `[NEEDS CLARIFICATION]` markers. This document
captures the **design decisions with options** — the places where the spec is
decisive about *what* but the implementation still has to choose *how*, given the
shipped 004/005/006 code. Each entry follows Decision / Rationale / Alternatives.

## R1 — Where the input mode lives

**Decision**: Add an `InputMode { Norm, Mult, Laat }` enum and store the current
mode as a field on `App` (e.g. `App.mode: InputMode`), not on `InputPad`. The mode
is surfaced to the status bar by passing it into `crate::ui::status::render`
instead of the hardcoded `DEFAULT_MODE` literal, and to the keymap-resolution and
Up/Down handling in `App::on_key`.

**Rationale**: The mode is a property of the *session's interaction state*, not of
the editable text buffer. `InputPad` today is a pure text+cursor+selection model
with no app coupling; keeping it that way preserves its unit-testability. `App`
already owns the other cross-cutting interaction state (`esc_pending`,
`passthrough`, `selection`), so the mode belongs alongside them. The status bar
already reserves a 4-column mode field (005); only `status::render`'s hardcoded
`DEFAULT_MODE` needs to become `app.mode.label()`.

**Alternatives considered**:
- *Mode on `InputPad`*: would entangle the pure buffer model with app-level
  concepts (history recall, LAAT stepping) and force `InputPad` tests to reason
  about modes. Rejected for Constitution VII (keep the buffer simple).
- *Derive mode implicitly from `line_count()`*: a single/empty buffer is `norm`,
  multi-line is `Mult`. Rejected because `Ctrl+Alt+1` must enter `Mult` from an
  empty buffer (FR-015) and LAAT is not distinguishable from `Mult` by line count
  (FR-016) — the mode must be explicit state.

## R2 — Mode label rendering (`Mult` / `1T` / `norm`)

**Decision**: `InputMode::label() -> &'static str` returns `"norm"`, `"Mult"`,
`"1T"` for the status bar's 4-column field (the existing `fit_mode` pads/truncates
to four columns, so `"1T"` renders left-justified in the field). The full name
`LaaT` is used only in docs and prose, never in the 4-column field. The existing
`status::DEFAULT_MODE` constant stays as the `Norm` label source.

**Rationale**: The pre-plan fixed `LaaT` (full) / `1T` (short, 2 chars) and `Mult`
as the labels; the 4-column field can hold `Mult` exactly and `1T` with padding.
No status-bar layout change is needed — only the mode string becomes dynamic.

**Alternatives considered**: `LaaT` (4 chars) in the field instead of `1T`.
Rejected: the pre-plan explicitly chose `1T` as the status field's short form.

## R3 — Extending `InputHistory` for the stashed draft (chat-style edge recall)

**Decision**: Extend `InputHistory` to own the **stashed draft**. Add a
`stash: Option<String>` field. When recall begins at an edge (first `recall_older`
from the live draft), the caller passes the current buffer in so it is stashed;
`recall_newer` past the newest entry returns the stashed draft (not `""`) and
clears the recall cursor. Concretely, change the recall entry points to
`begin_recall_older(&mut self, draft: &str) -> Option<&str>` (stashes `draft` on
the first step) and have `recall_newer` return the stash when stepping past the
newest entry. The stash persists until it is consumed by returning to the draft
*and edited*, and survives a push/pop round-trip because it travels inside the
saved `InputHistory`-adjacent snapshot (see R7).

**Rationale**: Today `recall_older`/`recall_newer` return only stored entry text,
and `App::on_key` calls `set_contents(text)` on Up/Down — which discards an
in-progress multi-line draft (the sprint-005 sharp edge, SC-002). The history type
already tracks a recall cursor (`None` = live draft); holding the stashed draft
there keeps all recall state in one place and makes `Down` restoring the draft a
pure-logic, unit-testable operation.

**Alternatives considered**:
- *Stash on `App` instead of `InputHistory`*: splits recall state across two types
  and complicates the push/pop snapshot. Rejected — recall is `InputHistory`'s job.
- *Re-derive the draft from the buffer on `Down`*: impossible once `Up` already
  overwrote the buffer via `set_contents`. Rejected.

## R4 — Mode-aware `Up`/`Down`

**Decision**: In `App::on_key`, branch `Up`/`Down` on `self.mode`:
- **`Norm`**: unchanged — recall history (`recall_older`/`recall_newer`) as today.
- **`Mult` / `Laat`**: move the caret between buffer lines via new
  `InputPad::caret_line_up` / `caret_line_down` (column-preserving, clamped). When
  the caret is already on the **first** line, `Up` performs chat-style edge recall
  (stash draft + recall older, R3); when on the **last** line, `Down` past the end
  restores the stash. In `Laat`, the same keys also move the **highlight** in
  lockstep with the caret (the highlight tracks the caret's line, R5).

**Rationale**: The pre-plan states LAAT keys == `Mult` keys ("LaaT = Mult +
highlight + step + gating"), so caret motion is shared and only the highlight and
the `Enter` semantics differ. Keeping the mode branch in `on_key` (where Up/Down
are already special-cased before keymap resolution) is the smallest change.

**Alternatives considered**: Make Up/Down named keymap actions resolved per-mode.
Rejected this sprint — per-mode keymap config sections are explicitly out of scope
(pre-plan), and Up/Down are already handled specially in `on_key` ahead of the
keymap; promoting them now would expand scope without benefit.

## R5 — LAAT highlight model and exit-code gating hook

**Decision**: Model LAAT as a small state struct (e.g. `LaatState`) owned by `App`
while `self.mode == Laat`, holding the **highlighted line index**, a per-line
**probable-failure flag set**, and a **pending-submission** marker recording which
line was last submitted and is awaiting completion. The highlight tracks the caret
line during navigation. On `Enter` in LAAT, submit the highlighted line as a normal
single-line submission and set the pending marker. Hook command completion where
`Boundary::CommandEnd { exit_code }` is already observed (the `drain_shell` /
`after_drain` path that updates `App.last_exit`): if a LAAT submission is pending,
exit `0` advances the highlight to the next line and clears the pending marker;
non-zero leaves the highlight in place, sets the probable-failure flag for that
line, and clears the pending marker. The input-pad renderer (`ui::input_pad`)
draws the highlight (a background style on the highlighted line) and the
probable-failure flag (a distinct background) when `self.mode == Laat`.

**Rationale**: Exit codes already arrive through the boundary side-tap that sets
`last_exit`; reusing that single completion hook keeps the gating deterministic and
avoids a second exit-tracking path (Constitution VI/VII). A submitted LAAT line is
"a normal single-line submission" (FR-005), so it flows through the existing
`submit` → `run_shell` path unchanged; only the *post-completion* reaction is new.

**Alternatives considered**:
- *Poll the block store for the last block's exit code each frame*: redundant with
  the existing `CommandEnd` observation and racy. Rejected.
- *Run the whole buffer and gate between lines internally*: contradicts the
  interactive, user-paced stepping model (each `Enter` advances one line). Rejected.

## R6 — Slash commands with arguments (`/save <file>`, `/filter <cmd>`, `/load <file>`)

**Decision**: Extend the slash layer to carry arguments. Today `slash::dispatch`
trims to a single name token and every `SlashCommand` variant is argument-less. Add
argument-bearing variants `Save(String)`, `Filter(String)`, `Load(String)` whose
payload is the remainder of the command line after the verb (the raw argument
string, not split — so `/filter rg foo | sort` keeps its pipes for the shell). The
verb is matched case-sensitively against `save` / `filter` / `load`; the rest of
the line (trimmed of the leading space) is the payload. An empty payload is still
dispatched (so `/save` with no path can produce the `'/save' requires path` status
per FR-022) rather than falling through to the unknown-command path.

**Rationale**: Keeps argument parsing in one place (`slash::dispatch`) consistent
with how the existing verbs are matched, and preserves the raw argument so the
shell sees pipes/globs verbatim for `/filter` (FR-025).

**Alternatives considered**: Parse arguments in `App::run_slash` after dispatch
returns the name. Rejected — splits command parsing across two modules and would
require `dispatch` to signal "known verb, has args", which is exactly what the new
variants encode.

## R7 — Push/pop one-item input stack

**Decision**: Model the stack as a single `Option<InputSnapshot>` on `App` where
`InputSnapshot { buffer: String, cursor, mode: InputMode, stash: Option<String>,
history_cursor }` captures the buffer, caret, mode, and the recall/stash state.
`PushInput` (default `Ctrl+Alt+Enter`) saves the snapshot, resets the pad to empty,
sets `mode = Norm`, and leaves any existing snapshot **replaced only if empty**
(one-item semantics: a push while already pushed does not nest — the second push is
a no-op or overwrites per FR-020; choose **no-op** so the first saved state is never
lost). The pop happens on the **next** `submit`: after routing/running the ad-hoc
command, if a snapshot is present, restore it (buffer, caret, mode, stash) and clear
the slot.

**Rationale**: A one-item `Option` is the minimum that satisfies FR-018/019/020
(Constitution VII). Restoring on the next submit matches the pre-plan ("switch to
normal mode until the next submit, then pop"). Capturing the stash inside the
snapshot satisfies FR-011 (stash survives the push/pop round-trip, SC-003/SC-006).

**Alternatives considered**:
- *A `Vec` stack*: the spec mandates a single-item stack (FR-020); a vector invites
  scope creep. Rejected.
- *Overwrite on second push*: would silently drop the user's saved state, violating
  SC-006 ("never silently dropping their saved state"). Rejected in favor of no-op.

## R8 — `/filter` execution model (shell pipe + chaining)

**Decision**: Implement `/filter <cmd>` by materializing the previous block's exact
stored output to a temporary file and submitting `cat <tempfile> | <cmd>` to the
wrapped shell as a normal command, with the **transcript block titled**
`/filter <cmd>` (the synthetic command label) rather than the literal `cat ... |`.
Because the result is a normal block captured by the existing block store, it
automatically **becomes the new previous output**, so a subsequent `/filter`
operates on it (chaining, FR-026). A non-zero exit is surfaced by the existing
`last_exit` status field plus a `filter non-zero exit` notice (FR-027). When there
is no previous block, show the same not-found notice as `/save` (edge case).

**Rationale**: Routing through the shell gives pipes/globs/aliases for free
(FR-025) and reuses the entire capture/transcript/exit-code pipeline, so chaining
and exit reporting need no new machinery. The temp file is the pre-plan's stated
model ("save previous output to a temp, `cat <temp> | <cmd>`").

**Alternatives considered**:
- *Pipe bytes directly into a spawned `<cmd>` outside the PTY*: would bypass the
  transcript/exit-code capture and require a parallel output path. Rejected.
- *Heredoc / process substitution*: shell-specific and fragile across the supported
  shells; a temp file is portable. Rejected.

**Temp-file hygiene**: write to a uniquely-named file under the system temp dir,
and remove it after the command completes (best-effort). This is a system-boundary
filesystem op (Constitution VII) and is the one place the feature touches temp I/O.

## R9 — `/save` overwrite prompt: a new modal input primitive

**Decision**: Add a minimal **pending-prompt** state to `App` (e.g.
`pending_prompt: Option<PendingPrompt>`). When `/save <file>` targets an existing
file, set the prompt and show `File exists, [O]verwrite, [A]ppend, [C]ancel?` on
the status line; `App::on_key` checks `pending_prompt` **first** and consumes the
next key: `o/O` overwrites, `a/A` appends, `c/C` or `Esc` cancels (any other key is
ignored, keeping the prompt up). The prompt carries the resolved target path and
the bytes to write so the action completes without re-deriving state. `/save`
resolves the path relative to `App.cwd` with `~` expansion, writes the previous
block's exact stored output, and on missing/evicted previous block shows
`Save failed, previous buffer not found` (FR-024). No-path `/save` shows
`'/save' requires path` and does **not** clear the buffer (FR-022).

**Rationale**: A one-key confirmation is the smallest interaction that satisfies
FR-023; gating it at the top of `on_key` mirrors how `esc_pending` and
`passthrough` already short-circuit key handling, so it composes with the existing
event loop without a separate modal render layer (Constitution VI/VII).

**Alternatives considered**:
- *A full modal overlay widget*: over-engineered for a 3-way confirmation
  (Constitution VII). Rejected.
- *Auto-overwrite or auto-append*: unsafe / surprising; the spec mandates an
  explicit prompt (FR-023). Rejected.

## R10 — Registering the new bindings in the 006 keymap engine

**Decision**: Add two named, rebindable `Action` variants —
`ToggleMultLaat` (default `Ctrl+Alt+1`) and `PushInput` (default `Ctrl+Alt+Enter`)
— to `src/action/mod.rs`, wire them into `Action::name`/`from_name`, the
`default_map`, and `dispatch_action`, and ship their defaults in
`docs/keymap-defaults.toml` (kept honest by the existing `keymap_defaults_doc`
sync test). The 006 key-string parser already accepts both (`1` → `Char('1')`,
`enter` → `Enter`, with `Ctrl+`/`Alt+` modifier prefixes), so **no parser change is
needed**. `ToggleMultLaat` enters `Mult` from `Norm` and toggles `Mult ↔ Laat`
when multi-line; `PushInput` performs the push (R7). Both are listed by `/keys`.

**Rationale**: 006 was built precisely so new behaviors are added as named actions
with config-driven bindings (no hardcoded keys). Verifying the parser already
handles `Ctrl+Alt+1` / `Ctrl+Alt+Enter` (confirmed in `parse_key`) means the only
work is registering the actions and their default bindings.

**Alternatives considered**: Hardcode the two keys in `on_key` like 005's pre-006
bindings. Rejected — it would regress the 006 design and FR-029 (rebindable, listed
by `/keys`). Per-mode `[keymap.laat]`/`[keymap.mult]` config tables remain out of
scope (pre-plan); the two actions are global.

## Resolved by pre-plan (no further research)

These were decided by the user before specification and are recorded here for
traceability — they are not open:

- Execution **gates on completion**; exit `0` advances, non-zero flags a
  **probable** failure (deliberate wording for non-zero-success commands).
- Mode labels `LaaT` / `1T` / `Mult`.
- `Ctrl+Alt+1` enters `Mult` from `Norm` (even empty/single line); toggles
  `Mult ↔ Laat` once multi-line. `Alt+Enter` and `/load` are the other entries.
- Selection and submission stay **separate** in all modes; selecting lines then
  `Enter` submits one combined submission (FR-017).
- `/save` and `/load` paths resolve relative to kapollo's cwd (follows `cd`) with
  `~` expansion; `/filter` runs via the shell and chains.
- Stash survives until popped (FR-011); leaving LAAT clears the LAAT buffer
  (FR-007); deleting `Mult` back to one line returns to `Norm` (FR-012).
- Per-mode keymap config sections and cross-session LAAT persistence are out of
  scope.
