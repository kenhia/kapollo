# Contract: Fixed-format Status Bar

**Feature**: 005-input-and-status | Internal interface contract
**Modules**: `src/ui/status.rs`, `src/ui/mod.rs` (layout), `src/app.rs` (state),
`src/config.rs`

Covers US4 + US5 message lifetime (FR-018–FR-026). See [research.md](../research.md)
R3 (fold the above-input rule into this bar) and R4 (fit/truncate).

---

## Pure layout (`src/ui/status.rs`)

```rust
/// Fit the fixed layout `mode | cwd<greedypad>| message | exit` into `width`
/// columns, returning exactly one line of ≤ width columns (FR-019/FR-024).
fn fit(
    width: usize,
    mode: &str,           // 4-char field; default shell mode is the literal "norm" (FR-020)
    cwd: &str,
    message: Option<&str>,
    exit: Option<i32>,
) -> String;
```

**Contract** (research R4):
- Layout is `mode | cwd<greedypad>| message | exit` with the 4-char mode field and
  its `" | "` separator always present; a greedy pad sits between `cwd` and the `|`
  (**no `|` *immediately* after cwd**; FR-019); `message`
  is **right-justified** against the exit field.
- The `exit` field is laid out from the right and is shown only when `exit` is
  `Some` (FR-023); it is **never** broken or wrapped (FR-024).
- Under width pressure, truncate in this order: `message` (ellipsis `…`) → `cwd`
  (middle-ellipsis, keep the trailing path component) → never touch `mode`/`exit`,
  **never** wrap to a second row (FR-024).
- Output is always a single line of at most `width` columns (no flicker; Constitution
  VI).

## Visibility (`src/ui/mod.rs` layout)

```text
show_status_bar = config.status.enabled && terminal_rows >= 10   (FR-018/FR-021)
```

**Contract**: the bar occupies one row directly beneath the input pad when shown;
auto-hidden below 10 rows **regardless** of `enabled`, reappearing at ≥ 10 (FR-021).
When hidden, its row is reclaimed by the transcript/input area (no blank gap).

## State (`src/app.rs`)

| Field | Source | Notes |
|-------|--------|-------|
| `status_enabled` | `config.status.enabled` (default true), toggled by `/status` | FR-018/FR-022 |
| mode | constant `norm` (4-char) this sprint | FR-020 |
| cwd | `App.cwd` | FR-023 |
| message | `App.notice` (reused) — see lifetime below | FR-019/FR-025 |
| exit | most-recent completed command exit (`App.last_exit`) | FR-023 |

**Message lifetime** (FR-025/FR-026 — **no timeout**):
- Set when any subsystem posts a message (e.g. copy failure).
- Cleared on the next `Enter` submit (FR-025).
- Cleared on `Esc Esc` (`clear_status_message`, FR-026).

## Config additions (`src/config.rs`) {#config}

| Key | Type | Default | Notes |
|-----|------|---------|-------|
| `status.enabled` | bool | `true` | New `[status]` table; on/off (FR-018). |
| `scroll.context_lines` | u16 | `3` | Added to existing `[scroll]` table (FR-014). |

Existing keys (`shell`, `leader_char`, `prompt_char`, `prompt_color`, `[caps]`,
`[mouse]`, `[clipboard]`, `scroll.wheel_lines`, `scroll.scrollback_lines`) are
**unchanged** (FR-033). Unknown keys logged & ignored (existing policy).

**Test seam**: `tests/status_bar.rs` — `fit()` across widths (greedy pad, message
right-justify, message-then-cwd truncation order, mode/exit never broken), the
`<10`-row hide predicate, `/status` toggle, and message lifetime (set → persist
across non-submit → clear on Enter / Esc Esc). Resize/render verified in quickstart.
