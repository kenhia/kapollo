# Phase 1 Data Model: kapollo MVP

**Feature**: 001-mvp-repl | **Date**: 2026-05-29

Entities are in-memory for the MVP (no database). Types are described
conceptually; field names are indicative, not final Rust signatures.

## Block

One command plus its captured output and exit code. The atomic unit of the
transcript and the foundation for post-MVP features (D8).

| Field | Type | Notes |
|-------|------|-------|
| `id` | monotonically increasing id | unique within a session |
| `command` | string | the submitted input text (may be multiline) |
| `started_at` | timestamp | when the command was submitted |
| `ended_at` | optional timestamp | set when the block closes |
| `output` | ring buffer of bytes | capped; retains tail (R6) |
| `truncated` | bool | true if head bytes were dropped |
| `exit_code` | optional integer | from OSC 133 `D;<code>` or sentinel |
| `state` | enum: `Running` \| `Closed` \| `Interactive` | `Interactive` = entered passthrough |
| `private` | bool | reserved (D13); default false in MVP |
| `save_output` | bool | reserved (D13); default true in MVP |

**Validation / rules**:
- A block is created when non-slash input is submitted and a command is sent
  to the shell.
- `output` is appended as the output processor emits segments for this block.
- On exceeding the per-block cap, oldest bytes are dropped and `truncated`
  is set (FR-016).
- A block closes when the end mark (OSC 133 `D`) or sentinel is observed;
  `ended_at` and `exit_code` are set (FR-006).
- Entering alt-screen sets `state = Interactive` and suspends capture (FR-018).

**State transitions**:

```
(submit non-slash input) ──▶ Running
Running ──(alt-screen enter)──▶ Interactive ──(alt-screen leave)──▶ Running
Running ──(end mark / sentinel)──▶ Closed
```

## Transcript (Session)

Ordered collection of blocks for the running instance; the source of truth
the UI renders from.

| Field | Type | Notes |
|-------|------|-------|
| `blocks` | ordered list of Block | newest last |
| `total_bytes` | integer | sum of retained output bytes |
| `scroll_offset` | integer | view position, independent of input pad |

**Rules**:
- Enforces the whole-transcript cap: when `total_bytes` or block count
  exceeds the limit, oldest blocks are evicted first (FR-016, R6).
- `/clear` clears the visible transcript (FR-023).

## InputPad

The editable bottom region and its state.

| Field | Type | Notes |
|-------|------|-------|
| `buffer` | multiline text | current edit content |
| `cursor` | position | line/column within buffer |
| `scroll` | integer | internal scroll when content exceeds height cap |

**Rules**:
- Enter submits `buffer` (FR-009); Shift+Enter / Alt+Enter insert a newline
  (FR-010, FR-011).
- Auto-grows to a height cap, then scrolls internally (FR-012).

## InputHistory

kapollo's own history of submitted inputs, separate from the shell's (D20).

| Field | Type | Notes |
|-------|------|-------|
| `entries` | ordered list of string | submitted inputs, newest last |
| `cursor` | optional index | current recall position |

**Rules**:
- A submitted input is appended (FR-013).
- Up moves toward older entries, Down toward newer; recalled text replaces
  the input pad buffer (FR-013).
- In-session only for MVP (no persistence).

## Configuration

Loaded from `~/.config/kapollo/config.toml`; defaults when absent (FR-028).

| Key | Type | Default | Notes |
|-----|------|---------|-------|
| `shell` | string | `$SHELL` | wrapped shell (FR-002) |
| `leader_char` | char | `/` | slash-command leader (FR-021) |
| `per_block_cap_bytes` | integer | 1 MiB | hard max 64 MiB (R6) |
| `per_block_cap_lines` | integer | 50000 | whichever hit first |
| `transcript_cap_bytes` | integer | 128 MiB | (R6) |
| `transcript_cap_blocks` | integer | 1000 | whichever hit first |

**Rules**:
- Unknown keys are logged and ignored, not fatal (R10).
- Caps clamp to hard maxima.

## ShellSession

The wrapped shell process and its integration state.

| Field | Type | Notes |
|-------|------|-------|
| `pty` | PTY master handle | read/write byte channel |
| `child` | process handle | the spawned shell |
| `pgid` | process group id | for SIGINT forwarding (R7) |
| `shell_kind` | enum: `Fish` \| `Bash` \| `Other` | drives hook selection |
| `boundary_mode` | enum: `Osc133` \| `Sentinel` | detection strategy (R2/R3) |
| `env` | map | includes `KAPOLLO_ACTIVE=1`, `KAPOLLO_VERSION` (R12) |

**Rules**:
- On spawn, kapollo injects the integration hook for fish/bash and exports
  the env vars (FR-007, FR-008).
- Resize forwards new dimensions to the PTY (FR-017, FR-019).
- When the child exits, kapollo terminates cleanly (FR-027).

## Relationships

```
Configuration ──drives──▶ ShellSession (shell, caps via Transcript)
ShellSession ──produces bytes──▶ OutputProcessor ──segments──▶ Block
Transcript ──contains──▶ Block (ordered)
InputPad ──submit──▶ (slash? → SlashCommand) | (else → new Block + shell stdin)
InputPad ──submit──▶ InputHistory (append)
```
