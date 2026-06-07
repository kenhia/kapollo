# Quickstart: LAAT Mode, `/save`, `/filter`, and `/load`

**Feature**: 007-laat-mode | **Date**: 2026-06-07 | **Phase**: 1

A manual, interactive validation script for the parts that require a live TTY
(Constitution III integration/manual exception). Pure-logic behavior (mode
transitions, the LAAT gating function, history stash, snapshot save/restore, slash
dispatch, keymap defaults) is covered by `cargo test`; this script validates the
end-to-end feel and the rendered affordances. Each step maps to success criteria
(SC-00x) and the driving requirements (FR-0xx).

## Setup

```bash
cd /home/ken/src/tools/kapollo
cargo build --release
./target/release/kapollo        # or `kap`
```

Have a scratch directory to write into and a couple of commands that exit non-zero
on demand (e.g. `false`, or `rg nope` in an empty dir).

## 1. `Mult` mode — caret motion, no buffer loss (SC-002, FR-008/009/012)

1. Type `echo one`, then press `Alt+Enter`. Mode field shows `Mult`.
2. Type `echo two`. Press `Up`. ✅ The caret moves to line 1 (`echo one`) — the
   buffer is **not** discarded (the 005 sharp edge is gone).
3. Edit line 1, press `Down` back to line 2.
4. Press `Backspace` until only one line remains. ✅ Mode field returns to `norm`.

## 2. Chat-style edge recall (SC-003, FR-010/011)

1. Submit `echo alpha` so it enters history. Type a fresh `echo draft`, `Alt+Enter`,
   `echo more` (mode `Mult`).
2. Put the caret on line 1, press `Up`. ✅ History recalls `echo alpha`; your draft
   is stashed.
3. Press `Down` past the newest entry. ✅ Your stashed `echo draft\necho more` is
   restored byte-for-byte.

## 3. LAAT step-through with exit-code gating (SC-001, FR-001…005)

1. With a multi-line buffer (e.g. `echo a`, `Alt+Enter`, `echo b`, `Alt+Enter`,
   `echo c`), press `Ctrl+1`. ✅ Mode field shows `1T`; line 1 is highlighted.
2. Press `Enter`. ✅ `echo a` runs; on exit 0 the highlight advances to line 2.
3. Replace a line with `false` (a non-zero exit). Highlight it, press `Enter`.
   ✅ The highlight **stays** and that line's background flags a probable failure
   (FR-004).

## 4. LAAT failure recovery (FR-006)

From the failed line in step 3:
1. Press `Down` then `Enter` to treat the non-zero exit as success. ✅ Highlight
   advances past it.
2. Or press `Enter` to re-run; on exit 0 ✅ the flag clears and the highlight
   advances.
3. Or press `Esc Esc`. ✅ Mode returns to `norm` and the LAAT buffer is cleared
   (FR-007).

## 5. Push/pop mid-sequence (SC-006, FR-018/019/020)

1. In a `Mult`/`Laat` buffer, press `Ctrl+Alt+Enter`. ✅ Mode drops to `norm`, the
   pad is empty.
2. Run an ad-hoc command (e.g. `pwd`). ✅ On submit, your buffer and mode are
   restored exactly (including a `Laat` highlight if you pushed from `Laat`).
3. Push again while already pushed (before submitting). ✅ It is a no-op; your first
   saved state is preserved.

## 6. `/save` the previous output (SC-004, FR-021…024)

1. Run a command that produces output (e.g. `ls -la`).
2. `/save` (no path). ✅ Status shows `'/save' requires path`; the buffer is not
   cleared.
3. `/save ./scratch/out.txt`. ✅ The file holds `ls -la`'s exact output.
4. `/save ./scratch/out.txt` again. ✅ Prompt: `File exists, [O]verwrite,
   [A]ppend, [C]ancel?` — try `A` (appends), then re-run and try `C` (file
   untouched).
5. `/clear`, then `/save ./scratch/x.txt` with no prior block. ✅ Status:
   `Save failed, previous buffer not found`.

## 7. `/filter` with chaining (SC-005, FR-025…027)

1. Run `ps -e` (or any multi-line output).
2. `/filter rg <something-present>`. ✅ A new block titled `/filter rg <...>` shows
   only matching lines.
3. `/filter wc -l`. ✅ It operates on the **previous filter's** output (chaining).
4. `/filter rg <nothing-matches>`. ✅ `last_exit` shows the non-zero code **and**
   the status shows `filter non-zero exit`.

## 8. `/load` a script into LAAT (FR-028)

1. Create `./scratch/demo.sh` with a few command lines (e.g. `echo step1`,
   `echo step2`).
2. `/load ./scratch/demo.sh`. ✅ Each line becomes a LAAT buffer line, mode `1T`,
   line 1 highlighted. Step through with `Enter`.
3. `/load ./scratch/missing.sh`. ✅ A status message reports the failure; you are
   not left in `Laat` with a partial buffer.

## 9. Discoverability & rebinding (SC-007, FR-029/030/031)

1. `/keys`. ✅ Lists `toggle_mult_laat` (`Ctrl+1`) and `push_input`
   (`Ctrl+Alt+Enter`).
2. `/help`. ✅ Lists `/save`, `/filter`, `/load`.
3. Rebind in your config, e.g.:
   ```toml
   [keymap]
   toggle_mult_laat = "Ctrl+Alt+2"
   push_input = "Ctrl+Alt+p"
   ```
   `/reload-config`, then `/keys`. ✅ The new bindings are listed and active.

## Pass criteria

All ✅ checks above hold, the mode field always reflects the true mode
(`norm`/`Mult`/`1T`), no buffer is ever silently lost (SC-002/003/006), and every
`/save` error path produces a clear message or prompt (SC-004). Run the gate before
sign-off:

```bash
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
```
