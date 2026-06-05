# Research — Grid Rework (Phase 0)

Consolidated decisions for the 004 grid rework. Most unknowns were resolved upstream by the
grid-pivot planning effort (decisions **D25–D30**, see
[02-rework-vs-rewrite.md](../planning/grid-pivot/02-rework-vs-rewrite.md) §6) and the 003
spike ([recommendation.md](../../delos/docs/recommendation.md),
[stable-row-index.md](../../delos/docs/stable-row-index.md)). This file records each
decision in the SDD format (Decision / Rationale / Alternatives).

## R1 — Terminal-emulation engine

**Decision**: Use `wezterm-term`, git-pinned at
`rev = 577474d89ee61aef4a48145cdec82a638d874751`, with `alacritty_terminal 0.26` named as
the fallback (D27).

**Rationale**: The 003 spike scored all three engines on a weighted rubric; `wezterm-term`
won on every high-weight axis (high-weight total +11 vs alacritty +8 vs vt100 +4). The
deciding factor is `StableRowIndex` — a true absolute, eviction-proof row id — which gives
drift-free selection and a durable block/`/save` anchor with a single id space. Manual
validation confirmed wezterm's selection held under flood where the others drifted. The
planned in-memory block store and future deep-history make the single-id-space advantage
compounding (see [stable-row-index.md](../../delos/docs/stable-row-index.md)).

**Alternatives considered**:
- `alacritty_terminal` (published, lighter ~24-dep tree, semver) — strong runner-up; kept
  as the explicit fallback if the unpublished git dependency proves untenable. Its gap is
  the absence of an app-level absolute row id, forcing a hand-rolled `BASE − scroll` bridge.
- `vt100`/`tui-term` — lightest, but `~`/`-` on scrollback, selection, damage, OSC 8; out.

**Cost accepted**: `wezterm-term` is **not on crates.io**; reproducibility rests on the
pinned `rev` + committed `Cargo.lock`. Heaviest dependency tree (`image`, `terminfo`,
`finl_unicode`, many `wezterm-*`); ~38s cold dep build. Accepted per the spike's documented
trade.

## R2 — Dependency version bumps

**Decision**: Bump `ratatui` 0.29 → **0.30**, `crossterm` 0.28 → **0.29**, `portable-pty`
0.8 → **0.9** to match the spike-proven versions; add `base64 0.22` and `arboard 3.6`.

**Rationale**: The spike built the full slice against these versions; aligning avoids a
second integration pass and keeps the promoted `spike-support`/`selection.rs` code
compiling unchanged. `wezterm-term` pulls `termwiz`; verify no version conflict against
ratatui's `unicode-width`/`bitflags` during the dependency-add task.

**Alternatives considered**: Stay on current versions and back-port — rejected; needless
friction porting spike code that already targets the newer APIs.

## R3 — Block fidelity: store vs reconstruct (supersedes D29)

**Decision**: The **in-memory block store retains each block's output text** as the
canonical source for `/save`/`/filter` (option (b) from
[02-rework-vs-rewrite.md](../planning/grid-pivot/02-rework-vs-rewrite.md) §4). This
**supersedes D29's** v1 "reconstruct from grid rows" lean. Access is via a single accessor
(`block.text()` / `block.text_with_command()`) so the backing can change without touching
callers; a future database backing is added as a *secondary* store behind the same accessor.

**Rationale**: The user committed to pulling the rich store in from the start (it is the
foundation for the planned deep-history and privacy-toggleable persistence). Reconstruction
is lossy for rows evicted past the scrollback cap and depends on the engine's grapheme
handling; a retained store is byte/text-faithful and makes `/save` exact. Crucially,
populating the store mints the per-block text once, and the single accessor satisfies
SC-010 (DB backing requires no caller changes). The existing `OutputBuffer` (ringbuf)
already retains bounded output bytes per block — the store is its evolution, not new memory
machinery.

**Alternatives considered**:
- Reconstruct-from-grid (D29 v1 lean) — rejected: lossy past eviction, couples `/save` to
  render fidelity, and would be re-replaced within ~1 sprint once deep-history lands.
- Dual canonical sources (grid + bytes, both authoritative) — rejected: two id spaces and a
  reconciliation bridge that can corrupt durable history (the exact risk
  [stable-row-index.md](../../delos/docs/stable-row-index.md) warns about). With
  `StableRowIndex` the store can key off one id space.

**Tradeoff recorded**: ~2× scrollback-scale memory for retained text; trivial at MVP caps
and explicitly acceptable per the deep-history analysis. Persistence is **out of scope**
here — in-memory only — but the accessor + secondary-store seam anticipates it.

## R4 — Clipboard strategy

**Decision**: OSC 52 primary (terminal-mediated, works over SSH) with an `arboard` local
fallback; fallback order configurable. On total failure, surface a visible notice
(FR-013), never a silent drop.

**Rationale**: OSC 52 is the spike-validated path and is SSH-friendly (kapollo is often run
remotely). `arboard` covers hosts/terminals that don't honor OSC 52. The spike implemented
and unit-tested both (`spike-support::clipboard`: `osc52_frame`, `copy_local`).

**Alternatives considered**: arboard-only (breaks over SSH); OSC 52-only (breaks on
terminals that don't honor it). Both rejected in favor of primary+fallback.

## R5 — Mouse routing & alt-screen hand-over

**Decision**: A routing layer keyed on (a) alt-screen active and (b) child mouse-mode
enabled. When either is true, forward mouse/relevant input to the child and suspend
kapollo's selection/scroll; otherwise kapollo owns selection + wheel scroll. Shift always
bypasses to the host terminal's native selection (FR-016). Detection uses the
`spike-support::modes` `detect_mode` tap on the output stream (`?1049h/l`, `?1000/1002/
1003/1006 h/l`), corroborated by the engine's `is_alt_screen_active()`/`is_mouse_grabbed()`.

**Rationale**: Proven in all three spike slices (SC-005 confirmed). Keeping the detection in
the output side-tap (not only the engine) preserves the existing OSC 133/7 plumbing and
gives a single place to route.

**Alternatives considered**: Rely solely on the engine's mode flags — workable on wezterm,
but the side-tap is already needed for OSC 133/7 block marks, so one detector serves both.

## R6 — Content-stable row anchoring for selection & blocks

**Decision**: Anchor selections and block row-ranges to the engine's `StableRowIndex`
(absolute, eviction-proof). The app owns `scroll_offset`; `top_row` is a real stable id
(`phys_to_stable_row_index`), no `BASE − scroll` bridge.

**Rationale**: This is the spike's headline win and the reason wezterm was chosen. It makes
FR-008 (selection does not drift) and FR-018/FR-025 (block ranges survive scroll/eviction
unambiguously) direct properties of the engine rather than hand-rolled invariants.

**Alternatives considered**: The `coords` `BASE − scroll` bridge (needed on vt100/alacritty)
— rejected here since wezterm provides the real id; the bridge code is retained only as the
content↔screen *pixel* mapping, not as a fake absolute id.

## R7 — Block boundaries: shell integration marks, not the grid

**Decision**: Keep deriving block boundaries from OSC 133 `A/B/C/D` marks (and OSC 7 cwd)
emitted by the shell rcfile hooks, tapped from the output stream — **not** from grid
heuristics. The grid supplies the row ranges a block *occupies*; the marks supply *where a
block starts/ends*.

**Rationale**: The existing `pty/shell.rs` hooks + `output/sentinel.rs` fallback already do
this and carry over (reuse map: "keep"). The grid is additive (D29) — boundaries are a
shell-integration concern, orthogonal to cell rendering.

**Alternatives considered**: Infer boundaries from prompt-row detection on the grid —
rejected: fragile, and we already have authoritative marks.

## R8 — In-place rework vs rewrite (Path confirmation)

**Decision**: In-place rework (Path A), on the `004-grid-rework` branch.

**Rationale**: The 003 spike confirmed the selection + alt-screen model drops onto a real
emulator grid without a rewrite; ~50–60% of the existing crate (PTY, config, slash, input
router, chrome, hooks) is reusable as-is with its tests, and the rewritten ~40–50% is
exactly the core either path would rewrite. Path B's clean-slate benefit is small because
the grid module is new code inside the existing crate either way (doc 02 §5).

**Alternatives considered**: Path B (start over) — rejected; would re-port working,
well-tested layers for little benefit and discard green tests + history.

## Resolved unknowns summary

| Unknown | Resolution |
|---------|------------|
| Which engine? | `wezterm-term` git-pinned; alacritty fallback (R1) |
| Dep versions? | ratatui 0.30 / crossterm 0.29 / portable-pty 0.9 + base64 + arboard (R2) |
| `/save` fidelity? | Retained block-store text; supersedes D29 reconstruction (R3) |
| Clipboard? | OSC 52 + arboard fallback, configurable, visible-failure (R4) |
| Mouse/alt-screen? | Side-tap mode detection + engine flags; Shift bypass (R5) |
| Selection drift? | `StableRowIndex` absolute anchoring (R6) |
| Block boundaries? | OSC 133/7 shell marks (kept), grid supplies ranges (R7) |
| Rework vs rewrite? | In-place Path A (R8) |

No `NEEDS CLARIFICATION` remain.
