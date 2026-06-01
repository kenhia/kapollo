# Planning: The Grid Pivot

> **Status: OPEN DISCUSSION (multi-turn).** This directory is a working space
> for deciding kapollo's biggest architectural fork to date: whether to adopt a
> real terminal **grid model** and what that means for the existing codebase.
> Nothing here is decided until we promote it to `specs/` + the decisions log in
> [../brainstorm.md](../brainstorm.md) and review against the constitution.

Last updated: 2026-05-31 (initial framing)

## 1. Why we're here

After living with the MVP+ (sprint 002 hardening), Ken's assessment:

1. **Commit to the project** — pursue kapollo seriously and make it great.
2. **The "no grid model" decision (D4) was wrong.** kapollo does not feel like a
   native terminal, and it can't without modeling output as a real terminal
   grid. In-place redraws (progress bars, `\r` overwrites, cursor moves), inline
   color, and general fidelity all suffer under the current append-only,
   styling-stripped block model.
3. **Need advanced terminal UX:** mouse-driven **text selection** and
   **scroll-wheel** scrolling — *and* correct hand-over so alt-screen apps
   (`vim`, `bpytop`, `top`) keep working when they want the mouse themselves.

This reverses **D4 (no grid model)** and reshapes **D3 (hybrid transcript)**,
**D22 (color scope)**, and **D24 (scrolling / deferred mouse)**. It is a
foundational change, so it gets its own planning effort.

## 2. The crux (read this first)

The three goals are **not independent**. The chain:

- A native feel requires faithfully rendering what the program drew →
  **a terminal grid** (rows × columns of styled cells with a cursor that can
  move and overwrite). This is precisely what D4 said we would *not* build.
- Mouse **text selection** requires capturing mouse events. The moment kapollo
  captures the mouse, the host terminal's **native click-drag selection is
  disabled**. To give the user selection back, kapollo must render its own
  selection highlight over **cells it owns** → again, a grid.
- Therefore **goal #3 (mouse selection) forces goal #2 (grid)**. You cannot have
  app-driven selection without owning the cells. Scroll-wheel and alt-screen
  hand-over fall out of the same grid + mouse-routing machinery.

**Bottom line:** this is really one decision (adopt a grid model) with several
consequences, not three separable features.

## 3. What we need to decide (the agenda)

| # | Question | Doc |
|---|----------|-----|
| Q1 | Is the advanced mouse UX (selection + wheel + alt-screen hand-over) actually achievable, and with what building blocks? | [01-research-grid-and-mouse.md](01-research-grid-and-mouse.md) |
| Q2 | Grid scope: emulate the **whole** main screen as one scrollback terminal, or a **per-block** mini-grid, or grid-only-for-alt-screen? | [01](01-research-grid-and-mouse.md) + this doc |
| Q3 | Which crate(s): `alacritty_terminal`, `vt100`/`tui-term`, `wezterm-term`/`termwiz`, or hand-rolled? | [01](01-research-grid-and-mouse.md) |
| Q4 | **Rework in place** vs **start (mostly) over** — and what carries over either way? | [02-rework-vs-rewrite.md](02-rework-vs-rewrite.md) |
| Q5 | How do blocks (`/save`, `/filter`, the AI layer, OSC 133, exit codes) survive on top of a grid? | [02](02-rework-vs-rewrite.md) |
| Q6 | Does this still honor the constitution, and what superseding decisions (D25+) do we record? | TBD after Q1–Q5 |

## 4. Non-negotiables to preserve through the pivot

These are the parts of the vision that must survive whatever we choose:

- **Block model as the source of truth** (D8): command + output + exit code,
  feeding `/save`, `/filter`, and the AI layer. A grid must *augment* blocks,
  not erase them.
- **Wrap a real, configurable shell via PTY** (D1/D2). Unchanged.
- **Input-pad-at-bottom / transcript-above** metaphor (§2). Unchanged.
- **TUI integrity** (Constitution VI): logs off-screen, panic boundary, clean
  teardown. Arguably *harder* with a grid + mouse + clipboard; must stay true.
- **Linux-first** (D9); cross-platform door stays open (the candidate crates are
  cross-platform).

## 5. How we'll work this

1. Settle **Q1** (feasibility) — mostly research, see doc 01. *(drafted)*
2. Settle **Q4** (rework vs rewrite) — see doc 02; depends on a rough Q2/Q3
   direction. *(drafted, decision OPEN)*
3. Pick **Q2 + Q3** (grid scope + crate) — likely a spike/prototype to de-risk.
4. Write the superseding decisions (**Q6**, D25+) into
   [../brainstorm.md](../brainstorm.md) and revise
   [../../../docs/architecture.md](../../../docs/architecture.md).
5. Only then: a real spec under `specs/00X-grid-...` and a sprint plan.

## 6. Open questions for Ken (to drive the next turn)

1. **Scope of fidelity:** do you want kapollo to faithfully render *everything* a
   program draws on the main screen (true emulator), or "good enough" inline
   rendering with passthrough still doing the heavy lifting for fancy cases?
   - Leaning toward the former, "true emulator". Extended "spike" may provide
     additional direction indicators.
2. **Selection semantics:** terminal-style block/stream selection only, or
   smarter "select this block's output" affordances (which the block model
   uniquely enables)?
    - The later. "Brainstorm" level of thinking here:
        - Click-drag works as expected with termination being second left click
          or `ESC` -> cancel, right click or `Ctrl-C` -> capture to clipboard
        - Right click *before* selection offers copy options:
            - Block's output with command
            - Block's output (no command)
            - Current line
            - Advanced menu
                - Select range of commands to copy
3. **Clipboard path:** OSC 52 (works over SSH, terminal-mediated) vs a local
   clipboard crate (`arboard`) — or both with a fallback?
    - Suspect we need to investigate options in the "spike"
4. **Appetite for a prototype spike** before committing to rework-vs-rewrite? A
   ~1–2 day spike embedding a real grid in the transcript pad would de-risk Q2–Q4
   far better than more analysis.
    - Big appetite, not time limited. This is feeling like a "passion project"
      that I want to get right and I want to spend the time necessary. My
      intuition is that we can quickly prove feasibility of the core
      (mouse/render/feel) but that we will want to spend time deciding which
      crate(s) best serve our goals. I'm thinking the "Extended Spike" will
      start with `vt100` then move to `alacritty_terminal` then to `wezterm` (to
      do things right we will want good grapheme and hyperlink support and I
      think the image thing would be a cool "cherry on top"). Doing all three
      *will* take longer, but it will let us really dig into what we get with
      each, what the tradeoffs are, etc...and I suspect we may accidentally come
      across some things that we want to roll in (or specifically avoid) when we
      shift back to the primary project (see also answer to 5).
5. **Timeline/independence:** is this the next sprint (003), or do we want a
   throwaway prototype branch first?
    - Mix - rough thought is to share our research at the nuts-and-bolts level.
      Do the rework in place (branch/sprint) *but* publish it as a separate repo
      on GitHub, something like:
        - publish current `002-mvp-hardening` sprint to "clear the decks"
        - create new repo thinking `kenhia/kdelos` (Apollo's birthplace) with a
          README.md that clearly states what we are doing (investigating for the
          architecture redirect)...work may be of interest to some in "how did
          we get to the final implementation.
        - As noted in 4, take our time to explore so we get this right

**Interesting observation:** I am using GHCP CLI to work on a separate project.
It looks and feels like what I'm envisioning the end result of `kapollo`, so I'm
paying a bit more attention to the decisions that team made. For mouse selection
copy, it did not do what I expected, but I think I like how it handles things
better. If I drag while in the "output pad", the selection stays in that pad,
same for the "input pad". Even cooler, if I scroll up in either pad, start
selection and move down, when I get to the bottom, the text scrolls allowing me
to select a larger range (works from bottom pulling up as well).

I do find it odd that the GHCP CLI is not (at least that I could find) an Open
Source project. There is https://github.com/github/copilot-cli, but that appears
to be a "release and install" only repo.

---

## 7. Where we've landed — pass 2 (2026-05-31, GH Copilot response)

Reacting to Ken's answers above. Agreements, push-backs, and what's now firm.

### 7.1 Confirmed direction

- **Grid scope = Option A** (whole main screen + scrollback; blocks become an
  annotation layer over rows). Confirmed both sides.
- **Path A (rework in place) + spike-first.** Confirmed.
- **Fidelity = true emulator.** Confirmed — and a consequence: this likely rules
  `vt100` *out for production* (it's a spike stepping-stone) and points prod at
  `alacritty_terminal` or `wezterm-term`. The extended spike will confirm.
- **Clipboard default = OSC 52**, with terminal-survey + local-crate fallback
  evaluated during the spike.
- **Extended, un-timeboxed spike** across `vt100` → `alacritty_terminal` →
  `wezterm-term`. Endorsed — structured as a comparable scorecard in
  [03-spike-plan.md](03-spike-plan.md) so the three stay apples-to-apples.

### 7.2 Push-back / flags (please react)

1. **Ctrl-C conflict (must resolve now).** "Ctrl-C → copy" collides with
   SIGINT-to-the-running-command (FR-024), which we cannot globally remap.
   **Proposed rule:** Ctrl-C copies **only while a selection is active**;
   otherwise it is SIGINT exactly as today. The same state-based disambiguation
   resolves the two right-click behaviors: **right-click with no active
   selection → context menu; right-click with an active selection → copy.** This
   makes your selection brainstorm internally consistent. → folded into
   [03-spike-plan.md](03-spike-plan.md) §Selection.
2. **GHCP CLI selection behavior = a requirement, not a footnote.** Your
   observation (selection **scoped to the pad** it began in; **auto-scrolls** when
   the drag passes the edge, both directions, to extend the range) is a real,
   non-obvious design. It implies selection anchored in **content coordinates**
   (scrollback row/col), not screen coordinates, so it survives scrolling, plus a
   drag-past-edge → auto-scroll loop. Captured as a first-class spike requirement.
3. **`/save` reconstruction — accept, with a cheap insurance policy.**
   Reconstruction-for-v1 is fine **iff** all block text goes through a single
   accessor (`block.text()` / `block.text_with_command()`), making the
   reconstruct ↔ parallel-byte-retention swap a one-function change (your "~1
   sprint" bar). Recorded tradeoff: reconstruction is **lossy for output that
   scrolled past the scrollback cap** and depends on the crate's grapheme
   handling. → refines D29 in [02-rework-vs-rewrite.md](02-rework-vs-rewrite.md).

### 7.3 RESOLVED: no separate repo — `delos/` subdirectory, in place

Ken's call (03-spike-plan §"Repos/kdelos/etc."), and it's cleaner than the
hybrid: **no separate GitHub repo.** "delos" becomes a **subdirectory inside
kapollo** on a spike branch. Rationale: keeps history + green tests, "shows the
work" for future readers, and dissolves the (i)/(ii) split — Delos is a
birthplace *folder*, not a fork.

- All spike work lives under `delos/`.
- One spike branch, multiple commits (no need for parallel branches).
- At spike end, lean toward **merging into `main`** *provided* all spike work
  stays contained in `delos/` (so it never entangles the shipping binary).

**DECIDED — `delos/` is its own Cargo workspace, never touches the production
crate.** kapollo is a single crate; the three throwaway spike binaries live as
**separate crates in a Cargo workspace** under `delos/`. This guarantees heavy
spike deps (`wezterm-term`, `alacritty_terminal`, etc.) **never** enter the
shipping `kapollo`/`kap` dependency tree, build, or lockfile-relevant graph. No
feature-gated `[[bin]]` targets in the main crate. The root `kapollo` crate must
not list `delos/*` as path deps or workspace members that pull spike deps into
its build.

**Commit hygiene:** the `specs/planning/grid-pivot/` docs are **not** committed
with `002-mvp-hardening`. They land on the **spike branch** instead, keeping the
002 ship focused on shipped work.

### 7.4 Decks-clearing sequence (agreed)

1. Ship **002-mvp-hardening** to `main` (the `sprint-ship` skill). **T042** is
   **deferred** (manual quickstart not re-walked) — mark it deferred in tasks.md
   citing the re-architecture plan, rather than blocking the ship.
2. Cut a **spike branch**; create `delos/` (Cargo workspace) per §7.3.
3. Run the three-stage spike per [03-spike-plan.md](03-spike-plan.md); fill the
   scorecard; write the nuts-and-bolts notes (all under `delos/`).
4. Promote firm decisions to D25+ in [../brainstorm.md](../brainstorm.md) and
   revise [../../../docs/architecture.md](../../../docs/architecture.md).
5. Spec + sprint the in-place rework.

### 7.5 GHCP CLI being closed-source

Confirmed: `github/copilot-cli` is a release/install-only repo — no source to
read. That's fine; the **behaviors** are observable and worth emulating (we just
can't copy implementation). Nothing actionable beyond "treat it as a UX
reference, not a code reference."