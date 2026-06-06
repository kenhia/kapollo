# Phase 0 Research: Input Editing & Fixed Status Bar

**Feature**: 005-input-and-status | **Date**: 2026-06-05

All sprint-005 product decisions were settled in pre-planning
([pre-plan-005-input-and-status.md](../planning/pre-plan-005-input-and-status.md))
and encoded into [spec.md](spec.md) with zero `[NEEDS CLARIFICATION]` markers. This
document resolves the **implementation-level** unknowns the plan flagged: paste
mechanics, word-boundary models, the status-surface reconciliation, the greedy-pad
fit/truncate algorithm, the named-action registry shape, and the Esc/selection state
machine. No external research was required; each item is a decision grounded in the
existing codebase.

---

## R1 — Bracketed paste: enable, handle, and tear down safely

**Decision**: Enable bracketed paste at terminal setup
(`crossterm::event::EnableBracketedPaste`), handle the resulting
`Event::Paste(String)` in the `app.rs` event loop by inserting the string into the
input pad as one buffer, and add `DisableBracketedPaste` to **every** restore path
(normal exit and the panic guard), alongside the existing raw-mode / mouse-capture /
alt-screen teardown from 004.

**Rationale**: Bracketed paste is currently **off**. Today a multi-line paste arrives
as a stream of `Event::Key` presses, and the embedded `\n`s hit the `(Enter, _) =>
submit` arm — so each pasted line submits as a separate command. That is precisely the
hazard US2 (FR-010/FR-011) eliminates. With bracketed paste enabled, crossterm
coalesces the paste into a single `Event::Paste(String)` that bypasses the key path
entirely, so no embedded newline can reach the submit arm. The caret is then placed at
the end of the inserted content (FR-012) and the buffer stays fully editable.

The teardown is a Constitution VI obligation: leaving a terminal in bracketed-paste
mode after a crash would corrupt the user's shell. The 004 panic guard already restores
raw mode, mouse capture, and the alt-screen; `DisableBracketedPaste` joins that exact
list so there is a single, symmetric setup/teardown pair.

**Alternatives considered**:
- *Keep bracketed paste off and strip newlines from key-stream pastes* — impossible to
  distinguish a fast paste from real typing reliably; brittle and timing-dependent.
- *Heuristic "paste detected" via inter-key timing* — rejected; bracketed paste is the
  purpose-built terminal mechanism and crossterm exposes it directly.

**Implementation note**: `Event::Paste` carries the raw string including `\n`s;
`InputPad` inserts it verbatim, splitting on `\n` into buffer line boundaries (FR-010).
`\r\n` and lone `\r` are normalized to `\n` on the way in.

---

## R2 — Word boundaries: punctuation-aware motion vs. readline whitespace kill

**Decision**: Two distinct, hand-rolled char-class scanners — **no new dependency**:
- `Ctrl+Left/Right` word **motion** (FR-002) is **punctuation-aware**: it classifies
  each char as whitespace / word (alphanumeric + `_`) / punctuation, and a boundary
  sits between any two adjacent runs of different class. This matches editor-style word
  navigation.
- `Ctrl+W` **delete-word-before** (FR-006) uses the **readline whitespace rule**:
  consume any immediately preceding whitespace, then the preceding non-whitespace run
  (punctuation included). This matches shell muscle memory.

**Rationale**: The spec deliberately specifies *different* semantics for the two (Q2 in
the pre-plan), so they cannot share one scanner. Both are simple character-class
state machines over the **current line's** char sequence; ASCII/Unicode scalar
classification via `char::is_whitespace` / `char::is_alphanumeric` is sufficient for
shell command lines. A grapheme-cluster library (`unicode-segmentation`) would add a
dependency and complexity for combining-character correctness that command-line editing
does not require here. If a real-world need for grapheme-aware motion appears later, the
helper is the single seam to swap — but the default, per Principle VII, is **no new
crate**.

**Alternatives considered**:
- *`unicode-segmentation` word boundaries (UAX #29)* — rejected: heavier than needed,
  and UAX #29 word rules do not match either the editor or readline behaviors the spec
  names, so it would need overriding anyway.
- *One shared scanner for both* — rejected: the spec requires divergent semantics.

---

## R3 — Reconciling the new status **bar** with the existing status **rule**

**Decision**: **Fold** the existing above-input status *rule* into the new fixed status
*bar*. The 004 rule (`── cwd [exit N] (dur)` + transient notice, rendered **above** the
input pad in `ui/status.rs`) is replaced by the single fixed-layout bar **below** the
input pad: `mode | cwd<greedypad>| message | exit`. The cwd and exit code move into
their named fields; the existing `App.notice` becomes the `message` field; the sealed-
block duration is dropped from the chrome (it is not part of the spec's fixed layout and
can return via the sprint-008 template language if wanted).

**Rationale**: Two stacked status surfaces showing overlapping data (cwd, exit, notice)
would be redundant and waste a row. The spec's layout is authoritative and explicitly
positions the bar **beneath** the input pad (FR-018), so the above-input rule has no
remaining job. Folding keeps a single source of truth for cwd/exit/message, avoids
double-rendering, and means the `<10`-row auto-hide (FR-021) governs one element, not
two. `ui/status.rs` is **grown** (not duplicated) into the new bar so the render path
stays in one module.

**Alternatives considered**:
- *Keep both (rule above + bar below)* — rejected: redundant data, extra row, two hide
  rules, more flicker surface.
- *New `ui/status_bar.rs` parallel module* — rejected: splits status rendering across
  two files for no benefit; grow the existing module instead.

**Open follow-on**: the sealed-block duration `(1.2s)` indicator is intentionally
dropped from chrome this sprint; if missed, it returns naturally in the 008 status
template. Noted so the 004 behavior change is explicit, not accidental.

---

## R4 — Greedy-pad fit & truncation algorithm

**Decision**: Compose the bar left-to-right into a fixed width `W`:
1. Reserve the **4-char mode field** + its `" | "` separator (always present; FR-020).
2. Reserve the **exit field** (`" | <exit>"`) on the right when an exit code exists;
   it is laid out from the right edge and never broken (FR-024).
3. The middle region (between mode and exit) holds `cwd<greedypad>| message` with a
   greedy pad between `cwd` and the `|` (**no `|` *immediately* after cwd**; FR-019) and
   `message` **right-justified** against the exit field.
4. **Truncation order under width pressure** (FR-024): never break `mode` or `exit`
   first; truncate `message` (with a `…` ellipsis) before `cwd`; if still over, truncate
   `cwd` (ellipsizing the **left/middle** of the path so the trailing component stays
   visible). The greedy pad is whatever non-negative space remains; if zero, fields abut
   without wrapping.

**Rationale**: FR-024 forbids wrapping to a second row and forbids breaking `mode`/
`exit`. A fixed-precedence, single-pass layout (reserve fixed fields → fill middle →
clamp) is deterministic, unit-testable on a width parameter without a terminal, and has
no flicker risk (it always produces exactly one line of exactly `W` columns). Truncating
`message` before `cwd` keeps the user's location visible (more durably useful than a
transient notice); ellipsizing the cwd from the middle preserves the most-specific
trailing directory.

**Alternatives considered**:
- *Proportional/ratatui `Constraint::Ratio` layout* — rejected: harder to guarantee the
  "never break mode/exit, never wrap" invariant and harder to unit-test precisely.
- *Truncate cwd before message* — rejected: location is the more useful persistent
  signal; the message is transient and can be re-shown.

**Unit-test seam**: `status::fit(width, mode, cwd, message, exit) -> String` (pure,
width-parameterized) is the TDD target; the ratatui render just paints the fitted line.

---

## R5 — Named-action registry: minimum shape for FR-008 / `/keys`

**Decision**: A new `src/action` module with an `Action` enum (one variant per named
action in FR-001–FR-006, FR-013–FR-016, FR-026 — e.g. `LineMoveStart`, `WordMoveLeft`,
`SelectCharLeft`, `KillToLineStart`, `DeleteWordBefore`, `ScrollPageUp`,
`ScrollToTop`, `ClearStatusMessage`, …) plus the two **reserved, unmapped** names
`MultilineMoveStartBuffer` / `MultilineMoveEndBuffer` (FR-009). A `const`/static table
maps each mapped `Action` to its **single hardcoded default** key chord (a
`(KeyCode, KeyModifiers)`-style descriptor) and a stable string name. `/keys` renders
this table; `app.rs::on_key` resolves a chord to an `Action` and dispatches.

**Rationale**: FR-008 requires every behavior to be a *named action* "structured so the
future keymap engine can bind a default and an alternate per action without changing the
action's behavior." The minimum structure that satisfies this is: (a) a stable name per
action, (b) a single lookup table from chord → action that `on_key` consults, and (c) a
listing surface (`/keys`). Crucially this sprint builds **no** binding engine, **no**
config parsing of keys, and **no** alternate slot — those are sprint 006. The registry
is the seam 006 will extend; today it is just an enum + a static default table. The
reserved unmapped names exist in the enum with no table entry (FR-009).

**Alternatives considered**:
- *Inline the behaviors in `on_key` with no registry* — rejected: violates FR-008; gives
  `/keys` nothing concrete to enumerate and forces a 006 rewrite.
- *Build the full default+alternate binding model now* — rejected: that is sprint 006
  scope; YAGNI (Principle VII). Only the default slot exists this sprint.

---

## R6 — Esc / Esc-Esc and single-selection arbitration state machine

**Decision**: Model a single optional active selection that lives in **exactly one** of
the two pads (`enum ActiveSelection { None, Input(range), Transcript(range) }` — at most
one non-`None` at a time, FR-027). Starting a selection in one pad sets it and clears the
other. Then:
- **`Ctrl+C`** (FR-028): if `ActiveSelection != None` → copy the selection and set it to
  `None`; else → send SIGINT to the child. Copy never shadows interrupt.
- **`Esc`** (FR-029): if `ActiveSelection != None` → cancel it (→ `None`); else, on a
  **single-line** buffer → clear the current line; on a **multi-line** buffer → a single
  `Esc` clears only the current line and `Esc Esc` (two consecutive `Esc` presses with no
  intervening other key) clears the whole buffer.
- **`Esc Esc`** also clears the **status message** (FR-026, `clear_status_message`) — the
  double-Esc gesture both clears the multi-line buffer (if multi-line) and the message;
  these are independent effects of the same gesture and do not depend on a wall clock.

A tiny `esc_pending: bool` (or "last key was Esc") flag, reset on any non-Esc key,
implements the double-Esc detection without timers.

**Rationale**: FR-027/028/029 fully specify the precedence; the only design choice is the
representation. A single `ActiveSelection` enum makes the "at most one" invariant
*structural* (unrepresentable to have two) rather than enforced by discipline, which is
both simpler and impossible to violate. The double-Esc is a keypress-sequence flag, not a
timeout (the spec is explicit there is **no** wall-clock dependency), so it is fully
unit-testable by feeding key sequences.

**Alternatives considered**:
- *Two independent `Option<Selection>` fields (one per pad)* — rejected: makes "at most
  one" an invariant to police rather than a structural guarantee; easy to get wrong.
- *Timeout-based double-Esc* — rejected: the spec forbids wall-clock dependence and it is
  untestable without sleeping.

---

## R7 — Page-minus-context scroll clamping

**Decision**: `Transcript::page_up`/`page_down` take the context-lines value (config
`scroll.context_lines`, default **3**, FR-014) and advance by
`(viewport.saturating_sub(context)).max(1)` lines, clamped to `[0, max_scroll]` exactly
as the current page logic clamps. `Shift+PageUp/PageDown` reuse the existing
`scroll_up(1)`/`scroll_down(1)` as the named actions `scroll_line_up`/`scroll_line_down`
(FR-015). `Shift+Home/End` reuse the existing `scroll_to_top`/`scroll_to_bottom`
(FR-016). `Home`/`End` are **removed** from the transcript scroll path and reassigned to
input-line motion (FR-017).

**Rationale**: The transcript scroll API already has `page_up`/`page_down` using
`viewport.get().max(1)` clamped to `max_scroll`; the only change is subtracting the
context lines and re-flooring to **≥ 1** so a short pad still advances (FR-014). The
line-scroll and top/bottom jumps already exist as methods — they just need their
**bindings** moved (Home/End → Shift+Home/End) and exposed as named actions. This is the
smallest change that satisfies US3 and keeps the existing clamp semantics intact.

**Alternatives considered**:
- *Add brand-new scroll methods* — rejected: the existing methods already implement the
  clamp; only the page size formula and the bindings change.

---

## Summary of decisions

| # | Topic | Decision | New dep? |
|---|-------|----------|----------|
| R1 | Bracketed paste | Enable + handle `Event::Paste`; teardown joins panic/exit restore | No |
| R2 | Word boundaries | Two hand-rolled scanners: punctuation-aware motion, whitespace-rule kill | No |
| R3 | Status surfaces | Fold the above-input rule into the new below-input fixed bar | No |
| R4 | Bar fit/truncate | Reserve mode+exit, fill middle, truncate message→cwd, never wrap | No |
| R5 | Named actions | `src/action`: `Action` enum + static default-binding table (+ reserved unmapped) | No |
| R6 | Esc / selection | Single `ActiveSelection` enum; keypress-flag double-Esc; no timers | No |
| R7 | Scroll clamp | `page = (viewport - context).max(1)`; rebind Home/End → Shift+Home/End | No |

All `[NEEDS CLARIFICATION]` resolved. **No new dependencies.** Ready for Phase 1.
