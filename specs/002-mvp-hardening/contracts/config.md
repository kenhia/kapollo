# Contract Delta: `config.toml` — prompt character & color

Extends [specs/001-mvp-repl/contracts/config.md](../../001-mvp-repl/contracts/config.md).
Only the additions for this sprint are specified here (FR-010, FR-011, FR-023).

## New top-level keys

```toml
# ~/.config/kapollo/config.toml

# Character echoed before each submitted command in the transcript.
# Exactly one character. Default: "λ".
prompt_char = "λ"

# Color applied to prompt_char when color is enabled (NO_COLOR unset).
# A named color. Default: "red".
prompt_color = "red"
```

## Rules

- **`prompt_char`**
  - Type: string containing **exactly one** character (same rule as `leader_char`).
  - Default when absent: `λ`.
  - More than one character → `ConfigError::Value` (consistent with `leader_char`).
- **`prompt_color`**
  - Type: string naming a color, parsed to a `ratatui::style::Color`.
  - Accepted values: the standard named colors (`black`, `red`, `green`, `yellow`,
    `blue`, `magenta`, `cyan`, `gray`/`grey`, `white`, and their `dark*`/bright
    variants as supported by the parser). Case-insensitive.
  - Default when absent: `red`.
  - Unknown/unparseable value → warn (`tracing::warn!`) and fall back to the
    default; never a hard error (consistent with the lenient-config posture).
- Both keys are added to the known-key allow-list so they are **not** reported as
  "unknown config key ignored".
- `NO_COLOR` (any non-empty value) suppresses the prompt color regardless of
  `prompt_color`; the `prompt_char` is still rendered, just unstyled (FR-018).

## Examples

Minimal (all defaults — `λ` in red):

```toml
# (no prompt_* keys needed)
```

Custom:

```toml
prompt_char = "❯"
prompt_color = "cyan"
```

Invalid `prompt_char` (rejected):

```toml
prompt_char = ">>"   # error: must be exactly one character
```
