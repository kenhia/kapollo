# kapollo

> An Apollo-DM-style split-pad terminal REPL that wraps your real shell.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> ⚠️ **Not ready for prime time (yet!)** — kapollo is at a rough-MVP level.
> It works, but there are known UI and behavioral rough edges still to address.
> Linux only; wraps fish and bash, with a fallback for other shells. See
> [docs/specification.md](docs/specification.md) for the full scope.

kapollo (`kap`) wraps your real shell (fish or bash) in a PTY and presents a
two-pane UI: an **input pad** at the bottom where you compose commands, and a
**transcript pad** above where each command and its output appear as a discrete
**block**. Your working directory, environment, aliases, and shell features all
behave exactly as in your normal shell — because it *is* your shell.

The design is inspired by the "command palette + transcript" feel of tools like
Warp's blocks and the Apollo-DM split layout: a focused composition area plus a
scrollable record of what you ran and what came back.

## Screenshot

<!-- TODO: capture a screenshot or asciinema cast of kapollo running and embed
     it here (e.g. ![kapollo](docs/screenshot.png)). -->

_A screenshot/cast will be added here._

## Features

- **Command blocks** — each command + its output + exit code is a discrete,
  scrollable block.
- **Real shell** — fish/bash wrapped in a PTY; state persists across commands.
- **Precise boundaries** — OSC 133 semantic prompt marks (with a sentinel
  fallback) capture exact command spans and exit codes.
- **Multiline editing** — Shift+Enter / Alt+Enter insert newlines; Enter
  submits the whole buffer.
- **Input history** — kapollo's own Up/Down history, separate from the shell's.
- **Full-screen passthrough** — `vim`, `less`, `top` run natively; the split-pad
  UI is restored on exit.
- **Slash commands** — `/help`, `/clear`, `/quit`, with a `//` escape for a
  literal leader.
- **Safe by default** — Ctrl-C interrupts the running command (not kapollo); the
  terminal is always restored on exit, error, and panic.

## Install

Requires a stable Rust toolchain (pinned via `rust-toolchain.toml`) on Linux.

```sh
git clone https://github.com/kenhia/kapollo
cd kapollo
cargo install --path .
```

This installs both `kapollo` and the short `kap` alias into `~/.cargo/bin`.

### Build without installing

```sh
cargo build --release
./target/release/kap
```

## Usage

```sh
kap                      # wrap $SHELL
kap --shell /bin/bash    # wrap a specific shell
kap --config ./my.toml   # alternate config file
kap --verbose            # raise log verbosity (repeatable)
kap --help               # full help
```

### Key bindings

| Key | Action |
|-----|--------|
| Enter | Submit the input |
| Shift+Enter / Alt+Enter | Insert a newline (multiline compose) |
| Up / Down | Recall input history |
| Left / Right | Move the cursor |
| PageUp / PageDown | Scroll the transcript |
| Ctrl-C | Interrupt the running command |

Slash commands: `/help`, `/clear`, `/quit`. See [docs/usage.md](docs/usage.md)
for the configuration schema and full details.

## Documentation

- [docs/setup.md](docs/setup.md) — build, install, run
- [docs/usage.md](docs/usage.md) — keys, slash commands, configuration
- [docs/specification.md](docs/specification.md) — combined specification
- [docs/architecture.md](docs/architecture.md) — technical reference

## License

Licensed under the [MIT License](LICENSE).
