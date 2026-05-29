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
