# Feature Specification: Configurable Keymap Engine

**Feature Branch**: `006-keymap-engine`  
**Created**: 2026-06-06  
**Status**: Draft  
**Input**: User description: "Make every key binding configurable, with a sane default map. Each behavior from sprint 005 (and existing 004 keys) is a named action with a primary and an alternate binding. Parse human-writable key strings (Ctrl+Left, Shift+PageUp, Alt+Enter), validate them, detect conflicts (warn + last-wins), and let `/keys` reflect the live config. Bind the previously-unbound copy variants, allow disable-by-clearing, support per-mode keymaps and the Esc Esc chord, and reload on demand via a slash command. Keep the out-of-the-box behavior identical to the hardcoded defaults."

## Overview

Sprint 005 shipped kapollo's key bindings **hardcoded**, but it did so behind a
named-action registry built for exactly this moment. This feature turns that
registry into a **configurable keymap engine**: every behavior is a named action
the user can rebind from config, with a sane default map that keeps kapollo's
out-of-the-box behavior **identical** to today.

The engine introduces a human-writable key-string grammar (`Ctrl+Left`,
`Shift+PageUp`, `Alt+Enter`, the `Esc Esc` chord), a `[keymap]` config surface
that supports a **primary and an alternate** binding per action, per-mode keymap
sections (anticipating LAAT in sprint 008), conflict detection with a
**warn + last-wins** policy, and **disable-by-clearing** so opinionated defaults
like `Ctrl+U`/`Ctrl+K`/`Ctrl+W` can be turned off. The previously-unbound copy
variants from sprint 004 (`copy_block_without_command`, `copy_current_line`) gain
default bindings. `/keys` reflects the **live** effective map, and a new
`/reload-config` command re-reads config on demand (no file watching).

This realizes the resolved decisions recorded in
[pre-plan-006-keymap-engine.md](../planning/pre-plan-006-keymap-engine.md): the
keymap may vary by mode; alternates are expressed as an array
(`["Shift+Enter", "Alt+Enter"]`); conflicts warn and last-wins; key strings use
canonical short modifier names (`Ctrl` over `Control`) and are case-insensitive;
the only chord is `Esc Esc`; and reload is on-demand via a slash command, not a
file watcher.

This is deliberately **plumbing for existing behaviors**: it adds no new
end-user editing actions and does not configure mouse bindings. The stable
foundation (PTY, slash registry, input router, grid, status bar, action
registry) carries over; this feature replaces the hardcoded binding table with a
config-driven keymap, adds the key-string parser and conflict validation, and
wires `/keys` and `/reload-config` to the live map.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Rebind any action from config with identical defaults (Priority: P1)

A user opens their config file, finds every kapollo behavior listed as a named
action, and rebinds the ones they care about using readable key strings — and a
user who changes nothing gets exactly the same keys as before.

**Why this priority**: This is the headline win and the reason the sprint exists.
Without it, the named-action registry from 005 is inert. Identical defaults are
the contract that lets existing users upgrade with zero surprises; rebinding is
the new capability. It is independently demonstrable with nothing but a config
file and `/keys`.

**Independent Test**: With no `[keymap]` config, confirm every action resolves to
its sprint-005/004 default key. Then rebind a handful of actions (e.g. move
`word_move_left` to a different key) and confirm the new key triggers the action
and the old key no longer does.

**Acceptance Scenarios**:

1. **Given** no keymap configuration, **When** kapollo starts, **Then** every
   action is bound to its established default key and all sprint-004/005 behavior
   is unchanged.
2. **Given** a config that rebinds an action to a new key string, **When**
   kapollo loads it, **Then** the new key triggers the action and the action's
   former default key no longer triggers it (unless that key is also bound).
3. **Given** a config that rebinds several actions, **When** kapollo loads it,
   **Then** each rebound action responds to its configured key and all unlisted
   actions retain their defaults.
4. **Given** the previously-unbound copy variants `copy_block_without_command`
   and `copy_current_line`, **When** kapollo starts with defaults, **Then** each
   has a working default key binding.

---

### User Story 2 - Primary and alternate bindings per action (Priority: P1)

A user gives an action two keys — a primary and an alternate — so that a single
behavior is reachable two ways, which is also how legacy-terminal fallbacks are
expressed (e.g. `Shift+Enter` primary, `Alt+Enter` alternate for "insert
newline").

**Why this priority**: The default+alternate slot is the mechanism that turns
legacy-terminal fallbacks from a code change into a config concern, and it is the
pre-planning's central design decision. It is small but foundational, and it is
independently testable from rebinding and conflict handling.

**Independent Test**: Bind an action to an array of two key strings. Confirm both
keys trigger the action. Confirm that providing a single key string still works
(alternate optional). Confirm the default map ships an alternate where one is
expected (newline insertion).

**Acceptance Scenarios**:

1. **Given** an action bound to an array `["Shift+Enter", "Alt+Enter"]`, **When**
   the user presses either key, **Then** the action fires.
2. **Given** an action bound to a single key string (not an array), **When** the
   user presses that key, **Then** the action fires and the action simply has no
   alternate.
3. **Given** the default keymap, **When** kapollo starts, **Then** actions that
   ship an alternate (such as newline insertion) respond to both their primary
   and alternate keys out of the box.

---

### User Story 3 - Readable key strings, validated, with conflicts reported (Priority: P2)

A user writes bindings in a natural, forgiving grammar — `Ctrl+Left`,
`shift+pageup`, `Alt+Enter`, `Esc Esc` — and kapollo accepts the ones it
understands, warns about the ones it does not, and warns when two actions claim
the same key (binding the last one declared).

**Why this priority**: A configurable keymap is only usable if the grammar is
forgiving and mistakes are surfaced rather than silently breaking input. It
depends on the binding model from User Stories 1–2 but adds the parser and
validation that make the surface safe to edit.

**Independent Test**: Provide config with mixed-case and short/long modifier
names and confirm they parse to the same chord; provide an unparseable key
string and confirm a warning is emitted and the binding is skipped (kapollo still
starts); provide two actions bound to the same key and confirm a conflict warning
names both actions and the last-declared one wins.

**Acceptance Scenarios**:

1. **Given** key strings that differ only in case or modifier spelling (e.g.
   `Ctrl+Left` vs `ctrl+left`), **When** they are parsed, **Then** they resolve to
   the same key chord.
2. **Given** an unparseable or unknown key string, **When** kapollo loads the
   config, **Then** it emits a warning identifying the bad binding, skips it, and
   continues starting normally.
3. **Given** two actions bound to the same key, **When** kapollo loads the
   config, **Then** it emits a conflict warning naming both actions and the
   last-declared binding takes effect (warn + last-wins).
4. **Given** the `Esc Esc` chord in config, **When** it is parsed, **Then** it is
   recognized as a two-key chord and surfaced in `/keys`; its dispatch remains the
   contextual `Esc Esc` handler this sprint (FR-008, FR-018).
5. **Given** an action whose binding is cleared (set to an empty value), **When**
   kapollo loads the config, **Then** that action has no key and its former
   default no longer triggers it.

---

### User Story 4 - Per-mode keymaps and live `/keys` and `/reload-config` (Priority: P3)

A user organizes bindings by mode (anticipating LAAT in sprint 008), inspects the
**effective** map at any time with `/keys`, and re-applies an edited config
without restarting via `/reload-config`.

**Why this priority**: Per-mode sections and on-demand reload are the
forward-looking conveniences that make the engine pleasant and set up sprint 008,
but they are not required for the core rebinding value, so they come last.

**Independent Test**: Define a mode-specific keymap section and confirm its
bindings apply in that mode while the default-mode bindings apply otherwise; run
`/keys` and confirm it lists the live effective map (including configured
changes); edit the config and run `/reload-config` and confirm the new bindings
take effect without restarting.

**Acceptance Scenarios**:

1. **Given** a config with a per-mode keymap section, **When** kapollo is in that
   mode, **Then** the mode's bindings apply and fall back to the default-mode
   bindings for actions the mode does not override.
2. **Given** a running kapollo and a freshly edited config, **When** the user
   runs `/reload-config`, **Then** the keymap is re-read and the new bindings take
   effect without restarting; a malformed reload reports the error and leaves the
   previous map in place.
3. **Given** any effective keymap (default or configured), **When** the user runs
   `/keys`, **Then** the listing reflects the live bindings, including primary and
   alternate keys and which actions are unbound.

---

### Edge Cases

- **Same key, primary and alternate collapse**: if an action's primary and
  alternate resolve to the same chord, the duplicate is harmless (the action
  still fires); no conflict warning is needed against itself.
- **Reserved/contextual gestures**: actions that are contextual in the event loop
  (e.g. `Esc Esc` clearing the status message, `Ctrl+C` copy-or-interrupt, plain
  `Enter` submit) remain context-sensitive; the keymap surfaces them in `/keys`
  but rebinding contextual gestures is out of scope unless they already have a
  chord.
- **Clearing a contextual binding**: clearing a binding that has no standalone
  chord (a contextual gesture) has no effect and is not an error.
- **Reload mid-input**: running `/reload-config` while a buffer is being composed
  must not lose the in-progress input.
- **Unknown mode section**: a keymap section naming a mode kapollo does not know
  is warned about and ignored, not fatal.
- **Partial mode override**: a mode that binds only some actions inherits the
  rest from the default map.
- **Conflict across primary/alternate slots**: a conflict between one action's
  alternate and another action's primary is detected the same as primary-vs-
  primary.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Every kapollo behavior exposed as a named action MUST be bindable
  from configuration, including all sprint-005 input-editing, selection, and
  scrollback actions and the existing sprint-004 copy actions.
- **FR-002**: With no keymap configuration present, the effective keymap MUST be
  identical to the hardcoded sprint-004/005 defaults so out-of-the-box behavior
  is unchanged.
- **FR-003**: The system MUST support a **primary and an optional alternate** key
  binding per action, expressed in config as an array of key strings (first =
  primary, second = alternate); a bare string MUST be accepted as a primary with
  no alternate.
- **FR-004**: The default keymap MUST ship an alternate where the established
  behavior has one (notably newline insertion: `Shift+Enter` primary,
  `Alt+Enter` alternate).
- **FR-005**: The previously-unbound sprint-004 copy variants
  `copy_block_without_command` and `copy_current_line` MUST have default key
  bindings. Because these actions were previously mouse-targeted (the click
  supplied a row), their keyboard bindings MUST act on the **most recent
  (bottom-most) transcript output**: `copy_block_without_command` copies the most
  recently completed command block's output, and `copy_current_line` copies the
  newest (last) transcript line.
- **FR-006**: The system MUST parse human-writable key strings into the internal
  key representation, accepting canonical short modifier names (`Ctrl`, `Alt`,
  `Shift`) and the key names already surfaced by `/keys` (e.g. `Left`, `Right`,
  `Home`, `End`, `PageUp`, `PageDown`, `Enter`, printable characters).
- **FR-007**: Key-string parsing MUST be case-insensitive (e.g. `Ctrl+Left`,
  `ctrl+left`, and `CTRL+LEFT` resolve to the same chord) and tolerant of
  modifier order.
- **FR-008**: The system MUST support the `Esc Esc` two-key chord as a bindable
  key sequence; multi-key chords beyond `Esc Esc` are out of scope this sprint.
  This sprint `Esc Esc` is **parse-recognized** and **surfaced in `/keys`** (as
  the `clear_status_message` gesture); its dispatch remains the existing
  context-sensitive handler (see FR-018), so a config binding of `Esc Esc` is
  recognized and listed but does not replace the contextual gesture logic.
- **FR-009**: An unparseable or unrecognized key string MUST produce a warning
  that identifies the offending binding, MUST be skipped, and MUST NOT prevent
  kapollo from starting.
- **FR-010**: When two actions are bound to the same key, the system MUST emit a
  conflict warning naming both actions and apply a **last-declared-wins** policy
  (warn + last-wins); kapollo MUST still start.
- **FR-011**: Clearing an action's binding in config (an empty binding value)
  MUST disable that action's key path so its former default no longer triggers
  it (disable-by-clearing).
- **FR-012**: The configuration schema MUST provide a keymap table that maps
  action names to bindings and MUST support **per-mode** keymap sections; a mode
  section overrides only the actions it lists and inherits the rest from the
  default-mode map.
- **FR-013**: A keymap section that names an unknown mode, or a binding that
  names an unknown action, MUST be warned about and ignored, not fatal.
- **FR-014**: `/keys` MUST reflect the **live effective** keymap, including each
  action's primary and alternate keys and which actions are unbound, replacing
  the hardcoded listing from sprint 005.
- **FR-015**: The system MUST provide a slash command (`/reload-config`) that
  re-reads configuration on demand and applies the updated keymap without
  restarting; there MUST NOT be automatic file watching.
- **FR-016**: A reload that encounters a malformed config MUST report the error
  and leave the previously effective keymap in place (no partial application that
  breaks input).
- **FR-017**: A reload MUST NOT discard an in-progress input buffer or otherwise
  disrupt the current composing/selection state.
- **FR-018**: Contextual gestures whose meaning depends on runtime state (plain
  `Enter` submit, `Ctrl+C` copy-or-interrupt, `Esc`/`Esc Esc` cancel/clear) MUST
  remain context-sensitive and MUST be surfaced in `/keys`; rebinding of purely
  contextual gestures (those without a standalone chord) is out of scope.
- **FR-019**: All default key bindings and the keymap config surface (table
  shape, per-mode sections, alternate-array form, disable-by-clearing,
  `/reload-config`) MUST be documented in user-facing docs.

### Key Entities *(include if feature involves data)*

- **Action**: a named, stable behavior (e.g. `word_move_left`, `scroll_line_up`,
  `copy_current_line`). Carries a stable name surfaced by `/keys` and config. The
  full set is fixed in code; config binds keys to these names but does not create
  new actions.
- **Key chord**: a parsed key plus its modifier bits, or the `Esc Esc` two-key
  sequence. The normalized internal target of a key string. (Realized in the
  design as `KeySpec`, which distinguishes a single-key chord from the `Esc Esc`
  two-key chord; see [data-model.md](data-model.md) §3.)
- **Binding**: the association of an action to a primary key and an optional
  alternate key (or to nothing, when cleared).
- **Keymap**: the full set of bindings in effect for a mode, derived from the
  default map overlaid by any configured overrides; the effective keymap is what
  `/keys` reports and the event loop resolves against.
- **Mode keymap section**: a per-mode group of bindings in config that overrides
  the default-mode map for the actions it lists.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user running kapollo with no keymap configuration experiences
  100% of sprint-004/005 key behaviors unchanged.
- **SC-002**: A user can rebind any of the named actions to a key of their choice
  by editing config alone, with no code changes, and the new key works on the
  next load or `/reload-config`.
- **SC-003**: Every named action — including the two previously-unbound copy
  variants — has a discoverable binding (or an explicit "unbound" state) visible
  via `/keys`.
- **SC-004**: An invalid binding or a binding conflict never prevents kapollo
  from starting; 100% of such cases produce a warning and a usable session.
- **SC-005**: A user can apply an edited keymap without restarting kapollo, and a
  malformed edit never leaves the session with a broken key map.
- **SC-006**: Key strings are accepted regardless of letter case or modifier
  order, so equivalent spellings always resolve to the same binding.

## Assumptions

- The named-action registry from sprint 005 is the authoritative action set;
  this sprint extends its binding mechanism but does not add new editing
  behaviors.
- Modes are anticipated (LAAT, sprint 008) but only the default mode exists in
  practice this sprint; per-mode support is built and tested structurally so 008
  can use it, with the default mode as the only populated map.
- Mouse-binding configuration is out of scope; only key bindings are configurable.
- The config file format remains the existing TOML surface; the keymap is a new
  table within it, consistent with how prior sprints added `[status]`,
  `[divider]`, and `[scroll]` sections.
- `/reload-config` re-reads the same config path kapollo was started with.
- Out-of-the-box defaults that ship an alternate are limited to cases with an
  established legacy-terminal fallback (newline insertion); most actions ship a
  primary only.
