//! `spike-support` — shared, crate-agnostic plumbing and the unit-tested pure helpers
//! reused by all three terminal-grid spike stages (S1 `vt100`, S2 `alacritty_terminal`,
//! S3 `wezterm-term`). Keeping this thin and identical across stages is what makes the
//! scorecard comparison fair (see `specs/003-grid-spike/`).

pub mod clipboard;
pub mod coords;
pub mod modes;
pub mod pty;

pub use clipboard::{copy_local, osc52_frame};
pub use coords::{auto_scroll, content_to_screen, normalize, screen_to_content, Cell};
pub use modes::{detect_mode, ModeEvent};
pub use pty::{PtyShell, PtySizeReexport as PtySize};
