# Setup

How to build, install, and run kapollo on Linux.

## Prerequisites

- **Linux** (the MVP targets Linux only).
- **Rust** stable toolchain. The repository pins it via
  [`rust-toolchain.toml`](../rust-toolchain.toml); `rustup` will install the
  pinned version automatically on first build.
- A supported interactive shell for the best experience: **fish** or **bash**
  (these get precise block boundaries via OSC 133 hooks). Other shells work via
  a sentinel fallback.

## Build

```sh
git clone <repository-url> kapollo
cd kapollo
cargo build --release
```

The build produces two equivalent binaries (the `kap` name is the short alias):

- `target/release/kapollo`
- `target/release/kap`

## Install

Install both binaries into your Cargo bin directory (`~/.cargo/bin`):

```sh
cargo install --path .
```

Ensure `~/.cargo/bin` is on your `PATH`.

## Run

```sh
kap                      # wrap $SHELL
kap --shell /bin/bash    # wrap a specific shell
kap --config ./my.toml   # use an alternate config file
kap --verbose            # raise log verbosity (repeatable: -vv)
kap --version            # print version and exit
kap --help               # print help and exit
```

When standard output is not a TTY, kapollo does not start the TUI.

## Logs

Logging is quiet by default and **never** writes to the TUI. Logs go to a file
under your XDG state directory. Raise verbosity with `-v`/`--verbose` (or the
`KAPOLLO_LOG` environment filter) when diagnosing an issue.

## Configuration

kapollo reads `~/.config/kapollo/config.toml` if present (see
[usage.md](usage.md) for the full schema). It runs with sensible defaults when
the file is absent. The top-level keys are `shell`, `leader_char`,
`prompt_char`, `prompt_color`, and the `[caps]`, `[mouse]`, `[clipboard]`, and
`[scroll]` tables. A `prompt_char` must be a single character; an unknown
`prompt_color` name logs a warning and falls back to the default.
