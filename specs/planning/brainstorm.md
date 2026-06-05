# kapollo — Brainstorm & Planning

> Living notes. This is a conversation, not a spec. We iterate here, then
> distill decisions into `specs/` and `docs/architecture.md`.

Last updated: 2026-05-30 (post-MVP user test → hardening sprint 002; D22–D24 added)

## 1. Vision

A terminal "REPL" that divides the screen the way modern agentic CLIs
(GitHub Copilot CLI, Claude Code, etc.) do — but instead of chatting with
an AI, you type **shell commands** at the bottom and see **command output**
flow into the upper region. Beyond "just a shell wrapper," kapollo adds a
feature layer: slash commands, saved command slots/macros, rich rendering,
and other quality-of-life affordances.

### Inspiration
- **Apollo/DomainOS DM (Display Manager)** under X in the early-mid 1990s:
  commands always typed in a bottom input pad; output scrolled in the upper
  transcript pad. Multiline editing was natural; the input pad grew then
  scrolled. Commands could be saved as files or into slots ("macros").
- **GitHub Copilot CLI / agentic CLIs**: the input-at-bottom feel, plus
  `/slash` commands as an extensibility surface (e.g. `/yolo`).

### What kapollo is NOT (at least not for MVP)
- Not an AI chat tool (though the feature layer could host AI later).
- Not a full terminal multiplexer/replacement (tmux/zellij/wezterm).
- Not a new shell language — it wraps an existing shell.

## 2. The Core Metaphor: Input Pad / Output Pad

```
┌──────────────────────────────────────────────┐
│  OUTPUT / TRANSCRIPT PAD                       │
│  (scrollback of commands + their output)       │
│                                                │
│                                                │
├──────────────────────────────────────────────┤
│  STATUS / CHROME (cwd, exit code, mode, hints) │
├──────────────────────────────────────────────┤
│  INPUT PAD  (grows for multiline, then scrolls)│
│  $ _                                           │
└──────────────────────────────────────────────┘
```

Key behaviors to preserve from the Apollo feel:
- Multiline command editing is first-class and easy (not an afterthought).
- Input pad auto-grows up to a cap, then scrolls internally.
- Clear visual separation between "what I typed" and "what came back."
- The transcript is navigable/scrollable independent of the input pad.

## 3. Big-Picture Feature Map (to inform architecture, not MVP scope)

Tiered by how far from "shell wrapper" they push us.

### Tier 0 — MVP (shell wrapper REPL)
- Run as a CLI binary in any terminal; full-screen TUI.
- Bottom input pad with multiline editing + history.
- Upper transcript pad with scrollback.
- Spawn/wrap the user's shell (or run commands directly) via a PTY.
- Faithful exit codes, signal forwarding (Ctrl-C), cwd tracking.
- Status line: cwd, last exit code, current mode.
- Honor `NO_COLOR`, resize cleanly, clean teardown on exit.

### Tier 1 — Slash commands & ergonomics
- `/help`, `/quit`, `/clear`, `/cd`, `/history`.
- Saved command **slots/macros** (Apollo-style): save current input to a
  named slot, recall, edit, run. Persist to disk.
- Command palette / fuzzy recall of history and slots.
- Config file (key bindings, prompt, colors, shell to wrap).

### Tier 2 — Rich rendering & "special" slash commands
- `/view file.md` → render markdown in the transcript (color + layout).
- **Preserve & render ANSI SGR styling** (color, bold, etc.) emitted by the
  wrapped command *in transcript blocks* — i.e. translate the program's own
  color into styled cells, not just strip it (see D4/§6, which strip styling
  for the MVP). Distinct from kapollo generating its own color (chrome/render).
- Syntax-highlighted file preview, paged output.
- Structured output capture (per-command blocks you can fold/copy/re-run).
- Output filters/transforms (e.g. pipe last output through a slash command).
- **Output retention & redirection** (Ken): kapollo retains captured output
  per block (configurable size limits / ring buffer). Then:
  - `/save` — write the last command's output (or a chosen block) to a file.
  - `/filter rg <pat>` (or `/grep`, `/less`) — pipe a block's retained output
    through an external filter and show the filtered result. Useful when a
    command produced too much output to read inline.
  - Implies blocks must hold their captured bytes, not just rendered lines —
    a constraint to bake into the session/transcript model early.

### Tier 3 — Power features / aspirational
- Per-command result objects (re-run, copy, share, annotate).
- Pluggable slash commands (user scripts / a small plugin API).
- Sessions/transcripts saved + reloadable.
- Remote/SSH-aware sessions; split panes; tabs.

### AI Feature Layer (Ken: strong appetite — a natural fit, not MVP)
This is now an explicit planned direction, not a "maybe." It rides on the
same slash-command + block/render + output-retention infrastructure:
- **Local-model CLI chatbot**: turn a local model into a chat with a nice
  interface — query in the input pad, response rendered in the output pad.
- **JIT agent help**: a command just failed → one slash command asks an
  agent for help using the failed block's command + output as context.
- **Ad-hoc agent query**: send a prompt to an agent, get the answer as a
  block in the transcript.
- **Output → agent**: pipe a block's retained output to an agent to
  summarize, explain, or generate a report (reuses the §Tier-2 filter pipe,
  with an agent as the "filter").
- Architectural note: the AI layer is just another *consumer/producer* of
  blocks. If blocks carry (command, output, exit code) and slash commands
  can read a block + write a new block, the AI layer needs no special
  plumbing. Keep it opt-in and provider-agnostic (local first).

## 4. Interaction Model — Open Questions

These shape the architecture, so worth deciding early (even provisionally):

1. **Command execution model**: **DECIDED (a)** — wrap a real shell via PTY,
   feed it commands, preserve shell state (cwd, env, aliases, `&&`, pipes).
   - kapollo MUST support **multiple/different shells** (Ken uses fish, drops
     into bash, etc.). Shell is configurable; kapollo does not assume one.
2. **Transcript granularity**: **DECIDED — hybrid.** Append-mostly structured
   blocks for normal commands; **full-screen passthrough** to the real
   terminal when a program grabs the alt-screen (`vim`, `top`, `less`).
   - We do NOT build our own terminal grid model (see §6). The host terminal
     does the grid work during passthrough.
3. **Input pad capabilities**: **DECIDED** — MVP = basic line + multiline +
   history. Eventually very rich (readline-ish, completion, etc.) — deferred.
4. **Slash command interception**: **DECIDED** — intercept early. A leading
   leader char (`/`) enters **slash-mode**; a second leader char immediately
   exits slash-mode and inserts the literal character. Everything not in
   slash-mode passes through to the shell. Leader char is configurable.
   - MVP: the simple commands in §7. Later: rich slash-mode with fuzzy
     matching and per-command descriptions/help inline.

## 5. Architectural Implications (early)

The Tier 2/3 features (markdown render, blocks, re-run, plugins) push us
toward a clean separation:

- **PTY / process layer** — owns the wrapped shell, byte I/O, signals,
  resize. (`portable-pty` or similar.)
- **Terminal parser / VTE layer** — interprets shell output into a screen
  model. (`vte` crate for ANSI parsing; possibly a grid model.)
- **Session/transcript model** — the source of truth: command blocks,
  outputs, exit codes, slots, history. UI renders from this.
- **Input router** — decides: slash command vs. pass-through to PTY.
- **Slash command registry** — built-in + (later) pluggable commands.
- **Renderer / TUI layer** — draws input pad, transcript pad, chrome.
  (`ratatui` + `crossterm` is the obvious Rust stack.)
- **Config & persistence** — config file, slots, saved sessions.
- **Logging** — `tracing` to a file sink (never corrupt the TUI).

The big architectural fork is **Question 4.2** (raw emulation vs.
structured blocks vs. hybrid). Deciding the transcript model first
de-risks everything downstream.

## 6. Candidate Rust Stack

- **TUI**: `ratatui` + `crossterm` (cross-platform, mature).
- **PTY**: `portable-pty` (from wezterm) — cross-platform PTY spawning.
- **ANSI parsing**: `vte` (Alacritty's parser) if we emulate; or a
  higher-level terminal model crate.
- **Markdown render**: `pulldown-cmark` + custom ratatui styling, or an
  existing terminal-markdown crate.
- **Config**: `serde` + `toml`.
- **Logging**: `tracing` + `tracing-subscriber` (file appender).
- **Fuzzy match** (palette/history): `nucleo` or `fuzzy-matcher`.

**RESOLVED**: We do NOT build a full terminal grid model. A *grid model* is
what a real emulator keeps internally — a rows×columns array of styled cells
with a cursor that can jump and overwrite any cell (needed for `top`,
progress bars, in-place redraws). Instead:
- **Normal commands** → append-mostly: capture/stream output lines, append
  to the block. Simple, and enough for the vast majority of commands.
- **Alt-screen / interactive programs** → passthrough: hand the raw PTY to
  the host terminal and let *its* emulator do the grid work. kapollo never
  maintains a cell grid itself.
- Consequence: `vte`/ANSI parsing is needed mainly to (a) detect alt-screen
  enter/leave and (b) strip or translate styling for the block model — not
  to emulate a full screen.

## 7. MVP Definition of Done (draft)

- Launches in any terminal; clean enter/exit (restores terminal state).
- Bottom input pad: type, edit, submit. **Enter submits; Shift+Enter inserts
  a literal newline** (no submit). Explicit `\`-style shell continuation is
  deferred (would require per-shell command parsing).
- Command runs via wrapped shell (configurable; default = user's `$SHELL`);
  output appears in upper pad.
- Scrollable transcript; resize-safe; Ctrl-C interrupts running command.
- Accurate cwd + exit-code tracking in status line.
- At least a couple slash commands: `/quit`, `/clear`, `/help`.
- `NO_COLOR` honored; logs go to a file, not the screen.

## 8. Naming / Terminology (STANDARDIZED)
- **pad** — a screen region. **input pad** (bottom) and **output pad** /
  **transcript pad** (top). (Apollo nod.)
- **slot** — a saved, reusable command/input (Apollo "macro").
- **block** — one command + its captured output + exit code in the transcript.
- **slash command** — a feature command, invoked via the **leader char**
  (default `/`, configurable). Entering the leader char enters **slash-mode**.
- **passthrough** — alt-screen mode where the raw PTY is handed to the host
  terminal (for `vim`, `top`, `less`, etc.).

## 9. Decisions Log
> Record decisions as we lock them in.

- **D1 — Execution model**: Wrap a real shell via PTY (option a). kapollo
  feeds commands to a long-lived shell; shell state is preserved.
- **D2 — Multi-shell**: Shell is configurable; kapollo supports fish, bash,
  etc. No single-shell assumptions baked in.
- **D3 — Transcript model**: Hybrid. Append-mostly structured **blocks** for
  normal commands; **passthrough** to host terminal for alt-screen programs.
- **D4 — No grid model**: kapollo does not implement a terminal cell grid.
  ANSI parsing is used only for alt-screen detection and style handling.
- **D5 — Submit convention**: Enter submits; Shift+Enter inserts a newline.
  `\`-continuation deferred (needs per-shell parsing).
- **D6 — Slash-mode**: Leader char (`/`, configurable) enters slash-mode;
  doubled leader char escapes to a literal. Rich fuzzy slash-mode is post-MVP.
- **D7 — Terminology**: pad / input pad / output (transcript) pad / slot /
  block / slash command / leader char / passthrough (see §8).
- **D8 — Block data**: Blocks retain captured output bytes (size-limited),
  not just rendered lines — enables `/save`, `/filter`, and the AI layer.
- **D9 — Platforms**: Linux-only for MVP. macOS & Windows are in scope later
  but must not cost Linux functionality/parity now. Revisit at "tell our
  friends" stage.
- **D10 — Slots**: Not MVP. Target the sprint or two after MVP, alongside
  "save input pad contents to file."
- **D11 — AI layer**: Explicitly planned (post-MVP). Built on the same
  block/slash/output-retention infra; opt-in; local-model-first.
- **D12 — Block boundary detection**: OSC 133 prompt marks (a) where the
  shell supports it, sentinel-echo injection (b) as fallback. kapollo MAY
  install a small shell hook to emit the marks.
- **D13 — Rich history store (post-MVP)**: a file-based store (likely a
  small embedded DB, e.g. SQLite) persisting `{timestamp, command, output,
  exit_code}` per block. Requirements baked into the model NOW so it isn't
  hard later:
  - stdout+stderr captured as a best-effort interleaved stream per block.
  - User controls: disable entirely, purge all, and **purge-output-only**
    (keep the command, drop the output) — "I want the command I ran two
    weeks ago, not its 10k lines."
  - **Privacy leaders**: leading-space "don't save this command at all"
    (history-style); a second notation for "save command but not output."
  - MVP does not build the DB, but the block model and capture pipeline
    MUST be shaped so this drops in without redesign.
- **D14 — Output caps**: configurable, enforced at BOTH the per-block and
  whole-transcript level, ring-buffered (keep the tail). Defaults chosen by
  implementer (see Architecture); RAM is not the only concern (render cost,
  scrollback perf). Proposed defaults: per-block 1 MiB / 50k lines; whole
  transcript 128 MiB or 1000 blocks (whichever first); hard max per-block
  64 MiB. Overflow is truncated head-first with a visible "… truncated …".
- **D15 — Config**: `~/.config/kapollo/config.toml` (XDG). Single base file;
  richer subsystems (history DB, AI connections) get their own sections /
  sibling files so the base config stays lean. Project-level override TBD.
- **D16 — Newline keys**: BOTH `Shift+Enter` and `Alt+Enter` insert a
  newline by default (Enter submits). `Alt+Enter` is the reliable fallback
  where the terminal can't distinguish `Shift+Enter`. Remapping is post-MVP.
- **D17 — MVP shells**: validate against **fish + bash**. Default to
  `$SHELL`; other shells best-effort.
- **D18 — Naming**: crate/full name `kapollo`; ship a short alias **`kap`**
  (3 letters to launch).
- **D19 — Shell hook delivery**: auto-inject the OSC 133 hook into the
  spawned shell (belt-and-suspenders). May expose a manual
  `kap init <shell> | source` path and make injection configurable later.
- **D20 — Command history**: kapollo maintains its **own** input history
  (up/down-arrow recall), kept **separate** from the wrapped shell's native
  history; the shell still records its own history as usual. Richer history
  manipulation is post-MVP (several sprints out); a later sprint may add
  config to influence shell history via the D19 hook mechanism.
- **D21 — Active-session env var**: `kap` sets `KAPOLLO_ACTIVE=1` in the
  wrapped shell's environment so scripts/prompts can detect they run inside
  kapollo. (Likely also export a version var, e.g. `KAPOLLO_VERSION`.)
- **D22 — Color scope (phased)**: kapollo's **own chrome** (the `λ` prompt
  char, status line, `^C` hint) is colorized in the hardening sprint (002).
  **Block ANSI passthrough** — preserving/rendering the wrapped program's own
  SGR colors in transcript blocks — is explicitly deferred to **Tier 2** (new
  bullet in §3). MVP/hardening still *strips* program styling per D4/§6.
- **D23 — cwd tracking via shell hook (OSC 7)**: the cwd shown in the status
  line comes from the shell emitting **OSC 7** (`ESC]7;file://host/abs/path ST`)
  from the same prompt hook that emits OSC 133 (D12/D19), parsed in the
  existing vte layer. We do NOT scrape cwd from rendered prompt text
  (theme/shell-fragile, violates D2). Sentinel-fallback shells get no live
  cwd for now (consistent with their degraded support); optional parity later.
- **D24 — Transcript scrolling (keyboard-first)**: scrollback is
  **keyboard-only** for now — PgUp/PgDn (and Home/End to jump), surfaced in
  `/help`. **Mouse-wheel capture is deferred and will be opt-in** (config
  `mouse = true`, default off) because capturing the mouse breaks the host
  terminal's native click-drag text selection/copy — an unacceptable default
  for a shell REPL. When added, capture must be disabled during alt-screen
  passthrough so `vim`/`top` receive mouse events, and re-enabled on return.

### Round 3 — Grid pivot (promoted from grid-pivot planning, sprint 004)

> Source: [grid-pivot/02-rework-vs-rewrite.md](grid-pivot/02-rework-vs-rewrite.md) §6
> and the 003 spike recommendation. Realized by the `004-grid-rework` feature.

- **D25 — Grid model (reverses D4)**: kapollo **does** maintain a terminal grid
  model for the main screen, with scrollback. Supersedes D4's "no grid model".
- **D26 — Grid scope**: emulate the main screen + scrollback (render/selection
  surface); block boundaries stay a shell-integration concern (OSC 133/7).
- **D27 — Engine**: `wezterm-term`, git-pinned `rev =
  577474d89ee61aef4a48145cdec82a638d874751`, with `alacritty_terminal` as the
  named fallback (003 spike winner; `StableRowIndex` the deciding factor).
- **D28 — Mouse (revises D24)**: click-drag selection + wheel/PageUp-Down
  scroll + alt-screen/inner-mouse-mode routing; Shift bypasses to the host
  terminal; clipboard via OSC 52 with a local fallback. Revises D24's
  "mouse deferred/opt-in".
- **D29 — Block-as-annotation-over-grid (refines D8/D13/D14)**: a block is a
  row-range annotation over the grid's scrollback, behind a single text
  accessor (`block.text()` / `block.text_with_command()`).
  **SUPERSEDED in part by R3 (sprint 004):** the v1 lean of *reconstructing*
  block text from grid rows is replaced by an **in-memory block store that
  retains each block's output text as canonical** (byte/text-faithful `/save`,
  foundation for deep-history + privacy-toggleable persistence). The accessor
  surface is unchanged, so a future database backing is a drop-in (no caller
  changes). See [004-grid-rework/research.md](../004-grid-rework/research.md) R3.
- **D30 — Inline SGR color (revises D22)**: the wrapped program's inline color
  and text attributes are now rendered in the transcript. Revises D22's
  Tier-2 deferral.

### Scope boundaries (derived)
- **MVP (Linux only)**: PTY-wrapped configurable shell; input/output pads;
  Enter/Shift+Enter; history; scrollable transcript; resize-safe; Ctrl-C;
  cwd + exit-code status; alt-screen passthrough; `/quit`, `/clear`, `/help`;
  `NO_COLOR`; file logging. Blocks retain output (foundation for later).
- **Post-MVP sprint(s)**: `/save` input/output to file; slots; richer editor.
- **Later**: rich slash-mode (fuzzy + descriptions), `/view` markdown render,
  output filters, AI feature layer, then macOS/Windows parity.

## 10. Open Questions

### Round 1 — RESOLVED (see Decisions Log §9)
1. Execution model → D1 (PTY-wrapped real shell) + D2 (multi-shell).
2. Transcript model → D3 (hybrid) + D4 (no grid model).
3. Platforms → D9 (Linux MVP; macOS/Windows later).
4. Submit convention → D5 (Enter submits, Shift+Enter newline).
5. Slots → D10 (post-MVP, with file-save).
6. AI layer → D11 (planned, post-MVP, local-first).

### Round 2 — RESOLVED (see Decisions Log §9)
1. Block boundary detection → D12 (OSC 133, sentinel fallback) + D13
   (rich history store requirements: stdout/stderr interleave, purge,
   purge-output-only, privacy leaders).
2. Output retention limits → D14 (per-block + transcript caps, ring buffer,
   defaults proposed).
3. Config location/format → D15 (`~/.config/kapollo/config.toml`).
4. Shift+Enter reliability → D16 (Shift+Enter AND Alt+Enter newline).
5. MVP shell scope → D17 (fish + bash; default `$SHELL`).
6. Binary/crate name → D18 (`kapollo` + `kap` alias).

## 11. Suggested Next Steps
1. Resolve Round-2 Q1 (block boundary) — it's the next-biggest lever after
   the transcript model; it gates blocks, `/save`, `/filter`, and AI.
2. Draft `docs/architecture.md` from §5 + the decisions (layers, data flow,
   the block lifecycle, passthrough handoff).
3. Write the MVP spec under `/specs/001-mvp-repl/` with acceptance criteria
   derived from §7 + the MVP scope boundary in §9.
4. Scaffold the Rust crate (binary) with the §6 stack and a walking skeleton
   (open PTY, echo a command, render to two pads, quit cleanly).
