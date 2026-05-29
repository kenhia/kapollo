# Quickstart: kapollo MVP

**Feature**: 001-mvp-repl | **Date**: 2026-05-29

This is the developer/early-adopter quickstart for validating the MVP. It
doubles as the manual acceptance walkthrough mapping to the spec's success
criteria.

## Build & run

```bash
# Build (Linux)
cargo build --release

# Run the split-pad REPL (wraps $SHELL)
./target/release/kap

# Or with an explicit shell
./target/release/kap --shell /usr/bin/bash
```

## First-run expectations

- A full-screen UI appears: empty transcript pad on top, focused input pad at
  the bottom, a status line showing the current working directory (SC-001).
- No config file is required; defaults apply (FR-028).

## Acceptance walkthrough

### US1 — Core run loop (P1)

```bash
echo hello          # → block shows "hello"; input pad clears
pwd                 # → shows current directory
cd /tmp && pwd      # → shows /tmp; state persists across commands
false               # → status line shows non-zero exit code
alias               # → your shell aliases are present (fish/bash)
```

Expected: each command and its output appears as a discrete block; shell
state persists (SC-002, SC-009).

### US2 — Multiline & history (P2)

- Type `for i in 1 2 3`, press **Shift+Enter** (or **Alt+Enter**), type
  `echo $i`, Shift+Enter, `end` (fish) / `do echo $i; done` (bash), then
  **Enter** → runs as one command (FR-009–FR-012, SC-007).
- Press **Up** → previous input is recalled into the pad; **Down** moves
  toward newer (FR-013).

### US3 — Interactive programs (P2)

```bash
vim                 # opens full-screen; edit; :q to exit
less /etc/hostname  # pager works; q to exit
top                 # live UI; q to exit
```

Expected: each runs natively via passthrough; on exit the split-pad UI is
restored with the prior transcript intact (FR-018–FR-020, SC-003). Resize the
window while `top` runs → it reflows correctly (FR-019, SC-008).

### US4 — Interrupt, control, exit (P1)

```bash
sleep 60            # then press Ctrl-C → command interrupts, kapollo stays up
/help               # lists slash commands
/clear              # clears the visible transcript
/quit               # exits; terminal restored cleanly
```

Expected: Ctrl-C interrupts only the command (SC-004); on exit the terminal
is clean — cursor visible, no leftover alt-screen (SC-005). Typing `//foo`
sends a literal `/foo` to the shell (FR-022).

### Caps / large output (edge)

```bash
yes | head -n 5000000   # huge output
```

Expected: memory stays bounded; the block shows a `… output truncated …`
marker; UI remains responsive (FR-016, SC-006).

### Logging & non-TTY (edge)

```bash
kap | cat               # stdout not a TTY → diagnostic to stderr, no TUI
NO_COLOR=1 kap          # kapollo chrome renders without color
```

Logs are written to `~/.local/state/kapollo/kapollo.log`, never to the
screen (FR-030, FR-031, FR-032).

## Success criteria checklist

- [ ] SC-001 launch + first command within seconds, no setup
- [ ] SC-002 shell state persists across ≥100 commands
- [ ] SC-003 vim/less/top work and UI restores
- [ ] SC-004 Ctrl-C interrupts command, kapollo survives
- [ ] SC-005 terminal restored on exit/error/panic
- [ ] SC-006 large output bounded + truncation marker
- [ ] SC-007 multiline compose + arrow-key recall
- [ ] SC-008 resize preserves transcript; shell sees new size
- [ ] SC-009 identical core loop on fish and bash
