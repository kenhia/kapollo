# Changelog

All notable changes to this project are documented here. The format is based
on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **MVP split-pad shell REPL** wrapping the user's real shell (fish/bash) in a
  PTY, with a transcript pad of command blocks, an input pad, and a status line
  (current working directory + last exit code).
- **Block boundaries** via OSC 133 semantic prompt marks, with a sentinel-nonce
  fallback for shells without a hook. Per-command exit codes are captured.
- **Multiline editing** (Shift+Enter / Alt+Enter insert newlines; Enter submits
  the whole buffer) and kapollo's own **input history** (Up/Down recall),
  separate from the shell's history.
- **Independent transcript scrolling** (PageUp/PageDown).
- **Full-screen passthrough** for alt-screen programs (`vim`, `less`, `top`),
  with terminal resize forwarded during passthrough and the split-pad UI
  restored on exit.
- **Slash commands** `/help`, `/clear`, `/quit`, with a doubled-leader escape
  for literal leader characters.
- **Ctrl-C** forwarded to the running command (not kapollo); always-clean
  terminal teardown on exit, error, and panic.
- **Configuration** from `~/.config/kapollo/config.toml` (shell, leader char,
  output caps) with defaults and cap clamping; file-only logging; `NO_COLOR`
  support; graceful degradation on tiny terminals.

### MVP hardening (002)

#### Added

- **Borderless chrome**: the transcript and input pads drop their borders; each
  command is echoed with a colorized prompt glyph (`λ` by default, configurable
  via `prompt_char` / `prompt_color`) and blocks are separated by a blank line.
- **Status rule** directly above the input pad showing the cwd (always) and the
  last exit code (only when non-zero); the cwd follows `cd` via **OSC 7** cwd
  reports.
- **Page and jump scrolling**: PageUp/PageDown scroll the transcript a page at a
  time and Home/End jump to the oldest/newest output; submitting re-pins to the
  newest output.
- **`/exit`** as an alias for `/quit`.

#### Changed

- **Output normalization**: captured output is reduced to clean printable text —
  bare `\r` and other C0 controls are dropped, `\r\n` collapses to `\n`, and
  OSC/CSI/DCS escape sequences (including SGR styling and terminal
  query/responses) never leak into the transcript as visible text.
- **Flood responsiveness**: ring-buffer cap enforcement is amortized O(1) and the
  event loop drains the PTY in bounded passes, so multi-million-line output
  completes near shell-native time and Ctrl-C stays responsive.

#### Fixed

- **Passthrough fidelity**: stdin is forwarded to alt-screen programs verbatim
  (no `KeyEvent` re-encoding), so terminal query/responses (OSC 11/10/4, Device
  Attributes, cursor-position) reach the program intact; an explicit SGR/cursor
  reset is emitted on passthrough exit so no residual style or hidden cursor
  bleeds into the restored UI.
