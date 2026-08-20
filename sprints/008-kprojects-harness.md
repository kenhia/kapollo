# 008 — kprojects harness

korg #1462 (chore, M) · the straggler from the kprojects rollout (#737)

## Goal

Migrate kapollo off Spec-Kit onto the kprojects minimal harness: collapse the
seven spec directories into `sprints/`, remove the Spec-Kit machinery, and give
the repo a `just check` that matches the CI it already had.

## Why kapollo was not in batches 1–5

It was never routed. #737 listed kapollo only on its **working-skill-repo trial
candidate list** (`korg`, `kris`, `krag`, `kyac`, `kvllm`, `kapollo`); when that
trial was carved out to #1223 on 2026-08-12, the candidate list was "retired
without judging any of them". kapollo got neither a *migrate* nor a *leave bare*
verdict and fell through the gap between them. Ken asked for it directly on
2026-08-19.

## What shipped

- **Harness applied** — `kproject-install --agent both`, stack **`rust`
  (detected)** from the root `Cargo.toml`; no `--stack` override needed.
  `sprints/{planning,review}`, `docs/`, `.scratch/`, `.env` in `.gitignore`,
  managed block in `CLAUDE.md` (new file) and `.github/copilot-instructions.md`.
- **Seven spec directories moved into `sprints/`**, numbering preserved — the
  numbers match the repo's GitHub branch names (`001-mvp-repl` …
  `007-laat-mode`), so they are history. All seven are multi-doc (spec, plan,
  tasks, research, data-model, quickstart, plus `checklists/` and `contracts/`;
  100–144K each), so all seven stayed directories rather than collapsing into
  single files — the same call kpidash's #1263 made for its larger specs.
- **`specs/planning/` → `sprints/planning/`** alongside the seeded `roadmap.md`:
  `brainstorm.md`, the four-part `grid-pivot/` series, and the pre-plans.
- **Spec-Kit machinery removed** — `.specify/` (templates, scripts, extensions,
  integrations, constitution), 13 `.github/agents/speckit.*`, 12
  `.github/prompts/speckit.*`, the 5-line SPECKIT stub that was all
  `.github/copilot-instructions.md` contained, and the empty
  `.github/instructions/`.
- **A gate that matches CI.** The seeded rust `justfile` lints with
  `--all-targets`; `.github/workflows/ci.yml` lints with `--all-targets
  --all-features`. Added `--all-features` so `just check` is not weaker than the
  thing it claims to mirror, and recorded the `fish` dependency the shell-parity
  tests carry.
- **Both agent files** carry the managed block plus an equivalent `## Project`
  section — what this is, build/run/test, the six things to read first, and the
  gotchas not derivable from the code.
- **`specs/…` cross-references repointed to `sprints/…`** — 19 files outside
  `sprints/` and 25 within it.
- **One pre-existing clippy error fixed** so the gate is green on arrival:
  `src/output/parser.rs:133`, `intermediates != [b'?']` → `!= *b"?"`. See the
  decision below. `just check` now exits 0: **304 tests across 38 suites**.

## Decisions

**The constitution was dropped whole, not folded in.** Ken's call, mid-sprint.
`.specify/memory/constitution.md` (v1.0.0, ratified 2026-05-29) held seven
principles, and the plan going in was to keep the durable half — the Code
Standards Gate, the docs-per-iteration rule, the Terminal-UX quality bar — and
drop the Spec-Kit half. Ken said remove it. What survives does so because it was
already true somewhere load-bearing: the gate is `ci.yml` and now `just check`,
and the UX and logging rules live in `docs/architecture.md` and are restated in
`## Project`. Recoverable from git if that turns out wrong.

**The harness took sprint number 008, which a pre-plan had reserved.**
`specs/planning/pre-plan-008-fancy-status-bar.md` planned 008 as the templated
status bar; it was never built. Sprint numbers here are the chronological record
of what shipped, and this shipped first, so the pre-plan was renamed to
`pre-plan-fancy-status-bar.md` — dropping a number claim that had become false
rather than leaving a misleading one in place. The 005/006/007 pre-plans keep
their numbers, which are still accurate.

**Source and test files were edited, which a chore's authorization normally
excludes.** 19 files outside `sprints/` referenced `specs/…`, including 12 `//!`
doc comments under `src/` and `tests/`. The edits are comment-only with zero
behaviour change, and the alternative was leaving a dozen source files pointing
at a directory that no longer exists.

**The duplicate-clone question resolved itself.** The korg project notes
(2026-08-14) recorded kai and kubs0 both holding kapollo at `448ff3f`, both
dirty, "neither obviously the primary" — which would have been a real blocker
for any migration. There is no kapollo anywhere under `~` on kubs0 now. kai is
the sole clone; the project notes were corrected.

**No `clippy.toml` was added**, consistent with the other Rust repos on kai and
with the harness rule to mirror a sibling rather than invent. kmuster's #1295
reached the same conclusion for the same reason.

**The gate was red before the harness touched it, and the fix shipped here.**
The first `just check` failed on `clippy::byte_char_slices` at
`src/output/parser.rs:133`. Three things established before deciding:

1. **Not caused by adding `--all-features`** — it fails identically without it.
   (Worth knowing: kapollo has no `[features]` section at all, so that flag is
   decorative here. It stays for symmetry with `ci.yml`, not for effect.)
2. **Not caused by the migration** — `cargo fmt --check` and every test were
   already green; only this lint was red.
3. **Caused by the floating toolchain.** `rust-toolchain.toml` pins `stable` and
   CI uses `dtolnay/rust-toolchain@stable`; both float. `byte_char_slices`
   arrived in a clippy newer than the repo's last commit (2026-06-07) — kai is
   on clippy 0.1.97 (2026-07-14). **kapollo's CI on `main` is red right now for
   this same lint**, and has been since the toolchain moved.

The krcmd slice's rule is "record and park, don't fix lint inside a chore", but
that guards against *fixing lint* — a judgement-heavy sweep. This is clippy's own
suggested rewrite of one expression into an identical type (`[u8; 1]` either
way), and the alternative was landing a harness whose headline promise —
`just check` is the gate — is broken on arrival, which the rollout has
consistently refused to do. Ken's call, 2026-08-19: ship it.

**Standing lesson, already in klams:** `@stable` floats, so a repo dormant for a
few months has a red gate waiting for whoever runs it next. That is a property of
the pin, not of this migration.

## Follow-ups

- **Both standing proposals were stale. Verified and merged, 2026-08-19** — see
  the postscript below. #184 declined, #176 now carries all ten open items, and
  **#43 `/save`** closed as the only one genuinely shipped.
- `.vscode/mcp.json` carries an uncommitted change dropping the `kwi` MCP server
  — Ken's in-flight work, predating this sprint. Left alone, not committed.

## Postscript — the proposal verification pass, 2026-08-19

This sprint's first pass flagged four work items as "looks shipped" on the
strength of matching names in the source. Ken asked for them verified and
closed. **Reading each work item's acceptance criteria refuted three of the
four.** The corrected result:

| WI | First read | Verified | Why the first read was wrong |
|---|---|---|---|
| **#43** `/save` | shipped | **shipped — closed** | correct. Answered all five of the WI's open design questions, including the evicted-block "unavailable" case (`"Save failed, previous buffer not found"`), and added an overwrite/append/cancel prompt it never asked for. `src/app.rs:1078`. |
| **#37** input-pad prompt char | shipped | **open** | `prompt_char` is in `src/config.rs`, but the WI says to *reuse* that existing transcript-echo config — so finding it confirms the **prerequisite**, not the feature. Referenced only from `src/ui/transcript.rs` and `src/app.rs`; `src/ui/input_pad.rs` has no prompt at all, and the required `input_prompt` toggle does not exist. |
| **#44** `/filter` | shipped | **open** | `/filter <cmd>` ships (sprint 007, FR-025) and pipes the previous block's output through a shell command. The WI asks for an interactive filter over the **transcript**, keyed on block boundaries and exit codes, with an overlay (FR-022, SC-008). Same command name, different feature. |
| **#46** whitespace suppression | shipped | **open** | The WI's own body splits it and says so: the *default* trailing-strip shipped in sprint 005 as T034 and is **explicitly out of this WI's scope**; the WI **is** the deferred config surface — `suppress_multiline_whitespace` and `suppress_multiline_trailing_whitespace_lines`. Neither key exists in `TOP_LEVEL_KEYS`. |

**The lesson, since it cost a wrong report: a matching identifier in the source
is not evidence a work item is done.** The three misses were, respectively, a
*prerequisite the item itself names*, a *different feature sharing a command
name*, and the *half explicitly carved out* of the item's scope. In two of the
three the work item's own text said as much; only the code was read.

Also confirmed genuinely open, with evidence:

- **#42** Ctrl-L semi-clear — no `semi_clear`/`SemiClear` in `src/`; the only
  clear-ish action is `Action::ClearStatusMessage`.
- **#47** prompt into divider — `src/ui/divider.rs:3` says it outright: *"A
  future feature (kwi #47) may fold the shell prompt into this rule; for now it
  is a single horizontal line."* Its stated prerequisite **is** satisfied
  (sprint 005 T035 restored the divider), but it still needs OSC 133 `A`/`B`
  from the injected shell hooks — `src/pty/shell.rs` emits only `C`/`D` + OSC 7.
- **#45** click-vs-drag threshold — research, unchanged since filing.
