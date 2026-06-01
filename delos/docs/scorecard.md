# Scorecard — Terminal-Grid Spike

Shared rubric for the three stages. Each stage fills one column. Ratings: `++` strong
/ `+` adequate / `~` partial-or-awkward / `-` poor / `n/a`, plus a one-line note.
A criterion a crate cannot satisfy is recorded explicitly — never left blank (SC-001).

| # | Criterion | Weight | S1 vt100 | S2 alacritty_terminal | S3 wezterm-term |
|---|-----------|--------|----------|-----------------------|-----------------|
| 1 | Render correctness (SGR, wide, combining) | high |  |  |  |
| 2 | Grapheme segmentation | high |  |  |  |
| 3 | Scrollback API (cap, eviction, reflow) | high |  |  |  |
| 4 | Selection primitives | high |  |  |  |
| 5 | Mouse/alt-screen ergonomics | high |  |  |  |
| 6 | Hyperlinks (OSC 8) | med |  |  |  |
| 7 | Images (sixel/kitty/iTerm) | low |  |  |  |
| 8 | Damage/dirty tracking | high |  |  |  |
| 9 | API ergonomics in event loop | med |  |  |  |
| 10 | Build/dep weight | med |  |  |  |
| 11 | Maintenance health | med |  |  |  |
| 12 | `/save` reconstruction | med |  |  |  |

Done when every criterion has an entry in all three columns (SC-001).
