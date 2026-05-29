# kapollo Architecture

> Status: **DRAFT** for review. Derived from
> [specs/planning/brainstorm.md](../specs/planning/brainstorm.md) decisions
> D1–D18. This is the authoritative technical reference per Constitution
> Principle II (Architecture First). Update during each spec's polish phase.

Last updated: 2026-05-29

## 1. Overview

kapollo (`kap`) is a Rust terminal application that wraps a user's real
shell and presents an Apollo-DM-style split UI: an **input pad** at the
bottom where commands are typed, and an **output (transcript) pad** above
where commands and their output appear as discrete **blocks**. A
**slash-command** layer (invoked by a configurable **leader char**) adds
features beyond a plain shell wrapper.

The shell runs in a **PTY**; kapollo feeds it input and captures its output.
Normal commands are captured into append-mostly blocks; full-screen
(alt-screen) programs like `vim`/`top`/`less` are handed to the host
terminal via **passthrough**.

### Design tenets
- **Wrap, don't reinvent** the shell (D1/D2): fidelity of cwd, env, aliases,
  pipes, exit codes comes from the real shell.
- **Blocks are the source of truth** (D3/D8): a block holds the command, its
  captured output bytes, and its exit code. The UI, `/save`, `/filter`, the
  future history DB, and the future AI layer are all just consumers and
  producers of blocks.
- **Don't build a terminal emulator** (D4): append for normal output; hand
  raw bytes to the host terminal for alt-screen apps.
- **TUI integrity** (Constitution VI): logs never touch the screen; panics
  are caught at the event-loop boundary; terminal state is always restored.

## 2. Layered Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                          TUI / Renderer                         │
│   input pad · transcript pad · status chrome · slash-mode UI    │
│                      (ratatui + crossterm)                      │
└───────────────▲───────────────────────────────┬─────────────────┘
                │ render(view of State)         │ key/resize events
                │                               ▼
┌───────────────┴─────────────────────────────────────────────────┐
│                        App / Event Loop                         │
│  owns State; routes input; orchestrates layers; catches panics  │
└───┬───────────────┬───────────────────┬───────────────┬─────────┘
    │               │                   │               │
    ▼               ▼                   ▼               ▼
┌────────┐   ┌──────────────┐   ┌──────────────┐  ┌──────────────┐
│ Input  │   │ Slash Cmd    │   │  Session /   │  │   Config &   │
│ Router │   │  Registry    │   │  Transcript  │  │ Persistence  │
│        │   │ (builtins)   │   │   (blocks)   │  │  (TOML/XDG)  │
└───┬────┘   └──────┬───────┘   └──────▲───────┘  └──────────────┘
    │ pass-through  │ builtin acts on  │ append output / close block
    │ to shell      │ State/blocks     │
    ▼               ▼                  │
┌───────────────────────────────────────────────────────────────┐
│                       PTY / Process Layer                     │
│  spawn shell · write stdin · read output · resize · signals   │
│                          (portable-pty)                       │
└───────────────────────────┬───────────────────────────────────┘
                            │ raw bytes
                            ▼
┌───────────────────────────────────────────────────────────────┐
│                  Output Processor (vte parse)                 │
│  detect OSC 133 marks · detect alt-screen enter/leave ·       │
│  split into block segments · strip/translate styling          │
└───────────────────────────────────────────────────────────────┘
```

### Layer responsibilities

- **PTY / Process layer** — Spawns the configured shell in a PTY, owns the
  master fd, writes bytes to the shell, reads bytes from it, propagates
  resize (`SIGWINCH`/`TIOCSWINSZ`), forwards signals (Ctrl-C → the
  foreground process group). Knows nothing about blocks. Crate:
  `portable-pty`.
- **Output Processor** — Parses the PTY byte stream with `vte`. Its jobs:
  (1) detect **OSC 133** prompt/command marks for block boundaries (D12);
  (2) detect **alt-screen** enter/leave (`?1049h`/`?1049l`) to trigger
  passthrough; (3) emit output segments tagged with the current block;
  (4) handle styling (preserve or strip for the block model). It does NOT
  maintain a cell grid (D4).
- **Session / Transcript model** — The source of truth: an ordered list of
  **blocks**. Each block: id, command text, start/end timestamps, captured
  output (ring-buffered bytes, D14), exit code, and flags (e.g.
  `save_output`, `private`). Enforces per-block and transcript caps.
- **Input Router** — Decides per submitted input whether it's a **slash
  command** (starts with leader char, not escaped) or **pass-through** to
  the shell (D6). Handles the doubled-leader escape.
- **Slash Command Registry** — Maps slash command names to handlers. MVP
  builtins: `/quit`, `/clear`, `/help`. Handlers receive an app context
  (read State / blocks, write a new block, mutate UI). Designed so later
  commands (`/save`, `/filter`, AI commands) and plugins slot in unchanged.
- **TUI / Renderer** — Draws input pad, transcript pad, and status chrome
  from a read-only view of State. Translates key/resize events into App
  messages. Manages alt-screen handoff for passthrough. Crates: `ratatui`,
  `crossterm`.
- **App / Event Loop** — Owns `State`, wires the layers, runs the main
  select loop (input events ⨉ PTY output ⨉ child-exit ⨉ ticks), and is the
  **panic boundary**: a panic is caught, the terminal is restored, and the
  error is logged + surfaced (Constitution VI).
- **Config & Persistence** — Loads `~/.config/kapollo/config.toml` (D15);
  provides typed config to all layers. Future: history DB and AI sections
  live here without bloating the base file.

## 3. The Block Lifecycle

A **block** is one command + its output + exit code. This is the central
data structure (D8) and the foundation for `/save`, `/filter`, the history
DB (D13), and AI (D11).

```
 user submits input (Enter)
        │
        ▼
 Input Router: slash command?  ──yes──▶ Slash Registry handles it
        │ no                              (may create a block directly)
        ▼
 open new Block { command, started_at }
        │ write command bytes to PTY stdin
        ▼
 Output Processor streams segments ──▶ append to Block.output (ring buffer)
        │
        ├── alt-screen detected ──▶ enter PASSTHROUGH (see §4); block paused
        │
        ▼
 OSC 133 "command finished" mark (or sentinel) with exit code
        │
        ▼
 close Block { ended_at, exit_code }  ──▶ enforce caps ──▶ render
        │
        └── (post-MVP) persist to history DB if enabled & not private
```

### Block boundary detection (D12)
1. **Primary — OSC 133 semantic prompt marks.** kapollo installs (or asks
   the user to source) a small per-shell hook that emits:
   - `OSC 133;A` — prompt start
   - `OSC 133;B` — command start (input accepted)
   - `OSC 133;C` — command output start
   - `OSC 133;D;<exit>` — command finished, with exit code
   The Output Processor reads these to delimit blocks and capture exit
   codes precisely. fish and bash hooks are provided for MVP (D17).
2. **Fallback — sentinel injection.** When marks are unavailable, kapollo
   appends a unique sentinel echo to each submitted command (e.g.
   `; printf '\\e]133;D;%s\\a' $?` equivalent) and watches for it. Less
   robust (breaks on certain shell constructs) but workable.
3. Heuristic prompt-sniffing is explicitly rejected (fragile).

### Output capture & caps (D14)
- Output is stored per block as a **ring buffer** keeping the tail; when a
  block exceeds its cap, the head is dropped and a visible
  `… output truncated …` marker is recorded.
- Two limits, both configurable:
  - **Per-block**: default **1 MiB** or **50,000 lines** (whichever first);
    hard max **64 MiB**.
  - **Whole transcript**: default **128 MiB** or **1000 blocks** (whichever
    first); oldest blocks evicted first.
- stdout and stderr are captured as a **best-effort interleaved stream**
  (single PTY stream; true separation isn't possible without losing
  ordering — documented limitation for D13).

### History store readiness (D13, post-MVP, design now)
The block model is shaped so a later embedded store (likely SQLite) can
persist `{ timestamp, command, output, exit_code }` per block:
- Block carries `private: bool` and `save_output: bool` flags.
- **Privacy leaders** at input time set these flags before execution:
  leading space → don't persist the command at all (history-style); a second
  notation (TBD, e.g. leading `space space` or a config char) → persist the
  command but not the output.
- User controls (post-MVP): disable persistence, purge all, **purge
  output-only** (keep commands, drop outputs).

## 4. Passthrough (alt-screen programs)

When the Output Processor sees the alt-screen enter sequence (`?1049h`),
kapollo:
1. Suspends block capture for the current block (marks it as having entered
   an interactive program).
2. Switches the renderer to **passthrough**: the PTY's raw bytes are written
   straight to the host terminal, and host terminal keystrokes are written
   straight to the PTY. The host terminal's own emulator does all grid work
   (D4) — kapollo draws no UI during this time.
3. On alt-screen leave (`?1049l`), resumes the normal split UI and reopens
   normal block capture.

PTY resize must be forwarded continuously so the inner program sees correct
dimensions. (Whether the kapollo chrome is hidden entirely or the inner app
gets the full terminal during passthrough is an MVP implementation detail to
validate with `vim`/`less`/`top`.)

## 4a. Shell Integration & Environment

- **Hook delivery (D19)**: kapollo **auto-injects** its OSC 133 hook into
  the spawned shell (belt-and-suspenders so block boundaries work without
  user setup). A manual `kap init <shell> | source` path may be exposed and
  injection made configurable in a later sprint. Per-shell hooks for fish
  and bash ship in MVP (D17); other shells are best-effort and fall back to
  sentinel injection (§3, D12).
- **History (D20)**: kapollo maintains its **own** input history (used for
  up/down-arrow recall in the input pad), kept **separate** from the wrapped
  shell's native history — the shell continues to record its own history as
  usual. Richer history manipulation is post-MVP (several sprints out); a
  later sprint may add config to influence shell history via the hook.
- **Active-session env (D21)**: when spawning the shell, kapollo exports
  `KAPOLLO_ACTIVE=1` (and `KAPOLLO_VERSION=<semver>`) into the child
  environment so scripts, prompts, and `rc` files can detect they are
  running inside kapollo and adapt (e.g. tweak prompt, skip a pager).

## 5. Input & Key Handling
- **Enter** submits the input pad contents (D5).
- **Up/Down arrows** navigate kapollo's own input history (recall previous
  submitted inputs into the input pad). kapollo maintains its **own**
  history, kept separate from the wrapped shell's history (D20). Richer
  history manipulation (search, editing, persistence policy) is post-MVP,
  likely several sprints out.
- **Shift+Enter** and **Alt+Enter** both insert a literal newline (D16);
  `Alt+Enter` is the reliable fallback where the terminal can't distinguish
  `Shift+Enter`. Where supported, kapollo enables the Kitty keyboard
  protocol via crossterm to disambiguate.
- **Ctrl-C** forwards SIGINT to the foreground process group (interrupts the
  running command), not kapollo itself.
- **Leader char** (default `/`) as the first char of input enters
  **slash-mode**; a doubled leader escapes to a literal leader char (D6).
- Scrolling keys navigate the transcript pad independently of the input pad.
- `\`-style shell line-continuation is deferred (needs per-shell parsing).

## 6. Concurrency Model

Single-threaded async event loop (or a small set of tasks):
- **PTY reader** produces a stream of bytes/segments.
- **Terminal input** produces key/resize events (crossterm event stream).
- **App loop** selects over: input events, PTY output, child-exit, and a
  render tick; mutates `State`; requests redraws.

Rationale: a TUI must serialize all `State` mutation and rendering. We avoid
shared-mutable-state across threads; the PTY reader hands bytes to the loop
via a channel. (Exact runtime — `tokio` vs. a hand-rolled `mio`/thread+channel
select — to be finalized in the MVP plan; the layering does not depend on the
choice.)

## 7. Module / Crate Layout (proposed)

Single binary crate for MVP; internal modules kept clean so they could
become crates later if needed.

```
kapollo/                  # crate (bin = "kap", also installs "kapollo")
├── src/
│   ├── main.rs           # arg parse, config load, terminal setup/teardown
│   ├── app.rs            # App, State, event loop, panic boundary
│   ├── config.rs         # TOML config (serde), defaults, XDG paths
│   ├── pty/              # PTY process layer (portable-pty)
│   │   ├── mod.rs
│   │   └── shell.rs      # shell detection, hook installation (fish/bash)
│   ├── output/           # Output Processor
│   │   ├── mod.rs
│   │   ├── parser.rs     # vte parsing, OSC 133, alt-screen detection
│   │   └── sentinel.rs   # fallback boundary detection
│   ├── session/          # block model & transcript
│   │   ├── mod.rs
│   │   ├── block.rs      # Block, flags, exit code, timestamps
│   │   └── ringbuf.rs    # capped output storage + caps enforcement
│   ├── input/            # input router + key handling
│   │   ├── mod.rs
│   │   └── router.rs     # slash vs pass-through, leader/escape
│   ├── slash/            # slash command registry + builtins
│   │   ├── mod.rs
│   │   └── builtins.rs   # /quit /clear /help
│   ├── ui/               # ratatui rendering
│   │   ├── mod.rs
│   │   ├── input_pad.rs
│   │   ├── transcript.rs
│   │   ├── status.rs
│   │   └── passthrough.rs
│   └── logging.rs        # tracing → file appender
├── tests/                # integration tests (PTY echo, block boundaries)
└── docs/                 # architecture.md, setup.md, usage.md
```

## 8. Technology Stack (committed)

| Concern        | Choice                         | Notes |
|----------------|--------------------------------|-------|
| TUI            | `ratatui` + `crossterm`        | Rendering + events + alt-screen |
| PTY            | `portable-pty`                 | Cross-platform PTY (Linux MVP) |
| ANSI parse     | `vte`                          | OSC 133 + alt-screen detection only |
| Config         | `serde` + `toml`               | `~/.config/kapollo/config.toml` |
| Logging        | `tracing` + `tracing-subscriber` (file appender) | Never to screen |
| Errors         | `anyhow` (app) / `thiserror` (libs) | Actionable messages |
| Fuzzy (later)  | `nucleo`                       | Slash-mode / history search |
| Markdown (later)| `pulldown-cmark`              | `/view` rendering |
| History DB (later)| `rusqlite` (SQLite)         | Rich history store (D13) |

## 9. Platform Strategy (D9)

- **MVP: Linux only.** Validate on fish + bash (D17).
- macOS and Windows are in scope **later**, gated on not degrading Linux
  parity. `portable-pty`, `ratatui`, and `crossterm` are cross-platform,
  which keeps the door open, but Windows PTY (ConPTY) and shell-hook
  differences are deferred problems.

## 10. Observability & Failure (Constitution VI)

- **Logging**: `tracing` to a file sink under the XDG state/cache dir;
  default quiet; `--verbose`/`KAPOLLO_LOG` opt-in. Never write logs to the
  TUI surface.
- **Panic boundary**: the event loop catches panics, restores the terminal
  (leave alt-screen, show cursor, disable raw mode), logs the panic, and
  surfaces a recoverable error rather than leaving a corrupted terminal.
- **Clean teardown**: terminal state is always restored on exit (normal,
  error, or signal).
- **Non-TTY invocation**: if stdout isn't a TTY, behave sanely (no TUI);
  honor `NO_COLOR`.

## 11. Open Implementation Questions (for the MVP plan, not blockers)

- Async runtime choice (`tokio` vs threads+channels) — layering is agnostic.
- Exact passthrough strategy: hide all kapollo chrome vs. reserve a status
  line during alt-screen apps.
- Per-shell hook details for fish vs. bash (prompt/preexec integration) and
  the exact auto-injection mechanism (D19): temp rc file, `--init-file`,
  `XDG_*` override, or sourcing a generated snippet at shell start.

## 12. Decision Traceability

| Arch element | Brainstorm decision |
|--------------|---------------------|
| PTY-wrapped real shell, multi-shell | D1, D2, D17 |
| Hybrid blocks + passthrough; no grid model | D3, D4 |
| Enter submit / Shift+Alt+Enter newline | D5, D16 |
| Slash-mode + leader escape | D6 |
| Terminology (pad/block/slot/leader/passthrough) | D7 |
| Block retains output bytes | D8 |
| Block boundary via OSC 133 + sentinel | D12 |
| Rich history store readiness | D13 |
| Output caps (per-block + transcript) | D14 |
| Config at `~/.config/kapollo/config.toml` | D15 |
| fish + bash MVP validation | D17 |
| `kapollo` + `kap` alias | D18 |
| Auto-inject shell hook | D19 |
| Shell-native command history | D20 |
| `KAPOLLO_ACTIVE` / `KAPOLLO_VERSION` env | D21 |
| Linux-only MVP | D9 |
