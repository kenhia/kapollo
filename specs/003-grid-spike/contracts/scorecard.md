# Contract: Scorecard Rubric

**Feature**: 003-grid-spike | Artifact: `delos/docs/scorecard.md`

The single shared rubric. Each stage fills one column. Ratings are a short symbol
plus a one-line note. Suggested rating scale: `++` strong / `+` adequate / `~`
partial-or-awkward / `-` poor / `n/a`. Weights guide the final synthesis (FR-024).

## Criteria (12, fixed — from spec FR-022)

| # | Criterion | Weight | What to record |
|---|-----------|--------|----------------|
| 1 | Render correctness (SGR, wide chars, combining) | high | fidelity of colors/attrs, CJK width, combining marks |
| 2 | Grapheme / Unicode segmentation | high | does the crate segment graphemes or hand back raw bytes/chars |
| 3 | Scrollback API (cap, eviction, reflow on resize) | high | is there real scrollback; reflow on width change |
| 4 | Selection primitives (or ease of hand-rolling) | high | built-in selection, or how hard our content-coord selection was |
| 5 | Mouse routing / alt-screen handover ergonomics | high | how cleanly alt-screen + child mouse modes integrate |
| 6 | Hyperlinks (OSC 8) | med | parsed/exposed? |
| 7 | Images (sixel/kitty/iTerm) | low | forwardable through an owned grid? (cherry, FR-028) |
| 8 | Damage / dirty tracking (perf under flood) | high | does it expose damage; responsiveness under flood |
| 9 | API ergonomics in the event loop | med | how naturally it fits our render/select loop |
| 10 | Binary size / build time / dep weight | med | compile time, transitive tree size, binary size |
| 11 | Maintenance health | med | release cadence, used-by, issues |
| 12 | Text reconstruction for `/save` | med | can we reconstruct block text from grid rows; lossiness |

## Template

```markdown
| # | Criterion | Weight | S1 vt100 | S2 alacritty_terminal | S3 wezterm-term |
|---|-----------|--------|----------|-----------------------|-----------------|
| 1 | Render correctness | high |  |  |  |
| 2 | Grapheme segmentation | high |  |  |  |
| 3 | Scrollback API | high |  |  |  |
| 4 | Selection primitives | high |  |  |  |
| 5 | Mouse/alt-screen ergonomics | high |  |  |  |
| 6 | Hyperlinks (OSC 8) | med |  |  |  |
| 7 | Images | low |  |  |  |
| 8 | Damage/dirty tracking | high |  |  |  |
| 9 | API ergonomics | med |  |  |  |
| 10 | Build/dep weight | med |  |  |  |
| 11 | Maintenance health | med |  |  |  |
| 12 | `/save` reconstruction | med |  |  |  |
```

## Completion (SC-001)

The scorecard is "done" when **every** criterion has an entry in **all three**
columns. A criterion the crate cannot satisfy is recorded explicitly (e.g. `-` or
`n/a` with a note), not left blank.
