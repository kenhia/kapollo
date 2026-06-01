# delos

Isolated spike workspace for the **Terminal-Grid Spike** (feature `003-grid-spike`).

`delos/` is its **own** Cargo workspace, deliberately excluded from the root
`kapollo` crate (`exclude = ["delos"]` in the root `Cargo.toml`). It has a separate
`Cargo.lock`, so the heavy terminal-emulation crates evaluated here
(`vt100`, `alacritty_terminal`, `wezterm-term`) **never** enter the shipping
`kapollo`/`kap` dependency graph (FR-002/FR-003/FR-004).

## What this is

A throwaway evaluation of three terminal-grid backends, each implemented as a
single vertical slice that proves the same core "feel": grid render +
content-coordinate selection + scroll + alt-screen handover + explicit copy. Each
stage fills one column of the shared scorecard; the synthesis picks one crate.

| Stage | Crate | Binary | Status |
|-------|-------|--------|--------|
| S1 | `vt100` (+ optional `tui-term`) | `spike-vt100` | complete |
| S2 | `alacritty_terminal` | `spike-alacritty` | complete |
| S3 | `wezterm-term` (git pin) | `spike-wezterm` | complete |

Shared plumbing + unit-tested pure helpers live in `spike-support`.

**Outcome:** recommendation is **`wezterm-term`** (fallback `alacritty_terminal`) —
see [docs/recommendation.md](docs/recommendation.md).

## How to run

```fish
cd delos
cargo run -p spike-vt100        # S1
cargo run -p spike-alacritty    # S2
cargo run -p spike-wezterm      # S3
```

Run the full workspace gate (all stages):

```fish
cd delos
cargo fmt --check; and cargo clippy --all-targets -- -D warnings; and cargo test
```

## Deliverables

See `docs/`: the shared `scorecard.md`, per-stage writeups (`s1-vt100.md`,
`s2-alacritty.md`, `s3-wezterm.md`), and the final `recommendation.md` that feeds
the rework spec.

## References

- Feature spec: [../specs/003-grid-spike/spec.md](../specs/003-grid-spike/spec.md)
- Spike plan: [../specs/planning/grid-pivot/03-spike-plan.md](../specs/planning/grid-pivot/03-spike-plan.md)
