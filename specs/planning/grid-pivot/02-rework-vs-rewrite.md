# Decision: Rework In Place vs. Start (Mostly) Over

> Answers **Q4** (path forward) and **Q5** (how blocks survive on a grid).
> Status: **DECISION OPEN** — analysis + recommendation for discussion.

Last updated: 2026-05-31

## 1. How big is what we have?

Measured 2026-05-31: **~2,857 LOC source**, **~1,131 LOC tests**, 12 deps. Small
enough that neither path is catastrophic — which means the decision should be
driven by *how much is reusable*, not by sunk cost.

## 2. Reuse map (what carries over regardless of path)

| Module | LOC | Verdict on grid pivot |
|--------|-----|-----------------------|
| `src/pty/` (mod + shell) | ~363 | **Keep.** PTY spawn, resize, signals, fish/bash hooks, OSC 133/7 emit — all unaffected. |
| `src/config.rs` | 237 | **Keep + extend** (mouse, selection, clipboard, scroll keys). |
| `src/slash/` | 111 | **Keep.** Registry + builtins are model-agnostic. |
| `src/input/` | ~295 | **Keep most.** Router (slash vs passthrough, leader escape) survives; key handling grows for mouse/selection. |
| `src/logging.rs`, `src/error.rs` | ~75 | **Keep.** |
| `src/session/` (block, ringbuf, mod) | ~342 | **Rework.** Block becomes an annotation over grid rows; ringbuf likely replaced by the emulator's scrollback (see §4). |
| `src/output/parser.rs` | 224 | **Mostly replaced.** The emulator's parser applies escapes to cells; we still tap the stream for OSC 133/7 + alt-screen. |
| `src/output/mod.rs`, `sentinel.rs` | ~262 | **Rework.** Boundary plumbing re-pointed at grid row ranges; sentinel fallback survives in spirit. |
| `src/ui/transcript.rs`, `passthrough.rs` | ~263 | **Replace.** This is the heart of the change: render the grid + selection; fold passthrough into grid hand-over. |
| `src/ui/input_pad.rs`, `status.rs`, `mod.rs` | ~224 | **Keep most.** Chrome is largely unaffected; layout tweaks only. |
| `src/app.rs` | 329 | **Heavy rework.** Event loop grows mouse routing, selection, scrollback, grid feed. |

**Rough split:** ~50–60% (PTY, config, slash, input router, logging, chrome,
shell hooks) carries over **as-is or lightly touched**; ~40–50% (output→grid,
session/block, transcript render, passthrough, event loop) is exactly the core
that this pivot rewrites *anyway*.

## 3. The two paths

### Path A — Rework in place (recommended, framed as "core replacement")
Swap the output/transcript/render core on a feature branch while keeping the
stable plumbing (PTY, config, slash, input, chrome, hooks). Concretely:
- Introduce a `grid`/`terminal` module that owns the emulated screen + scrollback.
- Re-point `output` to feed the grid and emit OSC 133/7 as a side-channel.
- Rebuild `ui/transcript` to render the grid + selection; fold `passthrough` into
  mouse/keyboard routing keyed on alt-screen + inner mouse mode.
- Re-model `session::Block` as a row-range annotation (see §4).
- Grow `app` event loop for mouse + scrollback.

**Pros:** keeps ~half the code + all tests for the stable layers; incremental,
demoable at each step; preserves git history and the working binary. **Cons:**
must carefully delete/replace D4-era assumptions without leaving hybrid cruft.

### Path B — Start (mostly) over
New crate skeleton; port the reusable modules across deliberately.

**Pros:** clean slate for the grid-centric architecture; no risk of half-migrated
abstractions; a chance to re-draw module boundaries around the grid. **Cons:**
we'd re-port ~50–60% that already works (PTY, config, slash, input, chrome,
hooks) and re-write their tests for little benefit; throws away green tests and
history; slower to a working binary.

## 4. How blocks survive on a grid (Q5)

The block model is non-negotiable (D8). On a grid it becomes an **annotation /
index layer** rather than a separate byte store:

- The emulated main screen has **scrollback**: an ever-growing sequence of rows.
- OSC 133 `A/B/C/D` marks delimit **row ranges**: a block = `{ command,
  start_row, end_row, exit_code, started/ended }` over the grid's scrollback.
- For `/save` / `/filter` / AI we still need the **bytes/text** of a block. Two
  options: (a) reconstruct text from the block's grid rows on demand, or (b)
  keep retaining raw output bytes per block in parallel (today's `ringbuf`) as
  the canonical "save" source while the grid is the canonical "render" source.
  *Leaning (b)* — cheap, keeps `/save` byte-exact, and the grid stays purely a
  render/selection concern. Caps (D14) then apply to both scrollback rows and
  retained bytes.
  **Update (pass 2, D29):** Ken chose **(a) reconstruction** for v1, behind a
  single `block.text()` accessor so (b) remains a ~1-sprint swap. Keep the
  accessor abstraction even if we later flip to (b).
- Selection can be **block-aware**: "select this block's output" becomes a
  first-class affordance the grid+annotation layer uniquely enables (a perk over
  a plain terminal).

This means **D8/D13/D14 all still hold**; the grid is additive.

## 5. Recommendation

**Path A (rework in place as a core replacement), on a dedicated branch, after a
short spike** (doc 01 §7) that proves the grid + mouse routing with `tui-term`/
`vt100` in the existing app. Rationale: the reusable half is genuinely reusable
and well-tested; the rewritten half is the exact core we'd rewrite under either
path; Path B's "clean slate" benefit is small because the grid module is new
code either way and can be designed cleanly *inside* the existing crate.

**Caveat that could flip this to B:** if the spike shows the existing
`session`/`output`/`app` abstractions actively fight a grid-centric design (e.g.
the event loop or block model can't cleanly host scrollback + mouse), a
from-scratch skeleton with deliberate ports becomes worth it. Let the spike
decide.

## 6. Decision record to write once we choose (Q6)

- **D25** — Reverse **D4**: kapollo **does** maintain a terminal grid model for
  the main screen (supersedes "no grid model").
- **D26** — Grid scope (A/B/C from doc 01 §5).
- **D27** — Crate choice (spike vs prod).
- **D28** — Mouse: selection + wheel + alt-screen/inner-mode routing; Shift to
  bypass; clipboard via OSC 52 (revises **D24**'s "mouse deferred/opt-in").
- **D29** — Block-as-annotation-over-grid. `/save`/`/filter`/AI text comes from
  **reconstruction from grid rows** for v1 (Ken, pass 2), gated behind a single
  accessor (`block.text()` / `block.text_with_command()`) so swapping to
  parallel byte retention is a one-function change (~1 sprint). Tradeoff:
  reconstruction is lossy for rows evicted past the scrollback cap and depends
  on the crate's grapheme handling. (Refines **D8/D13/D14**; supersedes the
  doc-04 "leaning (b)" note below.)
- **D30** — Inline SGR color now rendered (revises **D22**'s Tier-2 deferral).
- **Path** — rework-in-place vs rewrite (this doc).

## 7. Open questions for Ken

1. Agree with **Path A + spike-first**, or do you want a clean-slate Path B?
    - Strong agreement. See `specs/planning/grid-pivot/00-overview.md` responses
2. For `/save` fidelity, prefer **retaining raw bytes in parallel** (byte-exact)
   or **reconstructing from grid rows** (less memory, lossy on exotic content)?
    - I *think* reconstruction is okay. If this is something that will be deep
      in architecture we should discuss it more. If it's something we can decide
      and make a decision note with the ability to switch in ~ 1 sprint worth of
      work than let's go with "reconstruction" and note the decision with
      tradeoffs.
3. Want **block-aware selection** ("select this block") as a headline feature, or
   keep selection purely terminal-style for v1?
    - See answer in 6.2 of `specs/planning/grid-pivot/00-overview.md` along with
      the "Interesting observation" at the bottom of the same file.
