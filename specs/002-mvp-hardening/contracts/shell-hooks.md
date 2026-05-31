# Contract Delta: Shell hooks — OSC 7 cwd emission

Extends [specs/001-mvp-repl/contracts/shell-hooks.md](../../001-mvp-repl/contracts/shell-hooks.md).
Adds working-directory reporting via **OSC 7** to the existing OSC 133 hooks
(FR-019, D23/D19/D12).

## Sequence

The shell emits, on every prompt, an OSC 7 sequence reporting the current working
directory as a `file://` URI:

```text
ESC ] 7 ; file://<host>/<absolute-path> ST
```

- `ST` is the String Terminator (`ESC \`), matching the OSC 133 emission style.
- `<host>` MAY be empty or the local hostname; kapollo ignores it and uses only
  the path component.
- `<absolute-path>` is percent-encoded per the `file://` URI convention; kapollo
  percent-decodes it.

## Per-shell delivery

- **fish** (added to the existing `--init-command`): emit OSC 7 from a
  `--on-event fish_prompt` function using `$PWD`, e.g. printing
  `\e]7;file://$hostname$PWD\e\\`. Existing `fish_preexec`/`fish_postexec`
  OSC 133 functions are unchanged.
- **bash** (added to the generated rcfile): extend `PROMPT_COMMAND` (which already
  runs before each prompt) to also emit OSC 7 using `$PWD`. The existing
  `__kapollo_cmd_end` (OSC 133 `D`) and `PS0` (OSC 133 `C`) are unchanged. The
  idempotence guard around `PROMPT_COMMAND` continues to apply.
- **Other shells (sentinel mode)**: no OSC 7 is emitted; the status cwd remains at
  its last known value (initialized from `current_dir()` at startup). This is the
  accepted degraded behavior (D23).

## kapollo parsing & application

- The vte `osc_dispatch` recognizes OSC param `7`, extracts the path from the
  `file://` URI, percent-decodes it, and produces a `Boundary::Cwd(PathBuf)`.
- The event loop updates `App.cwd`, which the status rule renders (FR-007).
- Initialization: `App.cwd` starts from `std::env::current_dir()` so it is correct
  before the first prompt fires.

## Notes

- OSC 7 is emitted on every prompt, so it tracks `cd`, `pushd`/`popd`, `z`,
  subshell returns, and scripted directory changes — anything that changes `$PWD`
  by the time the next prompt renders (FR-019).
- Emission is best-effort: a shell or terminal that strips OSC 7 simply yields no
  cwd update; kapollo never errors on its absence.
