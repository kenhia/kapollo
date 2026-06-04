# S3 — `wezterm-term`

Nuts-and-bolts writeup for the `wezterm-term` stage. Unlike S1/S2 this crate is **not
published to crates.io** (docs.rs 404s), so it must be consumed as a git dependency.

## Approach

- **Git pin used**: `https://github.com/wezterm/wezterm.git`,
  `rev = "577474d89ee61aef4a48145cdec82a638d874751"` (a recent `main` commit). wezterm
  uses date-stamped *application* tags, not semver crate releases, and none of the
  `wezterm-*` library crates are on crates.io — so pinning a `main` commit is the
  pragmatic "latest green" choice. Recorded here per FR.
- Same vertical slice as S1/S2: PTY-backed shell → emulator → ratatui `Buffer`, with
  content-coordinate selection, scrollback, alt-screen handover, explicit copy. The
  `selection.rs` state machine is byte-for-byte identical across all three slices (same
  8 tests).
- Construction: `Terminal::new(TerminalSize, Arc<dyn TerminalConfiguration>, term_program,
  term_version, Box<dyn Write + Send>)`. Bytes are fed with `term.advance_bytes(&chunk)`.
- **Answerback path**: the emulator's device replies (DSR, DA, etc.) are written into the
  `Box<dyn Write>` we hand it. We pass a `ChannelWriter` that forwards those bytes over an
  `mpsc` channel; the main loop drains it into the PTY — the same role `EventProxy`'s
  `PtyWrite` played in S2.
- `TerminalConfiguration` has exactly one required method, `color_palette()`; everything
  else (incl. `scrollback_size()`, which we bump to 5000) has trait defaults. Nice.

## The coordinate win — `StableRowIndex`

This is the headline difference. `wezterm-term` does **not** track a display-scroll
position itself (the embedding app owns `scroll_offset`), but in exchange it exposes a
**true absolute row id**: `StableRowIndex`. It counts from the top of all history, grows
downward, and *survives scrollback eviction*. (Why this matters, and what its absence
costs S1/S2: [stable-row-index.md](stable-row-index.md).)

- S1 (`vt100`) and S2 (`alacritty_terminal`) both expose only a **bottom-relative**
  scroll position, so we faked absolute ids with the `top_row = BASE - scroll` hack.
- In S3 `top_row` is simply `screen.phys_to_stable_row_index(visible_top_phys)` — the
  *real* absolute id. No `BASE` constant, no fakery. A selection anchored to stable ids
  cannot drift as output streams in. This is exactly the property a durable `/save` and a
  scroll-stable selection model want.

```text
len      = screen.scrollback_rows()                  // total lines incl. history
top_phys = len - physical_rows - scroll_offset       // we own scroll_offset
top_row  = screen.phys_to_stable_row_index(top_phys) // absolute, drift-free
lines    = screen.lines_in_phys_range(top_phys .. top_phys + physical_rows)
```

## What worked

- **Compiled on the first real build** with zero API mismatches after the research pass —
  the only diagnostic was one unused parameter. Rare for an unfamiliar, unpublished crate.
- `Line::visible_cells()` yields grapheme clusters with `cell_index()` / `str()` /
  `width()` / `attrs()` — grapheme + wide-char segmentation is *native*, not something the
  app reconstructs. Best fidelity of the three engines.
- `CellAttributes` is rich and ergonomic: `intensity()`, `italic()`, `underline()`,
  `reverse()`, plus `foreground()`/`background()` resolved to RGB through
  `ColorPalette::resolve_fg/resolve_bg` → `SrgbaTuple::to_srgb_u8()`. OSC 8 hyperlinks are
  first-class via `attrs.hyperlink()` (not wired in the spike, but present).
- Mode introspection is clean: `term.is_alt_screen_active()` and `term.is_mouse_grabbed()`
  booleans corroborate our `detect_mode` routing.
- Damage tracking is real and *keyed to stable ids* (`get_changed_stable_rows` + per-line
  seqno) — the natural partner to incremental redraw and `/save`.

## What fought back

- **Not on crates.io.** Must git-pin a commit; no semver, no changelog discipline for the
  library crates. Reproducibility rests entirely on the pinned `rev` + committed
  `Cargo.lock`.
- **Heaviest dependency tree by far** (~hundreds of crates): pulls in `image`, `terminfo`,
  `finl_unicode`, `vtparse`, and a dozen-plus `wezterm-*` sub-crates
  (`wezterm-surface`, `wezterm-cell`, `wezterm-bidi`, `wezterm-char-props`,
  `wezterm-escape-parser`, `wezterm-blob-leases`, …). Cold dep build ≈ 38s here.
- The app must own the scroll position (no `display_offset` equivalent). That's a minor
  cost and arguably cleaner given `StableRowIndex`, but it is more wiring than S1/S2.
- Width/wide-cell handling is on the app: `visible_cells` gives the start `cell_index` and
  a `width`; the spike paints the symbol at the start column and leaves spacer columns —
  fine for the slice, but real wide/combining correctness needs care.

## Cherry probe — image protocols (T034, FR-028, time-boxed)

Distinct from S1/S2 (`n/a`): wezterm's model **retains image cells**. `wezterm-cell`
depends on `image` + `wezterm-blob-leases` and the escape parser understands sixel / iTerm
/ kitty image escapes, attaching image data to cells. So at the *model* level image
content is preserved and addressable — a genuine capability the other two lack. Actually
*rendering* pixels into a TUI buffer is out of scope (and not meaningfully possible in a
cell grid), so the cherry is recorded as "model-level support present; pixel rendering out
of scope" rather than a working demo. It did not gate the slice.

## Scorecard notes (S3 column)

- Render correctness: `++` grapheme-aware `visible_cells` + full `CellAttributes` → ratatui `Style`; palette-resolved RGB.
- Grapheme segmentation: `++` native cluster iteration with width; wezterm's core strength.
- Scrollback API: `++` `StableRowIndex` true absolute row id (survives eviction) + `lines_in_phys_range`; best of the three.
- Selection primitives: `++` anchor to real absolute ids — drift-free, no `BASE` hack.
- Mouse/alt-screen ergonomics: `+` clean `is_alt_screen_active()`/`is_mouse_grabbed()` booleans (less granular than alacritty bitflags).
- Hyperlinks (OSC 8): `++` first-class `attrs.hyperlink()` carrying the link target.
- Images: `+` model-level image cells retained (`image`/blob-leases); pixel render out of scope (vs `n/a` for S1/S2).
- Damage/dirty tracking: `++` `get_changed_stable_rows` + per-line seqno, keyed to stable ids.
- API ergonomics: `+` simple `advance_bytes`; one-method config; rich `Screen` API, app owns scroll.
- Build/dep weight: `-` heaviest tree (image, terminfo, finl_unicode, many `wezterm-*`); git-pin required, ~38s cold dep build.
- Maintenance health: `++` very actively developed, but library crates unpublished → must git-pin a commit.
- `/save` reconstruction: `++` absolute `StableRowIndex` anchors + seqno damage; strongest reconstruction story.

## Manual validation results

Maps to the acceptance scenarios and success criteria.

1. **Render (SC-001 / FR-007)**: Good
2. **Selection + content coords (SC-004 / FR-008/009)**: Great
3. **Auto-scroll on drag-past-edge (FR-010)**: Good
4. **Copy (SC-008 / FR-011/016)**: Good
5. **SIGINT vs copy (SC-008 / FR-015)**: Good
6. **Shift bypass (FR-017)**: Good
7. **Wheel scroll (FR-012)**: Good
8. **Alt-screen handover (SC-005 / FR-013)**: Good
9. **Child mouse mode (FR-014)**: Not tested
10. **Flood (FR-022 #8)**: Good

### Notes

Due to our hyperlink test, I couldn't see the "usable prompt" once the prompt
was at the bottom of the screen. Doing `echo '---...'` with enough dashes
worked, prompt still there, hidden by hyperlink test.

Noticed in all three, single left click selects a single cell. Not an issue for
spike, but ideally should not select unless there is a "drag"

- (9): I don't know an app that I can use for test
- (10): Responsive, different drift...did a "copy" (both right-click and ctrl-c)
  and got a range of `y`'s instead of original selection...may have overrun
  buffer. Still calling this one "Good" as selection in general was better *and*
  my thinking is that in the production `kapollo` app, submitting a command
  should clear selection.
