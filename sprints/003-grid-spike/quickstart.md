# Quickstart: Terminal-Grid Spike

**Feature**: 003-grid-spike | **Date**: 2026-06-01

How to stand up the `delos/` workspace, run each stage, and execute the manual
validation script. The interactive behavior is validated by hand (see plan.md
Constitution Check — TDD is relaxed for the slice; only pure helpers are unit-tested).

> Shell note: this repo's environment is **fish**. Use `set VAR value` (not
> `VAR=value`) and `$status` (not `$?`).

---

## 1. Create the isolated workspace

`delos/` is its **own** Cargo workspace, excluded from the root crate so spike deps
never enter kapollo's graph (FR-003).

1. In the **root** `Cargo.toml`, add the exclude so `cargo build` at repo root skips it:

   ```toml
   [workspace]
   exclude = ["delos"]
   ```

   (If the root has no `[workspace]` table, add one with just this key. Verify the
   root build is unaffected: `cargo build` and `cargo test` at the repo root still
   pass and `cargo tree` shows no `vt100`/`alacritty_terminal`/`wezterm-term`.)

2. Create `delos/Cargo.toml` as a standalone workspace:

   ```toml
   [workspace]
   resolver = "2"
   members = ["spike-support", "spike-vt100", "spike-alacritty", "spike-wezterm"]

   [workspace.dependencies]
   ratatui = "0.30"
   crossterm = "0.29"
   portable-pty = "0.9"
   arboard = "3.6"
   ```

3. Scaffold the crates:

   ```fish
   cd delos
   cargo new --lib spike-support
   cargo new --bin spike-vt100
   cargo new --bin spike-alacritty
   cargo new --bin spike-wezterm
   ```

---

## 2. Per-stage dependencies

Add to each binary's `Cargo.toml` (workspace deps inherited with `.workspace = true`):

```toml
# spike-vt100/Cargo.toml
[dependencies]
spike-support = { path = "../spike-support" }
ratatui.workspace = true
crossterm.workspace = true
vt100 = "0.16"
tui-term = "0.3"   # optional accelerator (R6)

# spike-alacritty/Cargo.toml
[dependencies]
spike-support = { path = "../spike-support" }
ratatui.workspace = true
crossterm.workspace = true
alacritty_terminal = "0.26"

# spike-wezterm/Cargo.toml  (wezterm-term is NOT on crates.io — git pin, R1)
[dependencies]
spike-support = { path = "../spike-support" }
ratatui.workspace = true
crossterm.workspace = true
wezterm-term = { git = "https://github.com/wezterm/wezterm.git", rev = "<PIN_AT_S3_START>" }
```

`spike-support/Cargo.toml` depends on `portable-pty.workspace = true` and
`arboard.workspace = true`.

---

## 3. Run a stage

```fish
cd delos
cargo run -p spike-vt100            # S1
cargo run -p spike-alacritty        # S2
cargo run -p spike-wezterm          # S3
```

Optional clipboard fallback evaluation:

```fish
cargo run -p spike-vt100 -- --clipboard=arboard
```

---

## 4. Manual validation script (run identically per stage)

Maps to the acceptance scenarios and success criteria.

1. **Render (SC-001 / FR-007)**: run an interactive shell; run `ls --color`,
   `printf '\e[1;31mbold red\e[0m\n'`, a wide-char line (e.g. `echo 日本語`).
   → output renders as a styled grid; colors/attrs/width correct.
2. **Selection + content coords (SC-004 / FR-008/009)**: click-drag across output;
   confirm highlight tracks the drag and is scoped to the output region. Scroll
   away with the wheel and back; confirm the selection did not shift.
3. **Auto-scroll on drag-past-edge (FR-010)**: produce > 1 screen of output; start a
   drag and pull below the bottom edge → view scrolls down, range extends. Repeat
   pulling above the top edge → scrolls up.
4. **Copy (SC-008 / FR-011/016)**: with an active selection, right-click → paste
   elsewhere to confirm clipboard contents. Repeat with `Ctrl-C` (active selection).
5. **SIGINT vs copy (SC-008 / FR-015)**: with **no** selection, run `sleep 30`, press
   `Ctrl-C` → the child is interrupted (not a copy). With no selection, right-click →
   the "Hello, World." menu appears (FR-019).
6. **Shift bypass (FR-017)**: hold Shift and drag → the host terminal's native
   selection (or the child) gets the mouse, not kapollo's selection.
7. **Wheel scroll (FR-012)**: wheel up/down scrolls scrollback.
8. **Alt-screen handover (SC-005 / FR-013)**: launch `vi` (or `bpytop`); confirm the
   app takes the full screen and works (including its own mouse if any), then `:q`
   → the slice restores its own grid handling cleanly.
9. **Child mouse mode (FR-014)**: in an app that requests mouse reporting on the main
   screen, confirm clicks reach the app, not kapollo selection.
10. **Flood (FR-022 #8)**: `yes | head -n 100000` or `cat largefile`; note whether
    render/selection stay responsive; record damage/dirty-tracking behavior.

Record results in `delos/docs/s{1,2,3}-*.md` and fill the column in
`delos/docs/scorecard.md`.

---

## 5. Host-terminal matrix (SC-006 / FR-026)

Run at least the render + selection + copy + alt-screen steps in each:

| Terminal | Required | OSC 52 honored? | Notes |
|----------|----------|-----------------|-------|
| Windows Terminal Preview | **primary** |  |  |
| GNOME Terminal (Ubuntu) | secondary (one of) |  |  |
| Konsole (Ubuntu) | secondary (one of) |  |  |
| WezTerm / Alacritty / Kitty | optional |  |  |

macOS is out of scope.

---

## 6. Isolation check (SC-007) — run after each stage builds

```fish
# at repo ROOT (not in delos/):
cargo tree | grep -E 'vt100|alacritty_terminal|wezterm-term|termwiz'   # expect: no matches
cargo build ; and cargo test                                            # shipping crate still green
```

If any spike crate shows up in the root `cargo tree`, the isolation is broken — fix
the root `exclude`/dependency before continuing.

---

## 7. Deliverables checklist (exit criteria)

- [ ] `delos/docs/scorecard.md` — all 12 criteria filled for S1, S2, S3 (SC-001).
- [ ] `delos/docs/s1-vt100.md`, `s2-alacritty.md`, `s3-wezterm.md` writeups (SC-002).
- [ ] `delos/docs/recommendation.md` — one crate, rationale, selection + alt-screen
      feasibility stated (SC-003/004/005).
- [ ] Host-terminal matrix filled (SC-006).
- [ ] Root `cargo tree`/test confirms zero spike deps + green shipping crate (SC-007).
