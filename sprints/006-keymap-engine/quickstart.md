# Quickstart: Configurable Keymap Engine — manual validation

A live-TTY walkthrough validating the sprint-006 success criteria (SC-001…006).
Pure logic (parsing, the default→effective overlay, conflicts, per-mode
inheritance) is covered by `cargo test`; this script covers the **live** behaviors
that the Constitution III integration/manual exception applies to: a rebound key
actually firing, `/keys` reflecting the live map, and `/reload-config` applying
edits without disrupting input.

**Build**: `cargo build` then run `./target/debug/kap` (or `cargo run`).

Use a scratch config so your real one is untouched:

```sh
mkdir -p /tmp/kap-006
kap --config /tmp/kap-006/config.toml
```

---

## 0. Baseline: identical defaults (SC-001)

1. Start kapollo with an **empty** `/tmp/kap-006/config.toml` (or no `[keymap]`).
2. Confirm every sprint-004/005 key still works: `Home`/`End` (line motion),
   `Ctrl+Left`/`Ctrl+Right` (word motion), `Shift+Left`/`Shift+Right` (selection),
   `Ctrl+U`/`Ctrl+K`/`Ctrl+W` (kills), `PageUp`/`PageDown` and
   `Shift+PageUp`/`Shift+PageDown` (scroll), `Shift+Enter`/`Alt+Enter` (newline).
   → **Expected**: behavior unchanged from sprint 005. *(SC-001)*

## 1. `/keys` reflects the live map (SC-003)

3. Run `/keys`.
   → **Expected**: every named action is listed with its key, including the two
   copy variants (`copy_current_line`, `copy_block_without_command`) with default
   bindings, `insert_newline` showing **both** `Shift+Enter` and `Alt+Enter`, and
   the `Esc Esc` gesture row for `clear_status_message`. *(SC-003, FR-005, FR-004,
   FR-014)*

## 2. Copy variants have working default bindings (FR-005)

4. With a few commands in the transcript, focus a block/line and press the default
   copy-current-line key (e.g. `Ctrl+Y`); paste elsewhere.
   → **Expected**: the focused line is on the clipboard.
5. Press the default copy-block-without-command key (e.g. `Alt+Y`); paste.
   → **Expected**: the block's output without its command line is on the
   clipboard. *(FR-005)*

## 3. Rebind an action (SC-002)

6. Quit. Edit `/tmp/kap-006/config.toml`:

   ```toml
   [keymap]
   word_move_left  = "Ctrl+B"
   word_move_right = "Ctrl+F"
   ```

7. Restart kapollo. Type a few words, then press `Ctrl+B` / `Ctrl+F`.
   → **Expected**: the caret moves by word — the new keys drive `word_move_left`/
   `word_move_right`. Press the old `Ctrl+Left`/`Ctrl+Right`.
   → **Expected**: they no longer move by word (rebind replaced the default).
   *(SC-002, FR-001, FR-011 interaction)*

## 4. Primary + alternate (FR-003)

8. Add to config and restart:

   ```toml
   [keymap]
   insert_newline = ["Shift+Enter", "Ctrl+J"]
   ```

9. Press `Shift+Enter`, then `Ctrl+J`.
   → **Expected**: both insert a newline without submitting. *(FR-003)*

## 5. Disable-by-clearing (FR-011)

10. Add and restart:

    ```toml
    [keymap]
    kill_to_line_start = ""
    ```

11. Type text mid-line and press `Ctrl+U`.
    → **Expected**: nothing is killed — the binding is disabled. *(FR-011)*

## 6. Case-insensitive, order-free key strings (SC-006)

12. Add and restart:

    ```toml
    [keymap]
    word_move_left = "shift+CTRL+left"
    ```

13. Run `/keys`.
    → **Expected**: `word_move_left` is listed (the spelling parsed regardless of
    case/order); pressing `Shift+Ctrl+Left` triggers it. *(SC-006, FR-007)*

## 7. Invalid binding never blocks startup (SC-004)

14. Add and restart:

    ```toml
    [keymap]
    word_move_left = "Control+Nope"   # unknown modifier + unknown key
    ```

15. → **Expected**: kapollo **starts normally**; the log shows a warning naming
    the bad `word_move_left` binding; `word_move_left` falls back to unbound/its
    default per the override semantics. Run `kap --verbose` if needed to see the
    warning. *(SC-004, FR-009)*

## 8. Conflict is warn + last-wins (FR-010)

16. Add and restart:

    ```toml
    [keymap]
    word_move_left  = "Ctrl+G"
    scroll_line_up  = "Ctrl+G"
    ```

17. → **Expected**: kapollo starts; the log warns of the `Ctrl+G` conflict naming
    both actions; `Ctrl+G` triggers the **last-declared** action (`scroll_line_up`).
    *(FR-010, SC-004)*

## 9. `/reload-config` applies edits live, without disrupting the session (SC-005)

> Note: a slash command occupies the whole input line — kapollo routes input to a
> slash command only when the entire submitted line begins with the leader char.
> So there is no separate "in-progress buffer" coexisting with `/reload-config` at
> the moment you run it; FR-017 means the reload must not disrupt the surrounding
> composing state (input history, transcript, and any active selection).

18. Start clean. Run a couple of commands so the transcript has content, then
    press `Up` to confirm input-history recall works (then clear the line).
19. In another terminal, edit `/tmp/kap-006/config.toml`:

    ```toml
    [keymap]
    word_move_left = "Ctrl+B"
    ```

20. Back in kapollo, run `/reload-config`.
    → **Expected**: a confirmation block appears; the **transcript and input
    history are intact** (press `Up` again — your prior inputs still recall) and
    the input pad is clear and usable; `Ctrl+B` now moves by word; `/keys` shows
    the new binding. *(SC-005, FR-015, FR-017)*

## 10. Malformed reload keeps the old map (FR-016)

21. Edit the config to be invalid TOML (e.g. `word_move_left = [` ).
22. Run `/reload-config`.
    → **Expected**: an error block appears; the **previous** keymap stays in
    effect (the binding from step 20 still works); the session continues normally
    (transcript and input history intact).
    *(FR-016, FR-017, SC-005)*

---

## Success-criteria coverage

| Criterion | Steps |
|-----------|-------|
| SC-001 identical defaults | 0 |
| SC-002 rebind via config | 3 |
| SC-003 every action discoverable via `/keys` | 1 |
| SC-004 invalid/conflict never blocks startup | 7, 8 |
| SC-005 live reload, never broken | 9, 10 |
| SC-006 case/order-insensitive key strings | 6 |
| FR-003 primary + alternate | 4 |
| FR-005 copy variants bound | 2 |
| FR-011 disable-by-clearing | 5 |

Record pass/fail per step; any failure blocks the sprint's definition of done.
