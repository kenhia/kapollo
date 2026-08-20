# 009 — `/filter` → `/pipe`

korg #1463 (tweak, S) · from proposal #176

## Goal

Hard-rename the sprint-007 slash command `/filter <cmd>` to `/pipe <cmd>`. No
behaviour change, no alias.

## Why

The command takes the previous block's output and pipes it through a shell
command — `ls -l`, then `/pipe grep foo`. It does not filter the transcript.
The old name said it did, and that cost something real: during sprint 008's
verification pass the name `/filter` was compared against work item #44
(*"filter transcript over block boundaries + exit codes"*), the two were judged
different features, and **a finished feature was reported as unbuilt**. Ken
settled it — the pipe behaviour was always the intent, #44 closed, and the name
is what needed fixing.

## What shipped

**Code**

- `src/slash/mod.rs` — `SlashCommand::Filter(String)` → `Pipe(String)`; the
  dispatch arm is now `"pipe"`; doc comments updated.
- `src/app.rs` — `run_filter` → `run_pipe`, `filter_active` → `pipe_active`,
  `filter_temp_path` → `pipe_temp_path`. The temp file is now
  `kapollo-pipe-{pid}-{n}.txt`, the transcript block label is
  `{leader}pipe <cmd>`, and both user-facing strings changed:
  `"'/pipe' requires a command"` and `"pipe non-zero exit"`.
- `src/slash/builtins.rs` — `/help` body. Worth noting the help text already
  read *"Pipe the previous output through &lt;cmd&gt;"* under the old name, which is
  its own evidence the name was wrong.
- `src/session/{block,store}.rs` — doc comments naming the command.
- `tests/slash_filter_save.rs` → `tests/slash_pipe_save.rs`.

**Docs** — `README.md` (both slash-command lists), `docs/usage.md`,
`docs/architecture.md` (§15.4 and five references), `docs/specification.md`
(FR-S24, the block-store section, the sprint-007 heading).

**A new test pins the decision.** `filter_is_no_longer_a_command` asserts both
`filter` and `filter rg foo` dispatch to `Dispatch::Unknown("filter")`, so the
user gets the `/help` suggestion rather than a silent miss. Gate: `just check`
exits 0, **305 tests across 38 suites** (one more than sprint 008).

## Decisions

**Hard rename, no alias.** Ken's call. There is precedent for aliasing —
`"quit" | "exit"` in the dispatcher — and keeping `"pipe" | "filter"` would have
cost one line. It was rejected because the entire point of the change is that
`/filter` misleads: an alias keeps the misleading name alive in `/keys`, in
`/help`, and in muscle memory, in exchange for a transition period whose only
user is the person who asked for the rename.

**The `/filter` name is documented as history, not erased.** `docs/usage.md`,
§15.4 of the architecture, the dispatcher doc comment and the test file header
each say the command was called `/filter` before this sprint and that no alias
was kept. Someone hitting the old name in an old sprint record or in muscle
memory should find out why it stopped working, not just that it did.

**Nothing was blanket-sed.** `filter` is a common Rust iterator method — nine
unrelated `.filter(` call sites across `src/` and `tests/`, plus `EnvFilter` in
`src/logging.rs` and `env-filter` in `Cargo.toml`. Each occurrence was judged
individually; `cargo check --all-targets` ran before the docs pass to catch a
miss early.

## Fixed in passing

`docs/specification.md` contradicted itself: **FR-S24 specified `/filter` as
shipped while §5 Scope still listed it as "deferred; tracked separately"**,
alongside `/save`, which also shipped in sprint 007. That stale line is a fair
suspect for why work item #44 stayed open and confusing for a year. §5 now
records sprint 007's actual scope and no longer claims either command is
missing.

## Follow-ups

- **Sprint 008 left 18 dangling `Constitution` references** — `docs/architecture.md`
  (×5, including "authoritative technical reference per Constitution"),
  `docs/specification.md`, `src/app.rs`, `src/logging.rs`, six test files and two
  `delos/docs/` spike notes all cite a document that sprint 008 deleted. Noticed
  here, out of scope here; filed as **korg #1464**. The fix is to inline the
  principle each site actually depends on — mostly "live-TTY behaviour is an
  integration-test exception" (III) and "logs never touch the screen" (VI).
