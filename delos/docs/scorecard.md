# Scorecard — Terminal-Grid Spike

Shared rubric for the three stages. Each stage fills one column. Ratings: `++` strong
/ `+` adequate / `~` partial-or-awkward / `-` poor / `n/a`, plus a one-line note.
A criterion a crate cannot satisfy is recorded explicitly — never left blank (SC-001).

| # | Criterion | Weight | S1 vt100 | S2 alacritty_terminal | S3 wezterm-term |
|---|-----------|--------|----------|-----------------------|-----------------|
| 1 | Render correctness (SGR, wide, combining) | high | `++` faithful via `tui-term` blit of `vt100::Screen` | `+` explicit `Cell` fg/bg/flags → ratatui `Style`; hand-rolled blit |  |
| 2 | Grapheme segmentation | high | `++` handled by vt100 cells + `tui-term` width logic | `+` cells carry grapheme + `WIDE_CHAR` flags, but app must honor them |  |
| 3 | Scrollback API (cap, eviction, reflow) | high | `~` `set_scrollback`/`scrollback` present but bottom-relative; no absolute row id; no reflow | `+` `scroll_display`/`display_offset` + signed `Line` indices; no app-level absolute id |  |
| 4 | Selection primitives | high | `~` `contents_between` extracts text; coords visible-relative, bridged via `coords` helpers | `+` native `bounds_to_string`/`selection_to_string`; we reused `coords` bridge |  |
| 5 | Mouse/alt-screen ergonomics | high | `+` authoritative `alternate_screen()` + `mouse_protocol_mode()` flags | `++` `TermMode` bitflags: `ALT_SCREEN` + granular mouse-report modes |  |
| 6 | Hyperlinks (OSC 8) | med | `-` no OSC 8 accessor in 0.16 | `+` `Cell::hyperlink()` carries OSC 8 data (not wired in spike) |  |
| 7 | Images (sixel/kitty/iTerm) | low | `n/a` no image support | `n/a` no image support |  |
| 8 | Damage/dirty tracking | high | `-` no public damage API; full redraw each frame | `++` `term.damage()`/`reset_damage()` real dirty-span tracking |  |
| 9 | API ergonomics in event loop | med | `++` small obvious surface (`Parser::new/process/screen`) | `~` capable but larger: `Processor`/`Handler`/`EventListener`/`Dimensions`/channel |  |
| 10 | Build/dep weight | med | `++` light; `tui-term` thin, shares `ratatui` | `-` 24 transitive deps (rustix-openpty, signal-hook, polling, windows-*) |  |
| 11 | Maintenance health | med | `~` mature/stable but low recent activity; pinned 0.16 | `++` actively developed (powers Alacritty); 0.26 current |  |
| 12 | `/save` reconstruction | med | `~` `contents_between` serializes text but lacks absolute line ids | `+` grid + signed scrollback lines + damage; better-anchored than S1 |  |

Done when every criterion has an entry in all three columns (SC-001).
