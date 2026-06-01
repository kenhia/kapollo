# Recommendation — Terminal-Grid Spike

Final synthesis. Written after all three stages are scored. Produces a single
production-crate recommendation with rationale grounded in the weighted rubric
(FR-024 of the 003 spec), feeding decisions D25–D30 and the rework spec.

## Recommendation

**`wezterm-term`** (git-pinned), with **`alacritty_terminal`** named as the explicit
fallback if the supply-chain cost of an unpublished crate proves unacceptable.

All three engines cleared the bar: each one supported the content-coordinate selection
model and the alt-screen handover (see *Feasibility* below). The decision therefore turns
on the **high-weight** criteria — render correctness, grapheme segmentation, scrollback,
selection primitives, mouse/alt-screen, and damage tracking — where the ranking is
unambiguous.

## Weighted rationale

Scoring only the six high-weight criteria (`++`=2, `+`=1, `~`=0, `-`=−1):

| Engine | 1 Render | 2 Grapheme | 3 Scrollback | 4 Selection | 5 Mouse/alt | 8 Damage | High-weight total |
|--------|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| S1 `vt100` | ++ | ++ | ~ | ~ | + | − | **+4** |
| S2 `alacritty_terminal` | + | + | + | + | ++ | ++ | **+8** |
| S3 `wezterm-term` | ++ | ++ | ++ | ++ | + | ++ | **+11** |

- **`wezterm-term` wins on every high-weight axis that matters to kapollo's roadmap.** Its
  decisive advantage is **`StableRowIndex`** — a *true absolute* row id that survives
  scrollback eviction (deep-dive: [stable-row-index.md](stable-row-index.md)). S1 and S2 only expose a bottom-relative scroll position, forcing the
  `top_row = BASE − scroll` fiction; wezterm hands us the real absolute id directly. That
  property is exactly what a **drift-free selection** and a **durable `/save`** want, and it
  is the highest-leverage differentiator in the whole rubric. Grapheme/wide-char
  segmentation is native (`Line::visible_cells`), OSC 8 hyperlinks are first-class, and
  damage tracking is keyed to stable ids.
- **`alacritty_terminal` is the strong, lower-risk runner-up (+8).** It delivers real damage
  tracking, the most granular mouse/alt-screen mode flags, and — crucially — it is
  **published to crates.io** with semver discipline and a far lighter dependency tree
  (~24 transitive crates vs. wezterm's hundreds, incl. `image`/`terminfo`). If the git-pin
  and build weight of wezterm become a liability, alacritty buys ~80% of the benefit with
  materially less supply-chain exposure. Its one structural gap vs. wezterm is the absence
  of an app-level absolute row id.
- **`vt100` is out (+4).** Lightest and simplest, but `~`/`-` on scrollback, selection,
  damage, and OSC 8 — the exact areas kapollo needs to be strong. Good for a quick
  read-only mirror, not for the durable interactive grid.

## Feasibility (SC-004 / SC-005)

- **Content-coordinate selection model — CONFIRMED achievable** on all three engines via the
  shared `spike-support::coords` bridge and the identical `selection.rs` state machine
  (8 passing tests, byte-for-byte portable across slices). It is *cleanest* on
  `wezterm-term`, where `top_row` is a real `StableRowIndex` and needs no `BASE` offset.
- **Alt-screen handover — CONFIRMED achievable** on all three: `detect_mode` on the output
  stream drives routing, corroborated by each engine's authoritative state
  (`vt100::alternate_screen()`, alacritty `TermMode` bitflags, wezterm
  `is_alt_screen_active()`/`is_mouse_grabbed()`). Full-screen apps (vim, less, htop) receive
  raw key/mouse input while the host owns selection only on the normal screen.

## Risks / caveats

- **Supply chain (primary risk).** `wezterm-term` and its `wezterm-*` siblings are **not on
  crates.io**; the build depends on a pinned `rev`
  (`577474d89ee61aef4a48145cdec82a638d874751`) plus a committed `Cargo.lock`. No semver, no
  changelog cadence. Mitigation: pin precisely, vendor if needed, and keep the
  `alacritty_terminal` fallback warm.
- **Build/dep weight.** Hundreds of transitive crates (`image`, `terminfo`, `finl_unicode`,
  many `wezterm-*`); ~38s cold dep build here. Heavier CI and a larger audit surface.
- **App owns scroll position.** wezterm tracks no `display_offset`; the host maintains
  `scroll_offset` itself. Minor, and arguably cleaner given `StableRowIndex`.
- All slices are **throwaway** (`delos/` is excluded from the shipping graph; isolation
  reverified — zero spike deps reached `kapollo`/`kap`). The production integration is a
  fresh, narrowly-scoped dependency add in the rework, not a lift of this spike code.

## Feeds into

- Decisions D25–D30 (see [02-rework-vs-rewrite.md](../../specs/planning/grid-pivot/02-rework-vs-rewrite.md)):
  the spike confirms an **in-place rework is viable** — the selection + alt-screen model
  drops onto a real emulator grid without a rewrite — and supplies the crate choice
  (`wezterm-term`, fallback `alacritty_terminal`) those decisions were waiting on (FR-027).
- The **rework spec**: promote `spike-support`'s coordinate/mode/clipboard helpers and the
  `selection.rs` state machine as the design seed; adopt `StableRowIndex` as the content-row
  anchor for selection and `/save`.
