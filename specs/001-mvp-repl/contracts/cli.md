# Contract: CLI Invocation

**Feature**: 001-mvp-repl

kapollo's external interface as a command-line program. Binaries `kap` and
`kapollo` are identical.

## Synopsis

```
kap [OPTIONS]
kapollo [OPTIONS]
```

Launches the split-pad REPL wrapping the configured shell. With no options,
reads `~/.config/kapollo/config.toml` (or defaults) and starts the TUI.

## Options

| Option | Description | Default |
|--------|-------------|---------|
| `--shell <PATH>` | Shell to wrap; overrides config and `$SHELL`. | config / `$SHELL` |
| `--config <PATH>` | Use an alternate config file. | `~/.config/kapollo/config.toml` |
| `--verbose` / `-v` | Raise log verbosity (repeatable). | quiet |
| `--version` / `-V` | Print version and exit. | — |
| `--help` / `-h` | Print help and exit. | — |

Environment:
- `KAPOLLO_LOG` — log level override (e.g. `debug`).
- `NO_COLOR` — disables color in kapollo chrome (FR-031).

## Behavior contract

- **Exit code 0** on clean exit (via `/quit`, wrapped shell exit, or EOF).
- **Non-zero** exit on fatal startup error (e.g. shell not found, cannot
  open PTY), with an actionable message to stderr.
- **No TTY**: if stdout is not a TTY, kapollo MUST NOT render the TUI; it
  prints a diagnostic to stderr and exits non-zero (FR-032).
- **Terminal restoration**: on any exit path (success, error, panic, signal),
  the terminal is restored to a clean state (FR-025, FR-026).
- **Environment**: the wrapped shell is spawned with `KAPOLLO_ACTIVE=1` and
  `KAPOLLO_VERSION=<version>` (FR-008).

## Acceptance mapping

- FR-001, FR-002, FR-008, FR-025, FR-026, FR-031, FR-032.
