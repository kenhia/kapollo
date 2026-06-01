# `StableRowIndex` — what it buys, and what its absence costs

A focused look at the one capability that genuinely separates `wezterm-term` from
`vt100` and `alacritty_terminal`. Everything else on the scorecard is a matter of degree;
this is a structural difference. If the rest of the rubric lands ~even (and after the S3
render fix it nearly does), **this is the decider**, so it deserves its own page.

## TL;DR

- A terminal grid is constantly churning: lines scroll up into scrollback, the oldest
  lines are **evicted** when the cap is hit, and the alt-screen swaps the whole buffer.
- `StableRowIndex` is a **monotonic, absolute** id for a logical row that **does not
  change** when rows scroll, and **does not get reused** when old rows are evicted. Line
  zero is the first line the terminal ever produced; ids only ever count upward.
- The other two crates expose only **relative** positions (offset from the top of the
  current buffer, or a bottom-relative scroll amount). Relative positions *move under you*
  as output streams in.
- Anything that needs to **remember where something was** across time — a selection that
  must survive new output, a saved transcript, a search hit, a bookmark, a hyperlink
  region — wants an id that doesn't move. That is exactly `StableRowIndex`.

## The problem it solves

Picture the live viewport plus scrollback as a window onto a long, growing tape:

```text
        ┌─────────────────────────── absolute history (grows downward) ───────────────┐
stable: 0      1      2      ...    4998   4999   5000   5001   ...   N-1     N
        │ (evicted once cap exceeded)      │◄──────── retained scrollback ───────►│◄ live ►│
                                           └─ visible viewport is a slice somewhere in here ┘
```

Two facts make naive coordinates dangerous:

1. **Scrolling renumbers relative coordinates.** "Row 3 of the buffer" is a different
   logical line one second later, after three lines of output arrived. A selection stored
   as "row 3" silently slides onto the wrong text.
2. **Eviction destroys low ids in a relative scheme.** When the scrollback cap is hit, the
   top line is dropped and *every* relative index shifts by one. A bottom-relative scheme
   avoids the shift but still can't *name* a line that has scrolled off — it only knows
   "N rows up from the bottom," which also changes as the bottom moves.

`StableRowIndex` sidesteps both: the id is minted once, tied to the logical line, and
never reused. Row 5000 is row 5000 forever, whether it's on screen, deep in scrollback, or
already evicted (in which case it simply no longer resolves to a physical row — and that
*absence* is itself unambiguous, not a silently-wrong hit).

## What it buys kapollo concretely

- **Drift-free selection (the headline).** Anchor a selection to two `StableRowIndex`
  values. New output can pour in, the user can scroll, lines can evict — the selection
  still refers to *exactly* the text the user dragged over. In S1/S2 we faked this with the
  `top_row = BASE − scroll` bridge; it works for the spike's short sessions but is a
  papered-over relative scheme that will drift in a long-lived, high-throughput session.
- **Durable `/save` / transcript reconstruction.** A saved region is a stable id range.
  Re-opening, re-rendering, or diffing later is trivial and unambiguous. With relative
  coordinates you must snapshot *and* somehow re-anchor, and you can never refer to a line
  that has since scrolled off.
- **Stable anchors for everything else that points at a row:** search results, "jump to
  last command" marks, error/diagnostic gutters, OSC 8 hyperlink regions, fold ranges. All
  of them want a name for a line that survives scrolling and eviction.
- **Efficient incremental redraw.** wezterm's damage model is *keyed to stable ids plus a
  sequence number* (`get_changed_stable_rows`), so "what changed since I last drew" is a
  precise, drift-proof query — the natural partner to a frame loop that doesn't want to
  re-blit the whole grid.

## What it costs us to *not* have it (vt100 / alacritty)

We don't lose the features — we re-implement a weaker version of this id ourselves, and
inherit its sharp edges:

- **A fragile coordinate bridge.** The `BASE − scroll` trick (see
  [spike-support `coords`](../spike-support/src/coords.rs)) fabricates pseudo-absolute rows
  from a bottom-relative scroll position. It is correct only while total history stays
  below `BASE` and nothing reflows; it is a constant source of off-by-one risk.
- **Selection drift under load.** Without a real anchor, a selection held while output
  streams in must be continuously re-mapped, and any mistake slides the highlight onto the
  wrong text. This is precisely the failure mode worth hammering in **T022/T027**.
- **Eviction ambiguity.** Once the cap is hit and lines drop, our home-grown id either
  shifts (wrong) or needs a running "lines evicted so far" counter that we must maintain
  perfectly and persist for `/save`. That counter *is* a hand-rolled, bug-prone
  `StableRowIndex` — we'd be reinventing the wheel, less robustly.
- **Reflow makes it worse.** If we ever support reflow-on-resize, a relative line index
  becomes almost meaningless across a resize; an absolute logical-line id is the only sane
  anchor. (`alacritty_terminal` doesn't reflow scrollback either, so this is latent for
  both non-wezterm options.)

Note the cost is **not** "alacritty can't do selection or save." It can — we proved it in
S2. The cost is that *we* own the correctness of the absolute-id machinery, in our code,
forever, instead of leaning on a well-tested implementation inside the emulator.

## How each crate stacks up

| Capability | `vt100` (S1) | `alacritty_terminal` (S2) | `wezterm-term` (S3) |
|---|---|---|---|
| Absolute, eviction-proof row id | ✗ (visible-relative) | ✗ (bottom-relative `display_offset`) | ✓ `StableRowIndex` |
| Name a line that scrolled off | ✗ | ✗ | ✓ |
| Damage keyed to stable ids | ✗ | partial (dirty spans, not stable-id keyed) | ✓ `get_changed_stable_rows` + seqno |
| Selection anchor survives flood | only via our bridge | only via our bridge | native |
| `/save` anchor | text-only, re-anchor needed | better, still relative | stable-id range, direct |

## The honest counter-argument

`StableRowIndex` is worth a real, ongoing **supply-chain cost**: `wezterm-term` is
unpublished (git-pin a `rev`, hundreds of transitive deps incl. `image`/`terminfo`, a
heavier audit/CI surface). The fair trade to weigh tonight:

> Is a robust, library-owned absolute row id worth carrying an unpublished, heavy git
> dependency — versus owning a thinner, hand-rolled eviction counter on top of the
> lighter, crates.io-published `alacritty_terminal`?

If T022/T027 show selection/`save` drift that is **annoying to fix correctly** on
alacritty, `StableRowIndex` earns its keep and wezterm wins. If our `coords` bridge holds
up comfortably under flood and the `/save` story feels tractable with a modest
eviction counter, alacritty's lower carrying cost likely wins. This page exists so that
call is made on the *mechanism*, not on a vibe.

## What to watch for in T022 / T027

- Start a selection, then `yes`/`seq 1 100000` a flood. Does the highlight stay on the
  same text, or crawl? (Tests the bridge vs. a real anchor.)
- Scroll up into deep scrollback during/after a flood, select, copy. Is the copied text
  the lines you see?
- Force eviction: exceed `SCROLLBACK_LEN` (5000) and repeat the above near the top of
  retained history.
- Note *how hard* any drift would be to fix correctly in each engine — that effort, not
  the spike's pass/fail, is the real signal feeding the recommendation.
