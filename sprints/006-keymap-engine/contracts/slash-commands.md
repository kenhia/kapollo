# Contract: Slash commands (`/reload-config`, live `/keys`)

The slash-command surface this sprint adds/changes. Routing in `src/slash`,
behavior in `src/app.rs`. Tested by the slash dispatch unit tests +
`tests/keymap_engine.rs` (listing) + the manual quickstart (live reload).

## `/reload-config` (new)

- **Routing**: `slash::dispatch("reload-config")` →
  `Dispatch::Command(SlashCommand::ReloadConfig)`. Add `ReloadConfig` to the
  `SlashCommand` enum. (Optional alias `/reload` MAY be added; primary name is
  `reload-config`.)
- **Behavior** (FR-015/FR-016/FR-017):
  - Re-read configuration from the **retained resolved config path**
    (`App::config_path`); re-apply the `--shell` override the same way startup
    does.
  - On **success**: rebuild `Keymaps`, swap `app.config` + `app.keymaps`, and emit
    a synthetic block confirming the reload.
  - On **failure** (malformed config): emit a synthetic block with the error and
    **keep** the previous `config` + `keymaps` (no partial application).
  - In **all** cases: do **not** touch `app.input` — an in-progress buffer and any
    active selection survive the reload (FR-017).
  - When the run has **no** config path (defaults only, no file), report that
    there is nothing to reload (or reload defaults) without error.

## `/keys` (changed: now live)

- **Behavior** (FR-014): list the **effective** keymap from
  `app.keymaps.for_mode(current_mode).listing()` instead of the static
  `action::listing()`. The output reflects configured overrides and the result of
  the most recent `/reload-config`, including primary + alternate keys and
  unbound actions.

## `/help` (changed)

- `builtins::help_text` lists `/reload-config` alongside the existing commands and
  keeps the `/keys` pointer for the full live binding list.

## Test obligations

- `dispatches_reload_config` — `slash::dispatch("reload-config")` resolves to
  `SlashCommand::ReloadConfig` (slash unit tests).
- `help_text_mentions_reload_config` — `help_text` includes `/reload-config`
  (builtins unit test).
- `keys_listing_reflects_effective_map` — covered by
  `tests/keymap_engine.rs::listing_reflects_effective_map_including_unbound`.
- Live reload (config edited on disk, `/reload-config` applies it without losing
  the in-progress buffer; a malformed edit reports + keeps the old map) — manual
  quickstart steps (Constitution III integration/manual exception; reload swaps
  data structures with no pure seam to assert in `cargo test`).
