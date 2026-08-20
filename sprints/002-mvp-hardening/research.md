# Phase 0 Research: kapollo MVP Hardening

This document resolves the technical unknowns behind the five fix clusters in
[spec.md](spec.md). Each decision records what was chosen, why, and the
alternatives considered. The spec carried no `[NEEDS CLARIFICATION]` markers
(D22–D24 resolved the open product questions); the unknowns here are
*implementation-mechanism* questions surfaced by the Technical Context.

---

## R1 — Output normalization (cluster A: FR-001, FR-004)

**Problem**: Captured block bytes currently reach the renderer largely raw.
`Block::output_lossy()` does `String::from_utf8_lossy` over the ring buffer, and
the vte `Performer` only drops *color* CSI and passes most else through. The user
test showed three symptoms with one family of root causes:

- bare carriage returns (`\r` not part of `\r\n`) move the cursor to column 0, so
  in a `Paragraph` the "first line" appears overwritten / shifted;
- OSC color-query **responses** (e.g. `]11;rgb:2020/2020/2020`) leak in as visible
  text;
- residual escape/control bytes survive into the rendered string.

**Decision**: Normalize at the *parser* boundary, not at render time. The vte
`Performer` already classifies bytes; extend it so the captured `Output` stream
contains only printable text plus the structural whitespace `\n` and `\t`:

- `print(c)` → keep (UTF-8).
- `execute(byte)`: keep `\n` and `\t`; **convert `\r` to a line-reset** — collapse
  `\r\n` to `\n` and drop a lone `\r` (we have no grid, so column-0 redraw cannot
  be represented; dropping is the faithful plain-text projection). Drop other C0
  control bytes (BEL, etc.).
- `osc_dispatch`: already consumed by the parser (133 handled, others swallowed) —
  ensure **all** OSC payloads (including 11 responses) are consumed by the parser
  and never emitted as `Output`. The leak today is from OSC *responses arriving on
  a different path* (see R4), not from `osc_dispatch`; this guarantees the block
  path is clean regardless.
- `csi_dispatch` / `esc_dispatch` / `hook`/`put`/`unhook` (DCS): consumed, never
  emitted (alt-screen CSI already handled; everything else dropped for the
  no-grid plain-text model, D4/§6).

**Rationale**: Doing it in the parser means every consumer (transcript render,
future `/save`, `/filter`, AI layer per D8) gets clean bytes for free, and the
ring buffer stores normalized text so line-cap counting (R3) matches what is
rendered. Render-time stripping would have to be redone by every consumer and
would leave the stored bytes dirty.

**Alternatives considered**:
- *Strip at render time* — rejected: duplicated work per consumer; stored bytes
  stay dirty; line caps would count invisible bytes.
- *Keep a tiny single-line grid to honor `\r` overwrite* — rejected: violates D4
  (no grid model); the vast majority of `\r` use we care about is `\r\n` and
  progress-bar redraws, which the plain-text projection intentionally flattens.

---

## R2 — Renderer owns the full surface (cluster A/B: FR-002, FR-003, FR-005)

**Problem**: Stray characters (the trailing `L`) persist across cells after
scroll, and output overwrites the frame border. The transcript is one
`Paragraph` inside `Block::bordered()`; ratatui *does* clear its drawn area, so
residue implies bytes are reaching the host terminal **outside** ratatui's draw
(the passthrough raw-write path) and/or the bordered block's title/edge cells
interact with leaked control bytes (R1).

**Decision**: Two reinforcing changes:
1. **Remove the transcript border** (FR-005) so the transcript `Paragraph` owns a
   full-width rectangle with no edge/title cells to corrupt — this deletes the
   "overwrites frame" failure mode by construction.
2. **Guarantee a clean buffer**: render the normalized text (R1) so no raw control
   bytes are handed to ratatui, and ensure the only non-ratatui writes to stdout
   are the *intended* passthrough writes (R4). After returning from passthrough,
   force a full clear + redraw (already partially present via `terminal.clear()`),
   and additionally reset SGR/cursor (R4) so no residual style bleeds into the
   normal UI.

**Rationale**: ratatui's double-buffer only stays correct if nothing else writes
to the screen between frames. The residue is a symptom of raw passthrough writes
and dirty text, both fixed upstream (R1, R4). Removing the border shrinks the
surface and matches the requested chrome (US2).

**Alternatives considered**:
- *Call `terminal.clear()` every frame* — rejected: defeats ratatui diffing,
  causes flicker; the correct fix is to stop the out-of-band writes.

---

## R3 — Ring-buffer cap enforcement: amortized O(1) (cluster D: FR-014, FR-016)

**Problem**: `OutputBuffer::push` calls `enforce_caps()` per append;
`count_lines()` rescans the whole `VecDeque` every time and trimming pops one
byte at a time → O(total × buffer) under a flood.

**Decision**: Track state incrementally and trim in bulk.
- Maintain a running `line_count: u64` updated by counting `\n` **in the appended
  slice only** (`data.iter().filter(|&&b| b == b'\n').count()`), never rescanning
  the buffer.
- Byte-cap trim: compute `overflow = len - cap_bytes` once and `drain(..overflow)`
  in bulk, decrementing `line_count` by the number of `\n` in the drained prefix.
- Line-cap trim: while `line_count > cap_lines`, find the index just past the
  next `\n` (bounded scan from the front) and `drain(..=idx)` in bulk; decrement
  `line_count` per dropped line. (Switching the backing store to a contiguous
  `Vec` with a head cursor, or keeping `VecDeque` with `drain`, both achieve bulk
  removal; `VecDeque::drain` is sufficient and minimal.)
- **Truncated fast-path (FR-016)**: once `truncated == true` and a single appended
  chunk alone exceeds `cap_bytes`, retain only the **tail** `cap_bytes` of the
  combined data rather than appending-then-trimming, so a 5M-line flood never
  materializes the whole stream. Concretely: if `incoming.len() >= cap_bytes`,
  clear and copy the last `cap_bytes` of `incoming`; else append then bulk-trim.

**Rationale**: Every byte is touched O(1) amortized (counted once on the way in,
dropped at most once via a bulk `drain`). The fast-path bounds peak work per
chunk to `cap_bytes`, independent of total flood size.

**Validation**: Unit tests assert `byte_len`/`truncated`/line retention equal the
old (correct-but-slow) implementation across the same inputs, plus a flood-shaped
input completing within a wall-clock budget in `caps.rs`.

**Alternatives considered**:
- *Enforce once per `apply` batch instead of per push* — complementary, adopted in
  R5 (the processor can call a single `push` per coalesced span), but the buffer
  must still be correct per-push for callers; incremental counting makes per-push
  cheap, so no special batching contract is needed.
- *Rope/segmented buffer* — rejected: over-engineered for a tail-retention buffer
  (Principle VII).

---

## R4 — Passthrough: no spurious input, clean restore (cluster C: FR-012, FR-013)

**Problem**: Launching `vi` injects `]11;rgb:2020/2020/2020` (an OSC 11
*background-color report*) as keystrokes, and the terminal is not restored after
`vi`/`bpytop` exit.

**Analysis**: This is the terminal **query/response round-trip**. On startup an
alt-screen program writes an OSC 11 *query* (`ESC ] 11 ; ? BEL`). In passthrough
that query is written to the host terminal (correct). The host terminal answers on
**kapollo's stdin** (the same channel as the keyboard). Today `drain`-side key
handling reads those bytes as `KeyEvent`s and `encode_key` re-encodes them into
the visible `]11;rgb:...` garbage sent back to the program. So the spurious input
is kapollo mangling the terminal's reply instead of forwarding it verbatim.

**Decision**:
- **Forward raw stdin verbatim in passthrough.** While `passthrough == true`, read
  raw bytes from stdin and write them straight to the PTY rather than going through
  crossterm `KeyEvent` decoding + `encode_key`. This delivers terminal
  query/responses (OSC 11/10/4, DA, cursor-position) back to the program intact and
  also fixes any other key-encoding lossiness. Implementation: in passthrough,
  use crossterm's raw byte source (read available bytes) and `shell.write_input`
  them unchanged; keep the structured `KeyEvent` path only for the normal split-pad
  mode.
- **Deterministic restore on alt-screen leave (FR-013).** On the `passthrough →
  normal` transition, emit an explicit reset sequence to stdout *before* the next
  ratatui frame: SGR reset (`ESC[0m`), show cursor, ensure the host is on
  kapollo's screen, then `terminal.clear()` + full redraw. Because kapollo itself
  holds the host alt-screen (TerminalGuard), re-assert that state so a program that
  toggled `?1049h/l` cannot leave the host in the program's saved buffer.

**Rationale**: Verbatim stdin forwarding is the standard way a terminal multiplexer
hands a child the real terminal; re-encoding via `KeyEvent` is lossy by design
(it can't represent arbitrary response bytes). The explicit reset guarantees a
clean visual handoff regardless of what the program left on the screen.

**Risk / follow-up**: crossterm consumes stdin through its event reader; mixing a
raw byte read with the event stream needs care. Approach: when entering
passthrough, switch to reading raw bytes (bypassing the event reader); when
leaving, resume the event reader. This is covered by a passthrough integration
test and a manual `vi`/`bpytop` validation step (quickstart), since full fidelity
needs a real TTY (Constitution III exception).

**Alternatives considered**:
- *Detect and specifically route only OSC responses back to the PTY* — rejected:
  fragile allow-list; verbatim forwarding handles every query type uniformly.

---

## R5 — Bounded, interruptible event loop (cluster D: FR-015, FR-017)

**Problem**: `drain_shell()` empties the entire channel backlog per loop pass;
under a flood (made worse by R3's old O(n²)) the loop rarely returns to
`event::poll`, so Ctrl-C is not serviced.

**Decision**: Bound work per pass and check input between batches.
- Cap each `drain_shell` pass at a **byte budget** (e.g. ~256 KiB) and/or a
  **chunk count** (e.g. 64 `PtyEvent`s); return to the top of the loop afterward so
  `event::poll` + render run, then continue draining next pass.
- Keep `POLL_INTERVAL` small; the bounded drain guarantees the loop reaches
  `event::poll` within a small bounded delay even mid-flood, so a Ctrl-C `KeyEvent`
  is read and `send_interrupt()` is forwarded promptly (FR-017).
- Combined with R3's truncated fast-path, the per-pass cost is bounded regardless
  of flood size.

**Rationale**: Cooperative yielding keeps the single-threaded loop (the panic
boundary, research R1 of 001) responsive without adding threads. The PTY reader
thread continues filling the channel; the bounded consumer plus truncated discard
(FR-016) prevents unbounded buffering from translating into unbounded per-pass
work.

**Alternatives considered**:
- *Separate input thread / select across channels* — rejected: adds concurrency
  and a second mutator of UI state, violating the single-loop invariant and
  Principle VII. The bounded-drain approach is sufficient.
- *Bounded `mpsc::sync_channel`* — considered; not required once the consumer is
  bounded and truncated blocks discard. May be revisited if memory pressure is
  observed, but out of scope now.

---

## R6 — cwd tracking via OSC 7 (cluster E: FR-019)

**Problem**: The status cwd does not follow `cd`.

**Decision** (per D23): the shell hook emits **OSC 7**
(`ESC ] 7 ; file://<host>/<abs-path> ST`) from the same prompt hook that already
emits OSC 133, and the vte `osc_dispatch` parses param `7`, percent-decodes the
path component of the `file://` URI, and surfaces a `Boundary`/event the `App`
applies to a `cwd: PathBuf` field rendered on the status rule.

- **fish**: add to the existing `--init-command` a `fish_prompt`/`fish_preexec`
  emission, or simplest: emit OSC 7 in a `--on-event fish_prompt` function using
  `$PWD`.
- **bash**: add OSC 7 emission to `PROMPT_COMMAND` in the generated rcfile (it
  already runs before each prompt), using `$PWD`.
- **Sentinel/Other shells**: no OSC 7; cwd stays at last known (acceptable, D23).
- Initialize `cwd` from `std::env::current_dir()` at startup so it is correct
  before the first prompt.

**Rationale**: Authoritative, shell-reported, reuses the existing hook+parser
channel (D12/D19); no prompt scraping (which D2/D23 reject as theme-fragile).

**Alternatives considered**:
- *Parse `cd` from submitted input* — rejected: misses `pushd`, `z`, subshells,
  and scripted directory changes; not authoritative.
- *`/proc/<pid>/cwd` of the shell* — rejected: needs the child PID and races with
  command execution; OSC 7 is simpler and already the decided mechanism.

---

## R7 — Chrome layout & color (cluster B/E: FR-005–FR-011, FR-018, FR-023)

**Decision**:
- **Layout**: replace the three-region bordered layout with `Layout::vertical`
  `[Min(1) transcript, Length(1) status-rule, Length(input_height) input]` — the
  single status **rule** sits *above* the input (matching the user-test mock).
  Drop the transcript border (FR-005) and the input box + "input" label (FR-006);
  the input area renders bare with the `λ` cursor prompt.
- **Status rule (FR-007/008)**: a one-line horizontal rule containing the cwd
  (from R6) and, only when the last exit code is non-zero, ` exit: <code> `.
- **Blank line between blocks (FR-009)**: transcript builder appends `\n` after
  each block's output.
- **`λ` prompt (FR-010)**: each echoed command line is prefixed with the
  configured `prompt_char` (default `λ`) instead of `$`.
- **Color (FR-011/018/023)**: reuse the existing `ui::color_enabled()` (already
  honors `NO_COLOR`). The `λ` prefix is styled with `prompt_color` (default red)
  only when `color_enabled()`. Note the user-test observed *no color at all even
  without `NO_COLOR`* — the fix is to actually apply styled `Span`s in the
  transcript/status (today the transcript builds a plain `String`; it must build
  styled `Line`/`Span`s so color renders). Config gains `prompt_char: String`
  (single char) and `prompt_color: String` (named color, parsed to
  `ratatui::style::Color`), with defaults applied when absent and unknown values
  warned-and-defaulted (mirroring existing `leader_char`/caps handling).

**Rationale**: Minimal layout change, removes the corruption-prone border,
matches the requested look, and makes color actually take effect by moving the
transcript from a flat `String` to styled lines.

**Alternatives considered**:
- *Keep `String` and post-style* — rejected: ratatui needs styled spans to color;
  a flat string cannot carry per-span color.

---

## R8 — Keyboard scrolling (cluster E: FR-021, FR-022)

**Decision**: Bind in `App::on_key` (normal mode): `PageUp` →
`transcript.scroll_up(n)`, `PageDown` → `scroll_down(n)` (n = viewport height − 1
for a page, or a fixed step), `Home` → scroll to oldest (offset = max), `End` →
scroll to newest (offset = 0). The user test reported PgUp/PgDn "didn't work"; the
existing handlers call `scroll_up(1)`/`scroll_down(1)` — verify the offset is
actually applied in `transcript::render` (it is read as `scroll_offset()`), and
fix the off-by/clamping so visible movement occurs. List all four keys in
`/help` (FR-022). Mouse capture remains out of scope (D24).

**Rationale**: Reuses existing `Transcript::scroll_up/down/offset`; only binding
and clamping fixes plus `Home`/`End` jumps are new. Keeps native terminal
selection intact (D24 rationale).

**Alternatives considered**: mouse-wheel capture — explicitly deferred (D24).

---

## Constitution III exception (carried from 001)

PTY/terminal interactive behavior (passthrough fidelity, OSC 7 round-trip,
interrupt-under-flood, real `vi`/`bpytop` restore) cannot be fully unit-tested in
isolation. These are covered by integration/smoke tests against a headless PTY
harness plus the manual validation steps in [quickstart.md](quickstart.md). This
is the documented Principle III exception.

## Summary of decisions

| # | Decision | Primary FRs |
|---|----------|-------------|
| R1 | Normalize output in the vte parser (drop CR/OSC/residual escapes) | FR-001, FR-004 |
| R2 | Borderless transcript + clean buffer ownership | FR-002, FR-003, FR-005 |
| R3 | Incremental line count + bulk `drain` + tail fast-path | FR-014, FR-016 |
| R4 | Verbatim stdin forwarding in passthrough + explicit restore | FR-012, FR-013 |
| R5 | Bounded per-pass drain; prompt interrupt servicing | FR-015, FR-017 |
| R6 | OSC 7 cwd from fish/bash hooks, parsed in vte layer | FR-019 |
| R7 | New layout (rule above input), styled spans, `λ`/color config | FR-005–011, 018, 023 |
| R8 | Keyboard scroll bindings (PgUp/PgDn/Home/End) + `/help` | FR-021, FR-022 |
