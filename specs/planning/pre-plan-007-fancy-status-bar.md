# Pre-plan 007 — Templated ("fancy") status bar

> Source: `.scratch/005-pre-planning.md`. Sprint 005 ships a fixed-format status
> bar; this sprint makes its content user-templatable.

## Goal

Let users define the status line's content and formatting via a small template
string, replacing the fixed format from sprint 005.

## In scope

- **Template engine** for the status line. The pre-plan sketched a Django-ish
  syntax (illustrative, with typos) — final syntax to be chosen by research into
  what's easy and idiomatic on our Rust stack (see Q1).

  Illustrative example (not a spec):
  ```
  "| {{ kap_mode|full }} | {{ cwd|trim }} {padfill}{{ message }} | {{ exit|color,on_error }} |"
  ```
  Producing something like:
  ```
  | LaaT | ~/src/kapollo/target/release          copied block w. cmd | exit 1 |
  ```

- **Variables** (at least): `kap_mode`, `cwd`, `message`, `exit`. Extend as
  needed.
- **Filters** (candidate set): `full` / short (mode long vs. 2-char form),
  `trim` (path shortening), `color,on_error` (conditional styling by exit code),
  and a `padfill`/justify mechanism for left/right alignment within the width.
- **Width-aware rendering** — fill/pad to the terminal column count; define
  truncation when content overflows.
- Config key for the template; validation with a helpful error on bad syntax.

## Decisions (resolved in pre-planning)

- The Django-ish notation was **flavor only** (and had typos). The actual syntax
  is an open design choice, optimized for ease of use on our stack — not a
  commitment to that exact grammar.

## Out of scope

- The on/off toggle, `<10`-row hide, mode field, and `/status` command — those
  ship in **005** with the fixed bar; this sprint only adds templating.

## Open questions

- **Q1 — Template syntax & library.** Research: hand-rolled mini-parser vs. an
  existing crate (e.g. a minimal `{{ }}` templating lib). Pick for simplicity
  and small dependency footprint (Constitution VII). What's the minimum filter
  set worth shipping?
- **Q2 — `padfill`/alignment model.** How is left/right/center justification
  expressed, and how does it interact with `{{ }}` substitutions? (The example
  mixed `{padfill}` and `{{ }}` delimiters — unify or define both deliberately.)
- **Q3 — Color/style vocabulary.** How users express colors and conditional
  styling (`color,on_error`) in the template, mapped to ratatui styles.
- **Q4 — Overflow.** Truncate which field first when the rendered line exceeds
  the width? (Likely `cwd` via `trim`, then `message`.)

## Dependencies / sequencing

- **Depends on 005** (status bar infrastructure, variable sources, mode field).
- Independent of 006 and 008.
