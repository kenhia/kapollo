# Phase 1 Data Model: kapollo MVP Hardening

This sprint changes behavior and a few field/accessor shapes; it does **not**
introduce new top-level entities. Below are the deltas to existing entities
(see [specs/001-mvp-repl/data-model.md](../001-mvp-repl/data-model.md) for the
baseline). Each delta cites the driving requirement and research decision.

---

## OutputBuffer (`src/session/ringbuf.rs`) — CHANGED

The capped, head-dropping byte buffer for block output. Cap **semantics** are
unchanged (D14); the **enforcement algorithm** becomes amortized O(1) (R3).

| Field | Type | Change | Notes |
|-------|------|--------|-------|
| `bytes` | `VecDeque<u8>` | unchanged | Retained tail of output. |
| `cap_bytes` | `u64` | unchanged | 0 disables the byte cap. |
| `cap_lines` | `u64` | unchanged | 0 disables the line cap. |
| `truncated` | `bool` | unchanged | Set once data is dropped. |
| `line_count` | `u64` | **NEW** | Running count of `\n` in `bytes`, maintained incrementally (FR-014). |

**Behavioral changes**:
- `push(data)`:
  - Increment `line_count` by the number of `\n` in `data` (count the slice, never
    rescan the buffer).
  - **Tail fast-path (FR-016)**: if `cap_bytes > 0` and `data.len() >= cap_bytes`,
    replace the buffer with the last `cap_bytes` of `data` (recompute `line_count`
    for that tail), set `truncated = true`, then apply the line cap. This bounds
    per-call work to `cap_bytes` regardless of flood size.
  - Otherwise append, then bulk-trim.
- `enforce_caps()`:
  - Byte cap: `overflow = len - cap_bytes`; `drain(..overflow)` once; subtract the
    `\n` count of the drained prefix from `line_count`; set `truncated`.
  - Line cap: while `line_count > cap_lines`, bulk-`drain` up to and including the
    next `\n`; decrement `line_count`; set `truncated`.

**Invariant (test target)**: for any input sequence, `byte_len()`, `truncated()`,
and the retained byte content equal the previous (correct) implementation's
output. Verified in `tests/caps.rs`.

---

## Block (`src/session/block.rs`) — CHANGED (accessor only)

| Member | Change | Notes |
|--------|--------|-------|
| `output` | type unchanged (`OutputBuffer`) | Now stores **normalized** text (R1), because normalization happens upstream in the parser before `push_output`. |
| `output_lossy()` | unchanged signature | Returns the (now already-normalized) captured text; no longer expected to contain bare `\r`, OSC responses, or residual escapes (FR-001/004). |

No new fields. The privacy/save-output reserved fields (D13) are untouched.

---

## OutputProcessor / Parser (`src/output/`) — CHANGED

**Boundary enum** gains a cwd event so OSC 7 can flow to the `App` (R6):

| Variant | Change | Carries |
|---------|--------|---------|
| `Boundary::Cwd(PathBuf)` | **NEW** | Absolute path decoded from an OSC 7 `file://host/path` URI (FR-019). |

**Performer normalization (R1)** — the `Output` event stream now contains only
printable text plus `\n`/`\t`:
- `\r\n` → `\n`; lone `\r` dropped; other C0 controls dropped.
- All OSC payloads consumed by `osc_dispatch` (133 handled, 7 → `Cwd`, others
  swallowed); never emitted as `Output`.
- CSI/ESC/DCS consumed; only alt-screen CSI produces a `Boundary` (unchanged);
  nothing emitted as visible text.

`OutputProcessor::apply(...)` returns the existing `Vec<Boundary>`; the `App`
handles the new `Cwd` variant by updating chrome state (below). Capture/state
machine for OSC 133 and alt-screen is otherwise unchanged.

---

## Config (`src/config.rs`) — CHANGED (new keys)

Two new optional keys with defaults applied when absent (FR-010/011/023). Parsing
mirrors the existing `leader_char`/caps pattern: unknown values are warned and
defaulted; `prompt_char` must be exactly one character.

| Key (TOML) | Type | Default | Maps to | Notes |
|------------|------|---------|---------|-------|
| `prompt_char` | string (1 char) | `"λ"` | `Config::prompt_char: char` | Echoed before each command instead of `$` (FR-010). |
| `prompt_color` | string (named) | `"red"` | `Config::prompt_color: Color` | Applied to the prompt char when color is enabled (FR-011). Parsed to `ratatui::style::Color`; unknown → default + warn. |

`TOP_LEVEL_KEYS` extended with `prompt_char`, `prompt_color` so they are not
warned as unknown. `Config` struct gains `prompt_char: char` and
`prompt_color: Color` (or a small `PromptStyle`), with `Default` = (`'λ'`, red).

---

## Chrome / Status state (`src/ui/status.rs`, `src/app.rs`) — CHANGED

The data rendered on the single status rule (R7). No new persistent entity; this
is `App` state surfaced to the renderer.

| State | Type | Source | Render rule |
|-------|------|--------|-------------|
| `cwd` | `PathBuf` | `current_dir()` at startup; updated by `Boundary::Cwd` (OSC 7, R6) | Always shown on the rule (FR-007). |
| `last_exit` | `Option<i32>` | existing, set on `CommandEnd` | Shown **only when non-zero** (FR-008). |

`App` gains a `cwd: PathBuf` field initialized from `std::env::current_dir()`.

---

## UI layout (`src/ui/mod.rs`, `transcript.rs`) — CHANGED

- Layout becomes `[Min(1) transcript, Length(1) status-rule, Length(input_height)
  input]`; the status **rule sits above the input** (R7). `input_height` no longer
  needs `+2` for a border.
- Transcript renders **borderless** styled `Line`/`Span`s (not a flat `String`) so
  color applies (R7), with a blank line between blocks (FR-009) and the `λ` prefix
  styled with `prompt_color` (FR-010/011).
- The renderer owns the full transcript rectangle; no out-of-band writes occur
  during normal mode (R2/R4).

---

## Relationships (unchanged)

```text
Config ──(prompt_char/color, caps, leader)──▶ App
App ──owns──▶ Transcript ──contains──▶ Block ──has──▶ OutputBuffer (normalized, capped)
PtySession(reader thread) ──PtyEvent::Output──▶ App.drain_shell (bounded)
  └─▶ OutputProcessor.apply ─▶ Boundary{OutputStart|CommandEnd|AltScreen…|Cwd}
                              └─▶ ProcessorEvent::Output(normalized) ─▶ Block.push_output
App.cwd ◀── Boundary::Cwd (OSC 7)      App.last_exit ◀── Boundary::CommandEnd
```

## Test coverage map

| Entity/Change | Test file | Asserts |
|---------------|-----------|---------|
| OutputBuffer incremental/bulk/fast-path | `tests/caps.rs` (extend) | parity with old impl; flood within time budget (FR-014/016) |
| Parser normalization | `tests/render_normalize.rs` (new) | CR/OSC-response/escape stripped; first-line parity (FR-001/004) |
| OSC 7 → cwd | `tests/cwd_osc7.rs` (new) | `file://` URI decoded to cwd update (FR-019) |
| Chrome layout/spans | `tests/chrome.rs` (new) | borderless; conditional exit; blank line; `λ` prefix (FR-005–010) |
| Passthrough | `tests/passthrough.rs` (extend) | no spurious input; clean restore (FR-012/013) |
| Scrolling | `tests/scrolling.rs` (new) | PgUp/PgDn/Home/End offsets (FR-021) |
| Config keys | `tests/config.rs` (extend) | prompt_char/color defaults + parse + unknown-warn (FR-023) |
