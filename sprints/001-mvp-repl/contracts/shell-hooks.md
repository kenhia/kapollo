# Contract: Shell Integration (Block Boundaries)

**Feature**: 001-mvp-repl

How kapollo delimits command blocks and captures exit codes from the wrapped
shell's PTY byte stream (D12; research R2/R3).

## Primary: OSC 133 semantic prompt marks

kapollo auto-injects a per-shell hook (FR-007) that emits these escape
sequences, which the output processor parses (`vte`):

| Sequence | Meaning | kapollo action |
|----------|---------|----------------|
| `OSC 133 ; A ST` | Prompt start | (boundary context) |
| `OSC 133 ; B ST` | Command start (input accepted) | mark command-input boundary |
| `OSC 133 ; C ST` | Command output start | begin appending to current block |
| `OSC 133 ; D ; <exit> ST` | Command end + exit code | close block, record `exit_code` |

(`OSC` = `ESC ]`, `ST` = `ESC \` or `BEL`.)

### Injection (FR-007)

- **bash**: kapollo starts bash so its snippet is sourced after the user's
  rc (e.g. generated rcfile / `PROMPT_COMMAND` + `PS0`), emitting `A/B`
  around the prompt and `C/D` around command execution. User rc is preserved.
- **fish**: kapollo sources a snippet (via `--init-command` / event
  functions `fish_prompt`, `fish_preexec`, `fish_postexec`) that emits the
  marks. User config is preserved.

## Fallback: sentinel injection (FR-005)

When OSC 133 marks are unavailable (unknown shell, hook injection failed),
kapollo wraps each submitted command so that, after it runs, the shell emits
a unique per-session nonce followed by the exit status. kapollo scans the
stream for the nonce to close the block and capture the code.

- Nonce is high-entropy and session-unique to avoid collisions with command
  output.
- Best-effort: documented limitation for commands that alter control flow or
  print the nonce themselves (R3).

## Selection rule

1. If the shell is fish or bash and hook injection succeeds → OSC 133 mode.
2. Otherwise → sentinel mode.

`boundary_mode` (see data-model ShellSession) records which is active.

## Environment

The wrapped shell is always spawned with `KAPOLLO_ACTIVE=1` and
`KAPOLLO_VERSION=<version>` (FR-008, D21).

## Acceptance mapping

- FR-004, FR-005, FR-006, FR-007, FR-008.
