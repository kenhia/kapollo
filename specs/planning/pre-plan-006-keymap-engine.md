# Pre-plan 006 — Configurable keymap engine

> Source: `.scratch/005-pre-planning.md`. Sprint 005 ships its keys hardcoded;
> this sprint makes the whole key map configurable and folds those bindings into
> the engine.

## Goal

Make every key binding configurable, with a sane default map, so users (and
future modes) can rebind actions — and so legacy-terminal fallbacks become a
config concern rather than a code change.

## In scope

- **Key-string parser** — parse human-writable bindings (e.g. `Ctrl+Left`,
  `Shift+PageUp`, `Alt+Enter`) into the internal key representation.
- **Action registry** — every behavior from sprint 005 (and existing 004 keys)
  exposed as a *named action* (e.g. `input.move_word_left`,
  `scrollback.line_up`, `selection.copy`, `app.interrupt`).
- **Default + alternate binding per action.** Each action supports a primary and
  an alternate key (the easy path to legacy-terminal fallbacks — e.g.
  `Shift+Enter` primary, `Alt+Enter` alternate for "insert newline").
- **Config schema + validation** — a `[keymap]` (or similar) table; validate key
  strings; detect and report **conflicts** (two actions bound to the same key).
- **`/keys` reflects the live config** — shows the effective map (replacing the
  hardcoded listing from 005).
- **Bind the previously-unbound copy variants** — `copy_block_without_command`
  and `copy_current_line` get default bindings (they exist in code since 004 but
  had no key path).
- **Disable-by-clearing** — clearing an action's binding in config disables it
  (so opinionated defaults like `Ctrl+U`/`Ctrl+K`/`Ctrl+W` can be turned off).

## Decisions (resolved in pre-planning)

- Direction is **"document and configure ALL key mappings"** with a
  default+alternate slot per action.
- Sprint 005 ships hardcoded; this sprint replaces that with the engine, keeping
  the same defaults so behavior is unchanged out of the box.

## Out of scope

- New end-user *behaviors* (this is plumbing for existing actions).
- Mouse-binding configuration (keys first; revisit mouse later if wanted).

## Open questions

- **Q1 — Config format/location.** TOML table shape, per-mode sections (does the
  keymap vary by mode, anticipating LAAT in 007?), and how alternates are
  expressed (array `["Shift+Enter", "Alt+Enter"]` vs. explicit `alt =`).
    - Keymap can vary by mode
    - array `["Shift+Enter", "Alt+Enter"]`
- **Q2 — Conflict policy.** Hard error and refuse to start, or warn + last-wins?
    - Warn + last-wins
- **Q3 — Key-string grammar.** Canonical modifier order and names
  (`Ctrl`/`Control`, `Cmd`/`Super`), case sensitivity, chord support (multi-key
  sequences like `Esc Esc`) — needed because 005 introduces double-`Esc`.
    - Canonical (prefer short, "Ctrl" over "Control"), case insensitive.
    - For now at least, I think the only "chord" we should support is the `Esc Esc`
- **Q4 — Reload.** Live reload on config change, or restart-only?
    - Live reload, but on demand, not file watching. `/reload-config` or similar

## Dependencies / sequencing

- **Depends on 005** (the named action set must exist).
- **Enables 007** if LAAT wants per-mode bindings.
