# Phase 0 Research: kapollo MVP

**Feature**: 001-mvp-repl | **Date**: 2026-05-29

This document consolidates the technical research backing the plan. Most
high-level decisions were already locked during planning (brainstorm D1–D21,
[docs/architecture.md](../../docs/architecture.md)); this file records the
remaining implementation-level resolutions and their rationale.

## R1 — Async runtime vs. threads + channels

- **Decision**: Use threads + channels (a dedicated PTY-reader thread feeding
  an `mpsc`/`crossbeam` channel) with a single-threaded event loop that
  `select`s over input events, PTY output, and child-exit. No async runtime
  for MVP.
- **Rationale**: The workload is a small fixed set of event sources; a TUI
  must serialize all state mutation and rendering anyway. crossterm's event
  stream + a blocking PTY read thread is simpler than introducing `tokio`,
  fewer dependencies, and aligns with Constitution VII (simplicity). The
  architecture is runtime-agnostic, so this is reversible.
- **Alternatives considered**: `tokio` (more machinery than warranted for a
  single PTY + keyboard); pure single-thread non-blocking `mio` poll (more
  manual fd plumbing than threads+channels for little gain).

## R2 — Block boundary detection (OSC 133) for fish and bash

- **Decision**: Auto-inject a shell integration snippet that emits OSC 133
  marks: `A` (prompt start), `B` (command start), `C` (output start),
  `D;<exit>` (command end + exit code). Parse these with `vte` to delimit
  blocks and capture exit codes. Fall back to sentinel injection when marks
  are unavailable.
- **Injection mechanism**:
  - **bash**: launch with a controlled startup so kapollo's snippet is
    sourced (e.g. a generated rcfile via `--rcfile`, or `PROMPT_COMMAND` +
    a `DEBUG` trap / `PS0` for command-start). Preserve the user's own rc by
    sourcing it first.
  - **fish**: source a generated snippet defining `fish_prompt`/
    `fish_preexec`/`fish_postexec` event functions (fish provides
    `--init-command` and event hooks) that emit the marks. Preserve user
    config.
- **Rationale**: OSC 133 is the de-facto standard used by modern terminals
  (WezTerm, kitty, VS Code) and is the most robust way to know precisely
  where output starts/ends and what the exit code was, without prompt
  sniffing (D12).
- **Alternatives considered**: prompt-regex heuristics (rejected — fragile,
  D12); sentinel-only (kept as fallback, less robust on complex shell
  constructs).
- **Exit-code capture**: from `OSC 133;D;<code>`; sentinel fallback appends
  an echo of `$?`/`$status` after the command.

## R3 — Sentinel fallback design

- **Decision**: When OSC 133 cannot be installed/observed, wrap each
  submitted command so a unique, high-entropy marker plus the exit status is
  emitted after it completes; kapollo scans the stream for the marker to
  close the block and record the code.
- **Rationale**: Guarantees blocks/exit codes even for unknown shells (D12).
- **Risk/limitation**: Can be confused by commands that themselves print the
  marker (mitigated by high-entropy nonce per session) or alter control flow;
  documented as best-effort.

## R4 — Alt-screen detection & passthrough

- **Decision**: Detect alt-screen enter/leave via the DEC private mode
  sequences `?1049h` / `?1049l` (and legacy `?47h/?47l`) in the `vte` parse
  stream. On enter, switch to passthrough: copy raw PTY bytes to host stdout
  and host stdin bytes to the PTY verbatim; suspend block capture. On leave,
  restore the split UI and resume capture.
- **Rationale**: Lets the host terminal's real emulator handle the cell grid
  (D4) so `vim`/`less`/`top` work natively. Avoids building an emulator.
- **Alternatives considered**: full terminal grid emulation (rejected, D4 —
  large effort, unnecessary).
- **Open implementation detail (non-blocking)**: whether to hide all kapollo
  chrome during passthrough or reserve a status line — validate with vim/less.

## R5 — Shift+Enter vs Alt+Enter disambiguation

- **Decision**: Enable the Kitty keyboard protocol via crossterm where the
  terminal supports it to receive a distinct `Shift+Enter`. Always accept
  `Alt+Enter` as a newline as a universally available fallback (D16).
- **Rationale**: Many terminals send identical bytes for Enter and
  Shift+Enter; Alt+Enter is reliably distinguishable. Supporting both meets
  FR-010 everywhere.
- **Alternatives considered**: Shift+Enter only (fails on common terminals);
  a leader-key newline (deferred — config remap is post-MVP).

## R6 — Output retention: ring-buffer & caps

- **Decision**: Each block stores output in a byte ring buffer with a
  configurable cap (default 1 MiB / 50k lines, hard max 64 MiB). The
  transcript enforces a whole-session cap (default 128 MiB / 1000 blocks),
  evicting oldest blocks first. Head-dropping within a block records a
  visible `… output truncated …` marker.
- **Rationale**: Bounds memory under pathological output (FR-016, SC-006)
  while keeping the most recent, most relevant output. Byte storage (not just
  rendered lines) satisfies D8 and future `/save`/`/filter`/AI/history.
- **Alternatives considered**: unbounded buffers (rejected — OOM risk);
  line-only storage (rejected — loses bytes needed for later features).

## R7 — Ctrl-C / signal forwarding

- **Decision**: Put the terminal in raw mode so kapollo receives Ctrl-C as a
  key event; forward SIGINT to the PTY's foreground process group (write the
  intr char / send signal to the child pgid) rather than terminating kapollo
  (FR-024). kapollo's own quit is `/quit`.
- **Rationale**: Matches user expectation that Ctrl-C interrupts the running
  command, not the wrapper (SC-004).
- **Note**: SIGWINCH/resize is propagated to the PTY via `TIOCSWINSZ` on
  terminal resize (FR-017, FR-019).

## R8 — Terminal lifecycle & panic safety

- **Decision**: A RAII terminal guard enters raw mode + alt-screen on start
  and unconditionally restores (leave alt-screen, show cursor, disable raw
  mode) on drop. Install a panic hook that runs the same restore before
  printing the panic, and catch panics at the event-loop boundary to surface
  a recoverable error (FR-025, FR-026).
- **Rationale**: Constitution VI — terminal must never be left corrupted.
- **Alternatives considered**: ad-hoc cleanup at each exit point (rejected —
  easy to miss a path; RAII + panic hook is robust).

## R9 — Logging to file (never to TUI)

- **Decision**: `tracing` with `tracing-appender` writing to a rolling file
  under the XDG state dir (e.g. `~/.local/state/kapollo/kapollo.log`).
  Default level quiet (warn/error); `--verbose` / `KAPOLLO_LOG` raises it.
  No stdout/stderr logging while the TUI is active.
- **Rationale**: Constitution VI — log output must not corrupt the TUI
  (FR-030).

## R10 — Config loading & defaults

- **Decision**: `serde` + `toml`, loaded from
  `~/.config/kapollo/config.toml` via the `directories` crate; missing file
  → all defaults. MVP config keys: `shell`, `leader_char`,
  `per_block_cap`, `transcript_cap` (FR-028, FR-029). Unknown keys warn (to
  log) but don't fail.
- **Rationale**: XDG-correct (D15), forgiving defaults (FR-028).
- **Alternatives considered**: env-var-only config (rejected — less
  discoverable); failing on unknown keys (rejected — brittle across versions).

## R11 — Testing strategy for PTY/TUI (Constitution III exception)

- **Decision**: Unit-test pure logic (input router, ring-buffer caps, config
  parsing, OSC 133 parser, sentinel scanner) directly. Cover end-to-end
  PTY/shell behavior with integration tests that spawn a real shell in a
  headless PTY (`portable-pty`) and assert on captured block output and exit
  codes. Rendering is validated via `ratatui`'s `TestBackend` where feasible;
  full interactive passthrough is covered by smoke tests.
- **Rationale**: Interactive terminal behavior cannot be fully unit-isolated;
  Constitution III explicitly permits integration/smoke tests on a headless
  harness for such code, documented here.
- **Alternatives considered**: mocking the PTY entirely (rejected — would not
  validate the real shell-integration behavior that is the crux of the MVP).

## R12 — `KAPOLLO_ACTIVE` environment export

- **Decision**: Set `KAPOLLO_ACTIVE=1` and `KAPOLLO_VERSION=<crate version>`
  in the spawned shell's environment (FR-008, D21).
- **Rationale**: Lets prompts/scripts/rc files detect the kapollo session and
  adapt (e.g. avoid nested launches, tweak prompt). Simple, no downside.

## Open items (non-blocking, defer to tasks/implementation)

- Exact passthrough chrome behavior (hide all vs. keep status line).
- Precise bash startup-file strategy vs. `PROMPT_COMMAND`/`PS0` mix to emit
  OSC 133 reliably without clobbering user config.
- Whether to ship hooks as embedded strings or generated via `kap init`.
