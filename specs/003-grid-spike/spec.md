# Feature Specification: Terminal-Grid Spike

**Feature Branch**: `003-grid-spike`  
**Created**: 2026-06-01  
**Status**: Draft  
**Input**: User description: "kapollo terminal-grid spike — an exploratory research spike (NOT the production rework) that builds the same vertical slice on three terminal-emulator crates and scores them against one rubric to choose the production grid crate."

## Context & Intent *(informative)*

This is an **exploratory research spike**, not a production feature. Its output is
**knowledge and a recommendation**, not shipped product capability. kapollo's MVP
decided against a terminal grid model (decision D4); the grid-pivot planning effort
(`specs/planning/grid-pivot/`) reverses that direction. Before committing to an
in-place rework, this spike de-risks the change by building one identical vertical
slice on three candidate terminal-emulator crates — `vt100` (optionally via
`tui-term`), `alacritty_terminal`, and `wezterm-term`/`termwiz` — and scoring them
against a single rubric.

The spike's deliverables (a filled scorecard, per-stage writeups, and a crate
recommendation) feed the decisions D25–D30 and the subsequent in-place rework spec.
The rework itself is **out of scope** here and will be specified separately after
this spike selects a crate.

Source of truth: `specs/planning/grid-pivot/03-spike-plan.md` (primary), with
`00-overview.md` §7, `01-research-grid-and-mouse.md`, and `02-rework-vs-rewrite.md`.

## User Scenarios & Testing *(mandatory)*

> The "user" for this spike is the kapollo maintainer making the production crate
> decision. Each user story is an independently demonstrable slice of the
> investigation. Stories S1→S2→S3 build the *same* vertical slice on successive
> crates; the slice is the unit of value because a fully-working slice on any one
> crate already teaches whether the selection model and alt-screen handover are
> achievable.

### User Story 1 - Working vertical slice on the first crate (Priority: P1)

As the maintainer, I build the complete vertical slice on `vt100` (optionally
accelerated by `tui-term`) so I can prove the core "feel" — grid render + mouse
selection + scroll + alt-screen handover — and learn the shape of the problem
before investing in heavier crates.

**Why this priority**: This is the minimum viable spike result. Even if no other
crate is evaluated, a working slice on one crate answers the central feasibility
question ("is the selection model + alt-screen handover achievable?") and produces
the first scorecard column. It is the foundation every later stage reuses.

**Independent Test**: Run the `vt100` spike binary against an interactive shell in
Windows Terminal Preview; confirm output renders as a styled grid, a click-drag
selects content (in content coordinates), the wheel scrolls scrollback, launching
`vi` hands the screen over and exits cleanly, and a selection copies to the
clipboard on release. Fill the S1 column of the scorecard.

**Acceptance Scenarios**:

1. **Given** the `vt100` spike binary built inside the `delos/` workspace, **When** the maintainer runs it and types shell commands, **Then** command output renders as a styled cell grid (main screen) reflecting SGR colors and cursor moves.
2. **Given** rendered output, **When** the maintainer click-drags across cells, **Then** a selection highlight tracks the drag anchored in content (scrollback) coordinates and the selection stays scoped to the output region.
3. **Given** an active drag that reaches the top or bottom edge, **When** the drag continues past the edge, **Then** the view auto-scrolls in that direction (both directions supported) and the selection range extends accordingly.
4. **Given** an active selection, **When** the maintainer releases the mouse button, **Then** the selected text is copied to the clipboard (OSC 52 default).
5. **Given** the spike running on the main screen, **When** the maintainer launches an alt-screen app (e.g. `vi`, `bpytop`), **Then** kapollo stops owning the grid and passes input/output through so the app works, and restores its own handling cleanly on exit.
6. **Given** the completed S1 slice, **When** the maintainer evaluates it against the rubric, **Then** every S1 scorecard criterion has an entry and a short nuts-and-bolts writeup exists.

---

### User Story 2 - Same slice on the proven baseline crate (Priority: P2)

As the maintainer, I rebuild the identical vertical slice on `alacritty_terminal`
(the "correct, proven" baseline used by Zed) so I can compare its real scrollback,
damage tracking, and selection primitives against the `vt100` result on equal terms.

**Why this priority**: `alacritty_terminal` is the leading production candidate. A
working slice here gives the apples-to-apples comparison that the crate decision
actually rests on. It depends only on the slice definition established in S1.

**Independent Test**: Run the `alacritty_terminal` spike binary through the same
manual script as S1 in the host-terminal matrix; confirm identical behaviors
(render, selection, auto-scroll, copy, alt-screen handover); fill the S2 scorecard
column and writeup.

**Acceptance Scenarios**:

1. **Given** the `alacritty_terminal` spike binary, **When** run through the same manual script as S1, **Then** it exhibits the same observable selection, scroll, copy, and alt-screen behaviors.
2. **Given** the S2 slice, **When** evaluated against the rubric, **Then** the S2 scorecard column is filled and a nuts-and-bolts writeup notes what surprised the maintainer versus S1.
3. **Given** a high-throughput output flood, **When** the maintainer observes rendering, **Then** the writeup records damage/dirty-tracking behavior and responsiveness for comparison.

---

### User Story 3 - Same slice on the maximum-fidelity crate (Priority: P3)

As the maintainer, I rebuild the identical vertical slice on `wezterm-term` /
`termwiz` to evaluate the most complete option (graphemes, hyperlinks, optional
images) and decide whether its extra fidelity justifies its weight/API cost.

**Why this priority**: This is the richest but heaviest option; it completes the
three-way comparison. Image support here is a stretch goal that must not block the
decision.

**Independent Test**: Run the `wezterm-term`/`termwiz` spike binary through the same
manual script; confirm the core behaviors; probe grapheme segmentation, OSC 8
hyperlinks, and (as a cherry) image protocol forwarding; fill the S3 scorecard
column and writeup.

**Acceptance Scenarios**:

1. **Given** the `wezterm-term`/`termwiz` spike binary, **When** run through the same manual script, **Then** it exhibits the same observable core behaviors as S1/S2.
2. **Given** content with wide/combining characters and OSC 8 hyperlinks, **When** rendered, **Then** the writeup records grapheme segmentation and hyperlink fidelity.
3. **Given** an image escape (sixel/kitty/iTerm) as a cherry goal, **When** the maintainer probes forwarding through the owned grid, **Then** the result is recorded as achievable/not-achievable without spending multiple days, and image support never gates the crate decision.

---

### User Story 4 - Crate recommendation from the scorecard (Priority: P1)

As the maintainer, I synthesize the three filled scorecard columns into a single
production-crate recommendation with rationale, plus confirmation (or a documented
reason against) that the selection model and alt-screen handover are achievable.

**Why this priority**: This is the spike's reason for existing. The recommendation
and feasibility confirmation are what feed D25–D30 and unblock the rework spec.
It is P1 because without it the spike produced effort but no decision.

**Independent Test**: Review the completed scorecard and writeups; confirm a written
recommendation names a production crate, cites rubric evidence, and states whether
the selection model + alt-screen handover are achievable.

**Acceptance Scenarios**:

1. **Given** all three scorecard columns filled, **When** the maintainer writes the recommendation, **Then** it names one production crate and justifies the choice against the weighted rubric criteria.
2. **Given** the slice results, **When** the recommendation is finalized, **Then** it explicitly confirms the selection model and alt-screen handover are achievable, or documents why they are not.
3. **Given** the recommendation, **When** the spike closes, **Then** its outputs are positioned to feed the D25–D30 promotion and the in-place rework spec.

---

### Edge Cases

- **Selection vs. SIGINT (Ctrl-C) conflict**: With **no active selection**, `Ctrl-C` must send SIGINT to the child (unchanged from kapollo's **002** FR-024 — Ctrl-C→SIGINT) and right-click must open the context menu. With an **active selection**, `Ctrl-C` copies and right-click copies. State (selection active?) disambiguates.
- **Shift held during mouse interaction**: Shift bypasses kapollo's selection and forwards the mouse event to the child app (native escape hatch).
- **Drag-past-edge in both directions**: Auto-scroll must work pulling the drag below the bottom edge *and* above the top edge, extending the range either way.
- **Selection survives scrolling**: Because the anchor is in content (scrollback row/col) coordinates, scrolling away and back must not lose or shift the selection.
- **Alt-screen apps that request mouse modes**: An inner app requesting mouse reporting (CSI ?1000/1002/1003/1006h) on the main screen, and alt-screen entry (?1049h), must both route the mouse to the child rather than to kapollo selection.
- **Clipboard fallback**: When a host terminal does not honor OSC 52, the writeup must note it and the spike evaluates a local-crate fallback.
- **Output flood**: Heavy/continuous output must not make the grid render or selection unusable; damage/dirty tracking behavior is recorded per crate.
- **Scrollback eviction**: Content scrolled past the scrollback cap is no longer selectable/reconstructable; the writeup notes this limitation per crate (relevant to later `/save` reconstruction).
- **Image escapes through an owned grid**: Since kapollo owns the cells, image protocols cannot simply pass through untouched; S3 records whether forwarding is even possible.

## Requirements *(mandatory)*

### Functional Requirements

#### Spike scope & isolation

- **FR-001**: The spike MUST build the *same* vertical slice on three terminal-emulator crates, evaluated sequentially: S1 `vt100` (optionally via `tui-term`) → S2 `alacritty_terminal` → S3 `wezterm-term`/`termwiz`.
- **FR-002**: All spike work MUST live under a `delos/` subdirectory, which is its **own Cargo workspace** of throwaway crates (one spike binary crate per stage).
- **FR-003**: Heavy spike dependencies (e.g. `wezterm-term`, `alacritty_terminal`) MUST NEVER enter the shipping `kapollo`/`kap` dependency tree, build, or lockfile-relevant graph. The root `kapollo` crate MUST NOT list `delos/*` as path deps or workspace members that pull spike deps into its build, and MUST NOT use feature-gated `[[bin]]` targets in the main crate for spike work.
- **FR-004**: The shipping kapollo/kap crate MUST continue to build and pass its existing tests unchanged while the spike exists (the spike is additive and contained).

#### The vertical slice (identical per crate)

- **FR-005**: Each spike binary MUST spawn a PTY-backed shell, reusing kapollo's existing `portable-pty` setup.
- **FR-006**: Each spike binary MUST feed shell output into the crate's grid/parser model.
- **FR-007**: Each spike binary MUST render the grid via ratatui (cells → styled spans), for the **main screen only**.
- **FR-008**: Each spike binary MUST support mouse click-drag text selection with the selection anchored in **content coordinates** (scrollback row/col), so the selection survives scrolling.
- **FR-009**: The selection MUST be scoped to the output region (a selection started in the output region stays there).
- **FR-010**: When a drag passes the top or bottom edge of the viewport, the view MUST auto-scroll in that direction (both directions supported) and extend the selection range.
- **FR-011**: On mouse release, an in-progress drag MUST finalize into an Active selection that stays highlighted; release MUST NOT itself copy. Copying the selection happens only via an explicit trigger while a selection is Active (right-press or `Ctrl-C`, per FR-016), each of which copies and then deselects.
- **FR-012**: The mouse wheel MUST scroll the scrollback.
- **FR-013**: When the child enters the alternate screen (`?1049h`), the spike MUST stop owning the grid and pass through input/output so alt-screen apps (e.g. `vi`, `bpytop`) work, and MUST restore its own handling cleanly on alt-screen exit (`?1049l`).
- **FR-014**: When the child requests mouse reporting on the main screen (CSI ?1000/1002/1003/1006h), the spike MUST forward mouse events to the child rather than consume them for selection.

#### Selection copy-trigger & routing rules

- **FR-015**: With **no active selection**, right-click MUST open a context menu and `Ctrl-C` MUST send SIGINT to the child (unchanged, preserving kapollo's **002** FR-024 behavior — Ctrl-C→SIGINT; distinct from this spec's FR-024 below).
- **FR-016**: With an **active selection**, right-click MUST copy the selection and then deselect; `Ctrl-C` MUST likewise copy the selection and then deselect.
- **FR-017**: Holding **Shift** during a mouse interaction MUST bypass kapollo's selection and forward the mouse event to the child app.
- **FR-018**: Selection termination MUST support: mouse **release** finalizes an in-progress drag into an Active (highlighted) selection; a second left-click OR `ESC` cancels an Active selection without copying; right-click or `Ctrl-C` on an Active selection copies and deselects (FR-016).
- **FR-019**: The context menu in the spike MUST be a trivial "Hello, World." menu that only proves render + route; the real menu entries are **deferred to the rework**.

#### Clipboard

- **FR-020**: The clipboard path MUST default to OSC 52, and the spike MUST record which host terminals honor it.
- **FR-021**: The spike MUST evaluate a local-crate clipboard fallback for terminals that do not honor OSC 52.

#### Evaluation & deliverables

- **FR-022**: The spike MUST fill all three columns of a shared scorecard rubric. The rubric criteria MUST include: render correctness (SGR, wide chars, combining); grapheme/Unicode segmentation; scrollback API (cap, eviction, reflow on resize); selection primitives (or ease of hand-rolling); mouse routing / alt-screen handover ergonomics; hyperlinks (OSC 8); images (sixel/kitty/iTerm); damage/dirty tracking under flood; API ergonomics in the event loop; binary size / build time / dep weight; maintenance health; and text reconstruction for `/save`.
- **FR-023**: The spike MUST produce a short nuts-and-bolts writeup per stage capturing surprises and gotchas.
- **FR-024**: The spike MUST produce a single production-crate recommendation with rationale grounded in the weighted rubric.
- **FR-025**: The spike MUST confirm that the selection model and alt-screen handover are achievable, or document a specific reason they are not.
- **FR-026**: The spike MUST validate the slice (especially OSC 52 clipboard, mouse, alt-screen) across the host-terminal matrix: Windows Terminal Preview (primary); GNOME Terminal and Konsole on Ubuntu (secondary). macOS is out of scope. WezTerm/Alacritty/Kitty are optional.
- **FR-027**: The spike outputs MUST be structured to feed the D25–D30 decision promotion and the subsequent in-place rework spec.

#### Stretch / cherry

- **FR-028**: Image support (sixel/kitty/iTerm) is a **cherry / easy-cut stretch goal**. The spike MAY probe whether image protocols can be forwarded through an owned grid in S3, but image support MUST NOT gate the crate decision and MUST NOT consume multiple days. (Windows Terminal does not support it today.)

### Key Entities *(include if feature involves data)*

- **Vertical slice**: The identical throwaway demonstration built per crate — PTY-backed shell + grid render + mouse selection + scroll + alt-screen handover.
- **Spike binary / stage**: One throwaway crate per candidate (`spike-vt100`, `spike-alacritty`, `spike-wezterm`) living in the `delos/` Cargo workspace.
- **Scorecard**: The shared weighted rubric with one column per stage; the comparison artifact driving the decision.
- **Selection**: A range anchored in content (scrollback) coordinates, scoped to the output region, with an active/inactive state that drives copy-trigger and right-click behavior.
- **Nuts-and-bolts writeup**: Per-stage prose recording surprises, gotchas, and rubric evidence.
- **Crate recommendation**: The synthesized decision output, with rationale, feeding D25–D30 and the rework spec.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All three scorecard columns are fully filled — every listed rubric criterion has an entry for S1, S2, and S3.
- **SC-002**: Each of the three stages has a written nuts-and-bolts writeup.
- **SC-003**: Exactly one production crate is recommended, with rationale tied to the weighted rubric.
- **SC-004**: The selection model (content-coordinate anchor, output-region scope, auto-scroll-on-drag-past-edge both directions, finalize-on-release with explicit copy via right-press/Ctrl-C) is demonstrated working on at least one crate, and its achievability is explicitly confirmed or refuted in writing.
- **SC-005**: Alt-screen handover (enter → pass through, exit → restore) is demonstrated working on at least one crate with an alt-screen app such as `vi` or `bpytop`.
- **SC-006**: The slice is validated on Windows Terminal Preview (primary) and on at least one secondary Ubuntu terminal (GNOME Terminal or Konsole), with OSC 52 clipboard support recorded per terminal.
- **SC-007**: The shipping `kapollo`/`kap` build and its existing test suite remain green throughout, and the shipping dependency graph gains zero spike dependencies (verifiable via the lockfile/dependency graph).
- **SC-008**: The state-based copy-trigger rules are demonstrable: with no selection, Ctrl-C sends SIGINT and right-click opens the "Hello, World." menu; with an active selection, Ctrl-C and right-click both copy.

## Assumptions

- The maintainer (single power user) is the audience and operator of the spike; no multi-user or productized UX is required.
- `tui-term` may be used to accelerate S1 to first pixels; the team decides during S1 whether the raw `vt100` API is needed.
- 002-mvp-hardening has shipped to `main` (decks cleared) before the spike runs; the spike proceeds on its own branch with all work contained in `delos/`.
- The spike is **not time-boxed**; a stage is "done" when its scorecard column is fillable, not when it is perfect.
- Grid scope is the whole main screen with scrollback (planning Option A); per-block mini-grids and grid-only-for-alt-screen are not evaluated here.
- Block annotation, `/save`, `/filter`, the AI layer, and the input pad are **out of scope** for the spike — they are additive and do not change crate selection.
- The full context menu, the production rework, and the D25–D30 decision records are out of scope; the spike *feeds* them.
- macOS validation is out of scope (the maintainer's Mac needs updating); optional cross-platform terminals (WezTerm/Alacritty/Kitty) are nice-to-have.
- The spike intentionally produces throwaway code; correctness of the slice matters only insofar as it makes the rubric fillable and the feel demonstrable.

## Dependencies

- kapollo's existing `portable-pty` setup and shell/PTY plumbing (reused by each slice).
- Candidate crates: `vt100` (and optionally `tui-term`), `alacritty_terminal`, `wezterm-term`/`termwiz`.
- `ratatui` for rendering the grid in each slice.
- Host terminals for the test matrix: Windows Terminal Preview, GNOME Terminal, Konsole.
- The grid-pivot planning docs under `specs/planning/grid-pivot/` as the authoritative direction.
