# Contract: Configuration File

**Feature**: 001-mvp-repl

Format: TOML at `~/.config/kapollo/config.toml` (XDG; D15). All keys
optional — kapollo runs entirely on defaults when the file is absent
(FR-028).

## Schema

```toml
# Shell to wrap. Defaults to $SHELL when omitted.
shell = "/usr/bin/fish"

# Leader character that begins a slash command.
leader_char = "/"

[caps]
# Per-block output retention. Whichever limit is hit first applies.
# bytes hard maximum is 64 MiB; values above are clamped.
per_block_bytes  = 1048576    # 1 MiB
per_block_lines  = 50000

# Whole-transcript retention. Oldest blocks evicted first.
transcript_bytes  = 134217728 # 128 MiB
transcript_blocks = 1000
```

## Rules

- **Missing file**: all defaults applied (FR-028).
- **Missing key**: that key's default applied.
- **Unknown key**: logged at warn level and ignored — never fatal (R10).
- **Out-of-range cap**: clamped to the hard maximum (per-block bytes ≤
  64 MiB) (R6).
- **Invalid TOML**: kapollo fails to start with an actionable error naming
  the file and the parse problem (boundary validation; Constitution VII).
- `--config <PATH>` overrides the default location (see cli contract).

## Defaults (authoritative)

| Key | Default |
|-----|---------|
| `shell` | `$SHELL` |
| `leader_char` | `/` |
| `caps.per_block_bytes` | 1 MiB (1048576) |
| `caps.per_block_lines` | 50000 |
| `caps.transcript_bytes` | 128 MiB (134217728) |
| `caps.transcript_blocks` | 1000 |

## Out of MVP scope

Key-binding remaps, color/theme config, history persistence, AI/DB sections
(post-MVP). The schema is namespaced (e.g. `[caps]`) so these add cleanly
later without disturbing base keys (D15).

## Acceptance mapping

- FR-002, FR-016, FR-028, FR-029.
