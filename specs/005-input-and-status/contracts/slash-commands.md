# Contract: Slash commands `/status` and `/keys`

**Feature**: 005-input-and-status | Internal interface contract
**Modules**: `src/slash/mod.rs`, `src/slash/builtins.rs`, `src/app.rs`

Covers FR-022, FR-030, FR-031. Extends the existing `SlashCommand` registry
(`Help`, `Clear`, `Quit`) without regressing it (FR-032).

---

## Registry (`src/slash/mod.rs`)

```rust
enum SlashCommand { Help, Clear, Quit, Status, Keys }   // + Status, Keys

fn dispatch(command: &str) -> Option<SlashCommand>;
//  "help"          → Help
//  "clear"         → Clear
//  "quit" | "exit" → Quit
//  "status"        → Status   (NEW)
//  "keys"          → Keys     (NEW)
```

**Contract**: existing command strings keep their meaning exactly (FR-032).
Unknown commands behave as before (existing not-found handling).

## `/status` (FR-022)

**Effect**: toggles `App.status_enabled` between on and off. When toggled off, the
status bar is not rendered (even at ≥ 10 rows); when on, it renders subject to the
`<10`-row auto-hide (FR-021). A confirmation is surfaced via the status **message**
(e.g. `status bar: off`) — itself subject to the `<10`-row hide.

## `/keys` (FR-030)

**Effect**: lists the **active hardcoded** key map by action and binding, sourced
from `action::listing()` (see [input-editing contract](input-editing.md)). Output is
rendered into the transcript like `/help`. Listing is **stable-ordered** and includes
every mapped action (motion, selection, kill, scroll, `clear_status_message`); reserved
unmapped actions (`multiline_move_start_buffer`/`_end_buffer`) are **not** listed as
bound (they have no binding; FR-009) — they may be shown as "(unbound)" or omitted.

**Example shape** (illustrative, not asserted verbatim):

```text
Action                Key
line_move_start       Home
line_move_end         End
word_move_left        Ctrl+Left
…
scroll_page_up        PageUp
scroll_to_top         Shift+Home
clear_status_message  Esc Esc
```

## `/help` pointer (FR-031)

**Contract**: `help_text` (in `builtins.rs`) gains a **one-line** pointer to `/keys`
(e.g. `/keys — list the current key bindings`). Existing help content is preserved.

**Test seam**: `tests/slash_status_keys.rs` — `dispatch` resolves `status`/`keys`
(and existing commands unchanged); `/status` flips `status_enabled`; `/keys` output
includes every mapped binding from `action::listing()`; `help_text` contains the
`/keys` pointer.
