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
  both non-wezterm options — but see [the deep-history note below](#does-a-deep-history-block-store-change-the-calculus), which softens this considerably.)

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

## Does a "deep history" block store change the calculus?

A planned-but-not-spiked kapollo feature reframes part of this trade, so it's worth
pricing in now. The idea: capture every command as a semantic **block** —
`command + output + exit code` — into a durable store (a DB on disk, or simply in memory at
roughly twice the scrollback footprint), with tiered eviction (drop the output but keep the
command line; or evict the block wholly). Modern RAM makes the in-memory version
essentially free at any sane scrollback size — a few thousand fully-styled lines is
single-digit megabytes, and storing plain text plus sparse style-runs is lighter still. The
"~2× scrollback" estimate is the right order of magnitude and only an issue at fidelity you
can dial down.

Does owning that store reduce the need for `StableRowIndex`? **Partly — and in a way that
genuinely shifts the decision, but does not erase the advantage.** Split by use case:

**What the block store subsumes outright (crate-agnostic):**

- **`/save`, transcript, replay.** A block store is a *richer* record than a stable-id
  range — it carries command boundaries and exit codes, not just rows. Reconstruction is
  "dump these blocks," no re-anchoring. This was a headline `StableRowIndex` benefit; the
  block store does it *better*, on any engine.
- **Jump-to-command, command marks, error gutters.** These become block-index operations
  once you store blocks (and you'd drive block boundaries off OSC 133 shell-integration
  markers anyway). The block id, not `StableRowIndex`, is the natural key.
- **Scrollback reflow.** This is the strongest hit to wezterm's edge. If the store holds
  the *logical* (unwrapped) text of each line, a resize re-wraps from your canonical text —
  you stop depending on the emulator to reflow scrollback at all. That neutralizes the
  "alacritty doesn't reflow scrollback" cost flagged above, regardless of crate. (Caveat:
  this covers *scrollback*. The live primary/alt screen still reflows inside the emulator;
  vim/htop are the emulator's problem either way.)

**What the block store does *not* solve:**

- **Live grid → absolute reconciliation.** Drift-free selection is about the
  *moment-to-moment* correspondence between "the physical row the user is dragging over
  right now" and "which absolute line that is," while output streams and the grid churns.
  The store is a write-side capture; it doesn't answer that live query. Anchoring a live
  selection still needs a stable per-line id *and* a correct mapping from alacritty's
  `display_offset`-relative grid onto it.
- **…but here's the pivot:** *populating* the store forces you to mint exactly that id.
  Every line you append gets a monotonic insertion index — and that index **is** a
  hand-rolled `StableRowIndex`. So if kapollo builds deep history regardless, the
  absolute-id machinery this page billed as a *penalty of choosing alacritty* exists
  anyway. It stops being a *differential* cost.

So the deep-history vision **does** weaken the case for wezterm: most of `StableRowIndex`'s
durable-anchor value is delivered better by the block store, crate-agnostically, and the
"you'd reinvent the id yourself" objection loses force because you're minting that id for
the store no matter what.

**The honest counter — why this doesn't fully settle it for alacritty:** with alacritty you
now run **two id spaces** — the emulator's bottom-relative `display_offset` grid and your
store's absolute insertion index — plus the bridge that keeps them in sync under flood,
eviction, scroll-region tricks, and resize. That bridge is exactly the off-by-one-prone
code this page warns about, and the store sits *downstream* of it: if the bridge mis-maps a
streaming line, the store records it under the wrong absolute id, and the error is now
**baked into durable history**, not just a transient highlight. With wezterm you have **one
id space**: `StableRowIndex` can *be* the block store's primary key, shared by emulator and
store, so there's no reconciliation layer to get wrong. Deep history doesn't only argue for
alacritty — it also hands wezterm a way to make the store itself more robust.

**Net for the decision:** deep history shrinks `StableRowIndex`'s *unique* value down to
one thing — **correct, single-id-space live reconciliation** — and makes the rest (save,
marks, reflow) a wash. That's a real point in alacritty's favor: if T022/T027 show the live
bridge holds up tolerably, the durable-history story no longer needs wezterm. The residual
question becomes narrow and concrete:

> Do you want the emulator and your history store to share one battle-tested id
> (**wezterm**), or are you comfortable owning the bridge between two id spaces
> (**alacritty**) — knowing a bridge bug now corrupts *saved history*, not just a transient
> highlight?

Weigh that against the supply-chain cost above. The two framings push opposite ways, which
is why this stays an evidence call for T022/T027 rather than a settled one.

### Where the maintainer currently leans, and why it tilts to wezterm

This shouldn't be read as neutral. Two design intentions, taken together, push the call
toward wezterm:

1. **Deep history is committed, not hypothetical.** The plan is to pull the rich
   in-memory store in *from the start* (persistence can come later). That removes the main
   escape valve for alacritty: the "most anchor value is crate-agnostic" argument only
   holds if deep history stays a maybe. Once it's load-bearing, the residual question
   collapses to the narrow one above — one shared id space vs. owning a bridge between two —
   and a bridge bug there corrupts *durable* history, not just a transient highlight. The
   more committed the feature, the more a single battle-tested id pays off.
2. **Persistence must be privacy-configurable, and the in-memory store makes that
   cleaner.** Persistence has to be off-able per session and toggleable mid-session (a
   slash command, later). Committing to the rich in-memory store from day one makes that
   tractable: capture always lands in memory; persistence becomes a *separate, async,
   likely queued* write off that store, gated by a single switch. Memory-only is then just
   "the persistence sink is disabled" — no second capture path, no divergence between the
   live model and what gets written. The toggle governs one well-defined egress, which is
   exactly what you want when the toggle is a privacy guarantee.

The cost ledger is unchanged by either point: the supply-chain weight (unpublished git-pin,
heavy transitive tree, audit/CI surface) is the same regardless. So the decision sharpens
to an honest one-liner: *a single, library-owned id space for a history feature we're
committed to, against a heavier, unpublished dependency.* That's a far better question to
decide on than rendering vibes — and on those terms wezterm is the current lean.

What T022/T027 still need to answer, even with that lean: **how bad is the alacritty bridge
actually?** Not to overturn the verdict, but because a tractable bridge is the credible
fallback if the wezterm dependency ever goes unmaintained or has to be dropped. Confirming
the escape hatch works is worth the evening even when you've picked the front door.

## What to watch for in T022 / T027

- Start a selection, then `yes`/`seq 1 100000` a flood. Does the highlight stay on the
  same text, or crawl? (Tests the bridge vs. a real anchor.)
- Scroll up into deep scrollback during/after a flood, select, copy. Is the copied text
  the lines you see?
- Force eviction: exceed `SCROLLBACK_LEN` (5000) and repeat the above near the top of
  retained history.
- Note *how hard* any drift would be to fix correctly in each engine — that effort, not
  the spike's pass/fail, is the real signal feeding the recommendation.
- With the [deep-history store](#does-a-deep-history-block-store-change-the-calculus) in
  mind, judge a subtler thing: would a bridge mis-map on alacritty merely flicker the live
  highlight, or would it write the wrong absolute id into a record you intend to *persist*?
  A transient drift is annoying; a corrupted durable anchor is a data bug. That distinction,
  more than raw drift, is what should move the verdict.
