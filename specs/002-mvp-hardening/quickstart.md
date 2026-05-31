# Quickstart & Validation: kapollo MVP Hardening

How to build, run, and validate the 002 hardening sprint. Each check maps to a
Success Criterion (SC) in [spec.md](spec.md). Items marked **[manual TTY]** need a
real terminal; the rest are covered by `cargo test`.

## Build & run

```bash
cargo build
./target/debug/kap            # or: cargo run --bin kap
```

## Automated gate (run first)

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

New/extended test suites for this sprint:
- `tests/render_normalize.rs` — output normalization & first-line parity (SC-001/002)
- `tests/caps.rs` (extended) — incremental/bulk cap enforcement parity + flood time budget (SC-005)
- `tests/passthrough.rs` (extended) — no spurious input; clean restore (SC-004)
- `tests/cwd_osc7.rs` — OSC 7 → cwd update (SC-008)
- `tests/chrome.rs` — borderless layout, conditional exit, blank line, `λ` prefix (SC-003)
- `tests/scrolling.rs` — PgUp/PgDn/Home/End (SC-009)
- `tests/config.rs` (extended) — `prompt_char`/`prompt_color` defaults & parsing

## Manual validation (mapped to SC)

### SC-001 / SC-002 — clean render *(US1)* **[manual TTY]**
1. `ls` then repeatedly `echo $SHELL` until the transcript scrolls past a full screen.
2. Confirm: no characters overwrite the input/status chrome; no stray characters
   (e.g. a trailing `L`) on any row after scroll; every line of every block intact.
3. `printf 'a\rb\n'` and `cat` a file with tabs/multiple lines — confirm the first
   line is not corrupted and no `\r`/escape artifacts appear.

### SC-003 — chrome redesign *(US2)* **[manual TTY]**
1. Confirm the transcript has **no** surrounding box/border.
2. Confirm a single horizontal **rule above the input** shows the cwd.
3. Run a command that exits 0 → **no** exit code shown; run `false` → non-zero exit
   shown on the rule.
4. Run two commands → a **blank line** separates the two output blocks.
5. Each command is echoed with a **`λ`** prefix, colorized red (color enabled).

### SC-004 — passthrough robustness *(US3)* **[manual TTY]**
1. `vi test.txt` → confirm **no** spurious characters (e.g. `]11;rgb:...`) appear in
   the buffer or on the command line; edit, `:q`.
2. Confirm the split-pad UI is restored intact with the prior transcript.
3. Repeat with `bpytop`; on exit confirm the terminal is fully restored (cursor
   visible, normal mode, no leftover alt-screen) — every time, repeated entries.

### SC-005 — performance under flood *(US4)* **[manual TTY + timed]**
1. Time the bare shell: `time sh -c 'yes | head -n 5000000 >/dev/null'`.
2. In `kap`: run `yes | head -n 5000000`. Confirm it completes in roughly the same
   order of magnitude (seconds, not minutes) and the UI stays responsive throughout.
3. The `tests/caps.rs` flood test also asserts cap enforcement stays within a
   wall-clock budget for a flood-shaped input.

### SC-006 — prompt Ctrl-C under flood *(US4)* **[manual TTY]**
1. Start `yes | head -n 5000000` (or unbounded `yes`).
2. Press **Ctrl-C** during the flood → the command is interrupted promptly
   (small bounded delay, not requiring the flood to finish).

### SC-007 — color & `NO_COLOR` *(US5)* **[manual TTY]**
1. Run `kap` → the `λ` prompt is **red**.
2. Run `NO_COLOR=1 kap` → chrome color is **suppressed** (`λ` rendered unstyled).

### SC-008 — cwd follows `cd` *(US5)* **[manual TTY]**
1. `pwd` (status shows current dir), then `cd /tmp`, then `pwd`.
2. Confirm the status rule cwd updates to `/tmp` on the next prompt (fish & bash).
3. A sentinel-fallback shell (e.g. `dash`) shows no live cwd update — expected (D23).

### SC-009 — `/exit` & scrolling *(US5)*
1. Type `/exit` → kapollo quits exactly like `/quit`. **[manual TTY]**
2. Fill the transcript; press **PgUp/PgDn** to scroll and **Home/End** to jump to
   top/bottom. **[manual TTY]**
3. Run `/help` → confirm it lists `/exit` and the scrolling keys (PgUp/PgDn,
   Home/End). (Content also asserted in `tests/`.)

## Config sanity (optional)

```toml
# ~/.config/kapollo/config.toml
prompt_char = "❯"
prompt_color = "cyan"
```
Run `kap` → the prompt is a cyan `❯`. Set `prompt_char = ">>"` → kapollo reports a
config error (must be exactly one character).

## Done criteria
- Automated gate green (fmt/clippy/test).
- All SC manual checks pass on a real terminal.
- `docs/` (architecture, usage, setup, specification) and README/CHANGELOG updated
  per the polish phase before the sprint is shipped.
