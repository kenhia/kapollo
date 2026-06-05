# Quickstart — Manual Validation (Grid Rework)

Interactive validation script for the grid rework, mapped to the spec's success criteria.
Automated tests cover the pure helpers and contracts; this script covers the *feel* and the
TUI-integrity criteria that only a live TTY can confirm (Constitution III integration
exception, the 003 spike pattern).

## Prereqs

```fish
# fish shell (this repo's working shell). Build the binary:
cargo build
# Run inside a real terminal (not a captured pipe):
./target/debug/kap
```

A shell with OSC 133/7 integration hooks installed (the repo's rcfile snippet) gives block
boundaries; without it, the sentinel fallback still segments blocks.

## Walkthrough → success criteria

| # | Action | Expect | Criterion |
|---|--------|--------|-----------|
| 1 | Run `seq 1 5000` | Renders as a single fast-scrolling stream; no flicker, no dropped lines | SC-003 |
| 2 | Run a `\r` progress (e.g. a curl download or `for i in (seq 100); printf "\r%d%%" $i; sleep 0.01; end`) | One line updates in place, not 100 lines | SC-001 |
| 3 | Run `ls --color=auto` / a colored build | Colors + bold/underline render correctly | SC-002 |
| 4 | Scroll wheel up after lots of output | Viewport scrolls into history; new output doesn't yank you to bottom while scrolled | SC-006 |
| 5 | Click-drag across several lines of output | Highlight tracks the drag in real time | SC-004 |
| 6 | While selected, run another command | Selection clears on submit (no stale highlight) | SC-004 / FR-017 |
| 7 | Drag a flood (`seq 1 5000` then immediately select) | Selection does **not** drift as output scrolls | SC-008 |
| 8 | Select text, copy (release/hotkey), paste elsewhere | Pasted text matches selection exactly; no off-by-one | SC-007 |
| 9 | Copy over SSH session | OSC 52 carries the copy to your local clipboard | SC-007 |
| 10 | Open `htop` / `vim` / `bpytop` (alt-screen + mouse app) | App's own mouse works; kapollo selection suspended; on quit, prior scrollback intact | SC-005 |
| 11 | Hold **Shift** and drag | Host terminal's native selection engages (kapollo bypasses) | FR-016 |
| 12 | Set `[clipboard] osc52 = false` and `local_fallback = false` in config, then copy a selection | Visible "copy failed" notice — never a silent drop | FR-013 |
| 13 | Exit via `/quit` (and, if a panic ever fires, the panic guard runs) | Terminal restored: cooked mode, mouse capture off, alternate screen exited, cursor visible | SC-009 / FR-027 |
| ~~14~~ | ~~Run a command, `/save <file>`~~ | **Deferred** — `/save` not yet implemented (kwi WI #43) | SC-007 / FR-019 |
| ~~15~~ | ~~`/save` an older still-retained block after eviction~~ | **Deferred** — depends on `/save` (kwi WI #43); store-outlives-eviction is covered by `tests/block_store_seam.rs` | FR-025 / R3 |

## Pass criteria

All rows behave as in **Expect**. Any flicker, dropped output, selection drift, off-by-one
copy, stuck mouse capture, or un-restored terminal is a **blocker** (Constitution VI). Record
results in the PR description, mirroring the 003 spike's manual-validation table.

## Notes

- Steps 1–3 exercise the grid render contract; 4–13 the mouse/selection/clipboard contract.
  Steps 14–15 (block-store via `/save`) are deferred with `/save` itself (kwi WI #43); the
  store/seam they would have exercised is covered by `tests/block_store_seam.rs`.
- Step 13: `Ctrl-C` is intentionally captured by kapollo (copies a selection, else forwards
  SIGINT to the child), so it is not a quit path — use `/quit`. The panic guard is the
  crash-path restore; hitting it in normal use would itself be a bug.
- The 003 spike's flood-overrun caveat is specifically retested at steps 6–7 (FR-017 should
  make it a non-issue).
