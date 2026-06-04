# Scorecard — Terminal-Grid Spike

Shared rubric for the three stages. Each stage fills one column. Ratings: `++` strong
/ `+` adequate / `~` partial-or-awkward / `-` poor / `n/a`, plus a one-line note.
A criterion a crate cannot satisfy is recorded explicitly — never left blank (SC-001).

| # | Criterion | Weight | S1 vt100 | S2 alacritty_terminal | S3 wezterm-term |
|---|-----------|--------|----------|-----------------------|-----------------|
| 1 | Render correctness (SGR, wide, combining) | high | `++` faithful via `tui-term` blit of `vt100::Screen` | `+` explicit `Cell` fg/bg/flags → ratatui `Style`; hand-rolled blit | `++` grapheme-aware `visible_cells` + full `CellAttributes`; palette-resolved RGB |
| 2 | Grapheme segmentation | high | `++` handled by vt100 cells + `tui-term` width logic | `+` cells carry grapheme + `WIDE_CHAR` flags, but app must honor them | `++` native cluster iteration with `width()`; wezterm core strength |
| 3 | Scrollback API (cap, eviction, reflow) | high | `~` `set_scrollback`/`scrollback` present but bottom-relative; no absolute row id; no reflow | `+` `scroll_display`/`display_offset` + signed `Line` indices; no app-level absolute id | `++` `StableRowIndex` true absolute id (survives eviction) + `lines_in_phys_range` |
| 4 | Selection primitives | high | `~` `contents_between` extracts text; coords visible-relative, bridged via `coords` helpers; *manual: drifts on scroll, copy off-by-one (`ken`→`ke`)* | `+` native `bounds_to_string`/`selection_to_string`; we reused `coords` bridge; *manual: "Fair" — drifts when window scrolls* | `++` anchor to real absolute ids — drift-free, no `BASE` hack; *manual: "Great" — best of the three, held under flood* |
| 5 | Mouse/alt-screen ergonomics | high | `~` authoritative `alternate_screen()` + `mouse_protocol_mode()` flags, but *manual: `bpytop` rendered scrambled in alt-screen* (vi fine; restores on exit) | `++` `TermMode` bitflags: `ALT_SCREEN` + granular mouse-report modes; *manual: alt-screen Good* | `+` clean `is_alt_screen_active()`/`is_mouse_grabbed()` booleans (less granular); *manual: alt-screen Good* |
| 6 | Hyperlinks (OSC 8) | med | `-` no OSC 8 accessor in 0.16 | `+` `Cell::hyperlink()` carries OSC 8 data (not wired in spike) | `++` first-class `attrs.hyperlink()` carrying the link target |
| 7 | Images (sixel/kitty/iTerm) | low | `n/a` no image support | `n/a` no image support | `+` model-level image cells retained (`image`/blob-leases); pixel render out of scope |
| 8 | Damage/dirty tracking | high | `-` no public damage API; full redraw each frame | `++` `term.damage()`/`reset_damage()` real dirty-span tracking | `++` `get_changed_stable_rows` + per-line seqno, keyed to stable ids |
| 9 | API ergonomics in event loop | med | `++` small obvious surface (`Parser::new/process/screen`) | `~` capable but larger: `Processor`/`Handler`/`EventListener`/`Dimensions`/channel | `+` simple `advance_bytes`; one-method config; rich `Screen`, app owns scroll |
| 10 | Build/dep weight | med | `++` light; `tui-term` thin, shares `ratatui` | `-` 24 transitive deps (rustix-openpty, signal-hook, polling, windows-*) | `-` heaviest tree (image, terminfo, finl_unicode, many `wezterm-*`); git-pin required, ~38s cold build |
| 11 | Maintenance health | med | `~` mature/stable but low recent activity; pinned 0.16 | `++` actively developed (powers Alacritty); 0.26 current | `++` very actively developed, but library crates unpublished → must git-pin a commit |
| 12 | `/save` reconstruction | med | `~` `contents_between` serializes text but lacks absolute line ids | `+` grid + signed scrollback lines + damage; better-anchored than S1 | `++` absolute `StableRowIndex` anchors + seqno damage; strongest reconstruction story |

Done when every criterion has an entry in all three columns (SC-001).

## Manual validation summary

Ground-truth from the per-stage quickstart runs (see `## Manual validation results` in
`s1-vt100.md` / `s2-alacritty.md` / `s3-wezterm.md`). Host: no noticeable difference on
GNOME Terminal vs. the primary terminal.

- **Selection is the decider, and the manual runs confirm it.** S3 (`wezterm-term`) was
  the only engine with drift-free selection — rated "Great" and holding even under a flood;
  S2 (`alacritty_terminal`) and S1 (`vt100`) both drift when the window scrolls (the
  `BASE − scroll` bridge showing through, exactly as [stable-row-index.md](stable-row-index.md)
  predicts). S1 additionally copied off-by-one (`ken` → `ke`).
- **Alt-screen fidelity separated S1 from the rest.** `bpytop` rendered scrambled under
  `vt100` (though `vi` was fine and both restored on exit); S2 and S3 handled alt-screen
  cleanly. This dropped S1's row-5 rating to `~`.
- **Render, copy, SIGINT-vs-copy, shift-bypass, wheel scroll** were Good across all three.
  Child-mouse-mode (#9) was untested on every stage (no suitable test app to hand).
- **Cross-stage UX nits (not engine faults):** a single left click selects one cell on all
  three (production should require an actual drag); and the spike's OSC 8 status-line probe
  overlays the bottom prompt row (cosmetic, expected).

Net: the manual evidence reinforces the `StableRowIndex` thesis — wezterm's selection was
visibly better in exactly the way the rubric predicted, and the gap is real, not theoretical.
