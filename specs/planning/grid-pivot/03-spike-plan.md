# 03 — Extended grid spike plan

Status: **draft for review** (pass 2). Nothing here is decided until promoted to
[../brainstorm.md](../brainstorm.md).

Companion to [00-overview.md](00-overview.md) §7, [01-research-grid-and-mouse.md](01-research-grid-and-mouse.md),
and [02-rework-vs-rewrite.md](02-rework-vs-rewrite.md).

## 1. Purpose

Prove the **feel** (grid render + mouse selection + scroll + alt-screen handover)
and **choose the production grid crate** by building the *same* vertical slice on
three crates and scoring them against one rubric. No time box — but each stage is
"done" when the scorecard is fillable, not when it's perfect.

## 2. The vertical slice (built identically on each crate)

A throwaway binary (`spike-<crate>`) that:

1. Spawns a PTY-backed shell (reuse kapollo's `portable-pty` setup).
2. Feeds shell output into the crate's grid/parser.
3. Renders the grid via ratatui (cells → styled spans), main screen only.
4. **Mouse selection** in content coordinates: click-drag selects; selection is
   scoped to the output region; **auto-scrolls** when the drag passes the top/
   bottom edge (both directions); copy to clipboard on release.
5. **Mouse wheel** scrolls scrollback.
6. **Alt-screen handover:** when the child enters the alternate screen
   (`?1049h`), stop owning the grid and pass through so `vi`/`bpytop` work;
   restore cleanly on exit.

That slice exercises every hard part. Block annotation, `/save`, and the input
pad are **out of scope for the spike** — they're additive and don't change crate
selection.

## 3. Selection model (applies to every stage)

Derived from Ken's brainstorm + the GHCP CLI observation (00-overview §6.2, §7.2):

- **Anchor in content coordinates** (scrollback row/col), not screen coords, so
  selection survives scrolling.
- **Pad-scoped:** a selection started in the output region stays there (later:
  same isolation for the input pad).
- **Auto-scroll on drag-past-edge,** both directions, to extend the range.
- **Termination:** second left-click **or** `ESC` → cancel; release / right-click
  → finalize.
- **Copy triggers (state-based to avoid the Ctrl-C/SIGINT clash):**
  - With **no active selection**: right-click → **context menu**; `Ctrl-C` →
    **SIGINT to the child** (unchanged, FR-024).
  - With an **active selection**: right-click → **copy**; `Ctrl-C` → **copy**.
- **Context menu (right-click, no selection):** deferred to the rework. The
  spike only needs a trivial "Hello, World." menu to **prove we can render +
  route** one; the real entries (block output *with* command / block output
  (no command) / current line / advanced range-select) require the OSC 133
  annotation layer and land in the rework.
- **Shift held → bypass** kapollo selection and forward mouse to the child app.
- **Clipboard:** OSC 52 first; note which host terminals honor it.

## 4. Stages (sequential, comparable)

| Stage | Crate | Goal | Expected verdict |
|-------|-------|------|------------------|
| S1 | `vt100` (optionally via `tui-term`) | Fastest path to a working slice; learn the shape of the problem | Likely **spike-only** — too limited for prod (no rich grapheme/hyperlink/image) |
| S2 | `alacritty_terminal` | The "correct, proven" baseline (Zed uses it); real scrollback/damage/selection primitives | Strong prod candidate |
| S3 | `wezterm-term` / `termwiz` | Maximum fidelity: graphemes, hyperlinks, images (sixel/kitty/iTerm) | Prod candidate if the weight/API is worth the extras |

`tui-term` can accelerate S1 (it's a ratatui widget around `vt100`) — use it to
get pixels on screen fast, then decide if the raw `vt100` API is needed.

**Image caveat for S3:** owning the grid means image escapes can't just pass
through to the host terminal untouched — kapollo must detect/forward them
deliberately, or the host protocol must be re-emitted at the right cell. Treat
"can we even forward an image protocol through our grid?" as an explicit S3
question, not an assumed freebie. **Image support is a *cherry*, easy cut** —
Windows Terminal (incl. Preview) doesn't support it today, so it's a stretch
goal at best; never let it sink a crate or burn multiple days.

## 5. Scorecard (fill one column per stage)

| Criterion | Weight | S1 vt100 | S2 alacritty | S3 wezterm |
|-----------|--------|----------|--------------|------------|
| Render correctness (SGR, wide chars, combining) | high | | | |
| Grapheme / Unicode segmentation | high | | | |
| Scrollback API (cap, eviction, reflow on resize) | high | | | |
| Selection primitives (or ease of hand-rolling) | high | | | |
| Mouse routing / alt-screen handover ergonomics | high | | | |
| Hyperlinks (OSC 8) | med | | | |
| Images (sixel/kitty/iTerm) | low | | | |
| Damage/dirty tracking (perf under flood) | high | | | |
| API ergonomics in our event loop | med | | | |
| Binary size / build time / dep weight | med | | | |
| Maintenance health (releases, used-by) | med | | | |
| Text reconstruction for `/save` (D29) | med | | | |

## 6. Host-terminal test matrix

Validate the slice (esp. OSC 52 clipboard, mouse, alt-screen) across:

- **Windows Terminal Preview** (Ken's daily driver) — primary.
- **GNOME Terminal** and **Konsole** on Ubuntu — secondary.
- (macOS: out of scope for the spike — Ken's Mac needs updating.)
- Optional cross-platform: WezTerm, Alacritty, Kitty (interesting because their
  emulator internals are the crates under test).

## 7. Exit criteria

- All three columns of the scorecard filled.
- A short nuts-and-bolts writeup per stage (what surprised us, gotchas).
- A crate recommendation for **prod** with rationale.
- Confirmation that the selection model (§3) and alt-screen handover are
  achievable — or a documented reason they aren't.
- Feed into D25–D30 promotion + the in-place rework spec.

## 8. Open questions for Ken

1. Confirm the **state-based Ctrl-C rule** (§3): copy only while a selection is
   active, else SIGINT. Acceptable?
    - Your take is actually what I meant. `Ctrl-C` as copy IFF selection active.
2. Is the **context menu** wanted in the spike (render+route only), or deferred
   entirely to the rework? I'd lean deferred — prove drag-select + auto-scroll +
   clipboard first; the menu is UI plumbing that doesn't inform crate choice.
    - Deferred; may want simple "Hello, World." context menu to prove that we
      *can* do it in rework.
3. Confirm repo destination for the spike per 00-overview §7.3 (kdelos-public vs
   in-place vs hybrid). I recommend the **hybrid**.
    - See section below. If you concur, that's the plan, if not we iterate;
      either okay!
4. Should image support (S3) be a *gate* (must-have) or a *cherry* (nice-to-have
   that doesn't sink a crate)? I'd make it a cherry.
    - Cherry...I'd like to see this but I don't want to spend multiple days on
      it. It is not currently supported in Windows Terminal/WT Preview, so not
      a *must have*; stretch goal at best. Easy cut.

### Repos/kdelos/etc.

I'm waffling a bit here. I'm starting to think...

- We still finalize `002-mvp-hardening` first. We've done reasonable signoff (didn't rewalk quickstart so mark that task as defered due to rearch plans)
- We do spike directly in this repo in a spike branch (or possibly multiple branches...don't think we need to, one branch with some number of commits will probably work)
- The spike work is placed in the "delos" subdirectory (I like the "birthplace" connection)
- When we get to the end of the spike, we can decide if we want to merge it into "main" kapollo (I'm leaning towards "yes" if we keep all work in the `delos` sub-directory)...I like to show my work if it can help someone else (or me!) in the future.

> **RESOLVED** (see [00-overview.md](00-overview.md) §7.3): no separate repo;
> `delos/` subdirectory on a spike branch, in place; merge to `main` at the end
> if all spike work stays contained in `delos/`. GH Copilot concurs — it's
> cleaner than the earlier hybrid. Open setup detail: make `delos/` a **Cargo
> workspace** of throwaway crates so spike deps never touch the shipping binary.