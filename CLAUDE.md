<!-- kproject:begin — managed by kprojects; do not edit inside this block -->
## kproject conventions

This project uses the kproject minimal harness
(<https://github.com/kenhia/kprojects>). Keep context small; prefer doing
over ceremony.

### Layout

- `sprints/` — the project's evolution, one record per PR-sized unit of
  work (a "sprint")
  - `planning/` — planning docs; at minimum `roadmap.md` (the general plan)
  - `review/` — more formal reviews as the project matures
  - sprint records: `###-<short-name>.md` for small projects, or a
    `###-<short-name>/` directory of files for larger/more formal ones
  - a sprint record is one informal narrative: goal, decisions, what
    shipped, follow-ups — written during the sprint, not after
  - projects that deploy end the record with a `## Deployed` section:
    what shipped, where, when, and what was verified live — appended
    after the deploy, not predicted before it
- `docs/` — project documentation, architecture, usage
- `.scratch/` — git-ignored scratch space for user or agent ephemera;
  use it instead of /tmp
- `justfile` — dev recipes; default recipe is `@just --list`; `just check`
  runs the CI gates; `just deploy` (or variants) if the project deploys
- `.env` — git-ignored; tokens and environment vars

### Workflow

- One sprint ≈ one PR. Sprint proposals and work items are managed in
  `korg`; durable cross-project knowledge goes in `klams`.
- Mark each work item resolved as its work completes — don't batch the
  resolutions into sprint-ship. A proposal's progress should be readable
  while the sprint is running, which is the only time it is useful.
- If the korg or klams MCP tools are unavailable in your session, say so
  up front — don't silently work around missing infrastructure.
- A few projects share contract surfaces with siblings and have a
  **guiding plan** constraining how those change; most have none, and one
  grep is the whole cost of finding out. Grep the `index.md` routing
  table in `kai:~/src/tools/cross-project-planning` — a local path on
  kai, read through kaed from any other host (`root: "kai:src"`, path
  `tools/cross-project-planning/…`); don't clone a second copy. Not
  listed → nothing applies. Listed → read the mapped plan folder before
  planning sessions and before changing a contract surface it names, and
  amend the plan in the same ship when what you build diverges from it.
- TDD preferred: write the failing test first when practical.

### Tooling preferences

- Rust managed by `cargo`; format with `cargo fmt`, lint with
  `cargo clippy --all-targets` (test targets included deliberately — a gate
  that skips them is a gate that lies)
- Mirror `rust-toolchain.toml`, `rustfmt.toml` and `clippy.toml` from a
  sibling homelab repo rather than generating them
- License is MIT unless specifically directed otherwise
<!-- kproject:end -->

## Project

kapollo (`kap`) is a Rust terminal REPL that wraps the user's real shell (fish
or bash) in a PTY and presents a two-pane UI: an **input pad** at the bottom for
composing commands, and a **transcript pad** above where each command and its
output appear as a discrete **block**. The transcript renders a real terminal
grid (`wezterm-term`), so progress bars, in-place redraws and inline color
display exactly as the program intended; full-screen (alt-screen) programs are
handed to the host terminal by passthrough. Linux only. MVP-level — the gaps
keeping it from daily use are completions and persistent input history.

### Build, run, test

```sh
just check                # the gate: fmt --check, clippy -D warnings, test
just fmt
cargo run --bin kap
cargo install --path .    # installs both `kapollo` and `kap`
```

`just check` runs exactly what `.github/workflows/ci.yml` runs, `--all-features`
included. Keep the two in step in both directions — if one gains a check, so
does the other. **`fish` must be installed** or `tests/shell_parity.rs` fails.

`delos/` is a **separate, workspace-excluded Cargo project** (the root
`Cargo.toml` carries `[workspace] exclude = ["delos"]`) holding throwaway
terminal-backend spike code from the sprint-003 grid pivot. Excluding it keeps
vt100/alacritty_terminal out of kapollo's dependency graph and lockfile;
`just check` does not build it.

### Read these first

- `docs/architecture.md` — the authoritative technical reference; decisions
  D1–D30 recorded with rationale. Don't diverge from it without updating it.
- `docs/specification.md` — the combined, current specification
- `src/app.rs` — the event loop, and the largest load-bearing file in the repo
- `src/grid/` — the `wezterm-term` grid and the block model
- `docs/usage.md` + `docs/keymap-defaults.toml` — every key, slash command and
  config key
- `sprints/` — one record per shipped sprint; 001–007 are the Spec-Kit specs
  migrated in sprint 008, kept as directories

### Conventions and gotchas

- **Terminal UX is the quality bar.** No flicker, no lost output, no orphaned
  cursor across resize, scroll and focus changes. The wrapped shell keeps TTY
  semantics, signal forwarding, exit codes and cwd — any deviation is
  documented and opt-in.
- **Logging goes to a file sink, never the terminal.** `tracing`, quiet by
  default, verbosity via repeated `-v`. Anything written to stdout/stderr
  corrupts the TUI.
- **Panics are caught at the event-loop boundary** and surfaced as recoverable
  errors; the terminal is restored on exit, error and panic alike.
- Block boundaries and exit codes come from auto-injected **OSC 133** shell
  hooks, with a sentinel fallback for shells that lack them.
- `wezterm-term` is a **pinned git rev**, not a crates.io release —
  reproducibility rests on that rev plus the committed `Cargo.lock`. Don't bump
  it casually.
- **Never run `cargo clippy --fix` against a dirty tree**, or any flag that
  auto-applies semantically risky changes, without asking first.
- Docs are part of done: a change to how kapollo is built, configured or used
  updates `README.md` and the relevant `docs/` file in the same sprint.
