//! Configuration loading. kapollo runs entirely on defaults when no config
//! file is present (FR-028). See `contracts/config.md` for the authoritative
//! schema. Unknown keys are logged and ignored; out-of-range caps are clamped.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use ratatui::style::Color;
use serde::Deserialize;

use crate::error::ConfigError;

/// Hard maximum for the per-block byte cap (64 MiB). Larger values are clamped.
pub const PER_BLOCK_BYTES_HARD_MAX: u64 = 64 * 1024 * 1024;

const DEFAULT_LEADER_CHAR: char = '/';
const DEFAULT_PROMPT_CHAR: char = 'λ';
const DEFAULT_PROMPT_COLOR: Color = Color::Red;
const DEFAULT_PER_BLOCK_BYTES: u64 = 1024 * 1024; // 1 MiB
const DEFAULT_PER_BLOCK_LINES: u64 = 50_000;
const DEFAULT_TRANSCRIPT_BYTES: u64 = 128 * 1024 * 1024; // 128 MiB
const DEFAULT_TRANSCRIPT_BLOCKS: u64 = 1_000;

// Grid-rework surface defaults (sprint 004). All chosen so kapollo behaves the
// same out of the box: mouse selection on, OSC 52 copy with a local fallback.
const DEFAULT_MOUSE_ENABLED: bool = true;
const DEFAULT_COPY_ON_SELECT: bool = false;
const DEFAULT_CLIPBOARD_OSC52: bool = true;
const DEFAULT_CLIPBOARD_LOCAL_FALLBACK: bool = true;
const DEFAULT_WHEEL_LINES: u16 = 3;
const DEFAULT_SCROLLBACK_LINES: u64 = 10_000;
// Sprint 005 surface defaults: the fixed status bar is shown, and page
// scrolling keeps three lines of context across a page boundary.
const DEFAULT_STATUS_ENABLED: bool = true;
const DEFAULT_CONTEXT_LINES: u16 = 3;
// The cosmetic dividing rule between the output and input pads (the Apollo /
// Domain OS lineage) is shown by default.
const DEFAULT_DIVIDER_ENABLED: bool = true;

const TOP_LEVEL_KEYS: &[&str] = &[
    "shell",
    "leader_char",
    "prompt_char",
    "prompt_color",
    "caps",
    "mouse",
    "clipboard",
    "scroll",
    "status",
    "divider",
];
const CAPS_KEYS: &[&str] = &[
    "per_block_bytes",
    "per_block_lines",
    "transcript_bytes",
    "transcript_blocks",
];
const MOUSE_KEYS: &[&str] = &["enabled", "copy_on_select"];
const CLIPBOARD_KEYS: &[&str] = &["osc52", "local_fallback"];
const SCROLL_KEYS: &[&str] = &["wheel_lines", "scrollback_lines", "context_lines"];
const STATUS_KEYS: &[&str] = &["enabled"];
const DIVIDER_KEYS: &[&str] = &["enabled"];

/// Effective kapollo configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Shell to wrap. `None` means fall back to `$SHELL` at spawn time.
    pub shell: Option<String>,
    /// Leader character that begins a slash command.
    pub leader_char: char,
    /// Prompt character echoed before each command (default `λ`; FR-010).
    pub prompt_char: char,
    /// Color applied to the prompt character when color is enabled
    /// (default red; FR-011).
    pub prompt_color: Color,
    /// Output retention caps.
    pub caps: Caps,
    /// Mouse capture / selection behavior (sprint 004, D28).
    pub mouse: Mouse,
    /// Clipboard copy path and fallback order (sprint 004, D28).
    pub clipboard: Clipboard,
    /// Scroll / scrollback behavior (sprint 004).
    pub scroll: Scroll,
    /// Fixed status bar behavior (sprint 005).
    pub status: Status,
    /// Cosmetic dividing rule between the output and input pads (sprint 005).
    pub divider: Divider,
}

/// Mouse capture / selection behavior (FR-013, FR-017).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mouse {
    /// Whether kapollo captures the mouse for selection. When `false`, mouse
    /// events are left to the host terminal.
    pub enabled: bool,
    /// Copy the selection to the clipboard automatically on release.
    pub copy_on_select: bool,
}

/// Clipboard copy path and fallback order (FR-020, FR-021).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clipboard {
    /// Use terminal-mediated OSC 52 copy (SSH-friendly). Tried first.
    pub osc52: bool,
    /// Fall back to the local OS clipboard (`arboard`) when OSC 52 is off or
    /// unavailable.
    pub local_fallback: bool,
}

/// Scroll / scrollback behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scroll {
    /// Lines advanced per mouse-wheel notch.
    pub wheel_lines: u16,
    /// Number of scrollback lines the grid retains.
    pub scrollback_lines: u64,
    /// Lines of overlap kept when paging through scrollback (sprint 005).
    pub context_lines: u16,
}

/// Fixed status bar behavior (sprint 005).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// Whether the fixed status bar below the input pad is shown.
    pub enabled: bool,
}

/// Cosmetic dividing rule between the output and input pads (sprint 005). Purely
/// decorative today; it is the visual lineage back to Apollo / Domain OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divider {
    /// Whether the dividing rule above the input pad is shown.
    pub enabled: bool,
}

/// Output retention caps (ring-buffer semantics; FR-016).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caps {
    pub per_block_bytes: u64,
    pub per_block_lines: u64,
    pub transcript_bytes: u64,
    pub transcript_blocks: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            shell: None,
            leader_char: DEFAULT_LEADER_CHAR,
            prompt_char: DEFAULT_PROMPT_CHAR,
            prompt_color: DEFAULT_PROMPT_COLOR,
            caps: Caps::default(),
            mouse: Mouse::default(),
            clipboard: Clipboard::default(),
            scroll: Scroll::default(),
            status: Status::default(),
            divider: Divider::default(),
        }
    }
}

impl Default for Mouse {
    fn default() -> Self {
        Self {
            enabled: DEFAULT_MOUSE_ENABLED,
            copy_on_select: DEFAULT_COPY_ON_SELECT,
        }
    }
}

impl Default for Clipboard {
    fn default() -> Self {
        Self {
            osc52: DEFAULT_CLIPBOARD_OSC52,
            local_fallback: DEFAULT_CLIPBOARD_LOCAL_FALLBACK,
        }
    }
}

impl Default for Scroll {
    fn default() -> Self {
        Self {
            wheel_lines: DEFAULT_WHEEL_LINES,
            scrollback_lines: DEFAULT_SCROLLBACK_LINES,
            context_lines: DEFAULT_CONTEXT_LINES,
        }
    }
}

impl Default for Status {
    fn default() -> Self {
        Self {
            enabled: DEFAULT_STATUS_ENABLED,
        }
    }
}

impl Default for Divider {
    fn default() -> Self {
        Self {
            enabled: DEFAULT_DIVIDER_ENABLED,
        }
    }
}

impl Default for Caps {
    fn default() -> Self {
        Self {
            per_block_bytes: DEFAULT_PER_BLOCK_BYTES,
            per_block_lines: DEFAULT_PER_BLOCK_LINES,
            transcript_bytes: DEFAULT_TRANSCRIPT_BYTES,
            transcript_blocks: DEFAULT_TRANSCRIPT_BLOCKS,
        }
    }
}

impl Config {
    /// Load configuration from `path`, or from the default XDG location when
    /// `path` is `None`. Returns defaults when the file does not exist.
    pub fn load(path: Option<&Path>) -> Result<Config, ConfigError> {
        let resolved = match path {
            Some(p) => p.to_path_buf(),
            None => match default_config_path() {
                Some(p) => p,
                None => return Ok(Config::default()),
            },
        };

        if !resolved.exists() {
            return Ok(Config::default());
        }

        let text = std::fs::read_to_string(&resolved).map_err(|source| ConfigError::Read {
            path: resolved.clone(),
            source,
        })?;
        Config::from_toml(&text, &resolved)
    }

    /// Parse configuration from an in-memory TOML string. `path` is used only
    /// for error messages.
    pub fn from_toml(text: &str, path: &Path) -> Result<Config, ConfigError> {
        let table: toml::Table = toml::from_str(text).map_err(|e| ConfigError::Parse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        warn_unknown_keys(&table);

        let raw: RawConfig = toml::from_str(text).map_err(|e| ConfigError::Parse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        raw.into_config()
    }
}

/// Default config path: `~/.config/kapollo/config.toml` (XDG).
pub fn default_config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "kapollo")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    shell: Option<String>,
    leader_char: Option<String>,
    prompt_char: Option<String>,
    prompt_color: Option<String>,
    caps: Option<RawCaps>,
    mouse: Option<RawMouse>,
    clipboard: Option<RawClipboard>,
    scroll: Option<RawScroll>,
    status: Option<RawStatus>,
    divider: Option<RawDivider>,
}

#[derive(Debug, Default, Deserialize)]
struct RawMouse {
    enabled: Option<bool>,
    copy_on_select: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct RawClipboard {
    osc52: Option<bool>,
    local_fallback: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct RawScroll {
    wheel_lines: Option<u16>,
    scrollback_lines: Option<u64>,
    context_lines: Option<u16>,
}

#[derive(Debug, Default, Deserialize)]
struct RawStatus {
    enabled: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct RawDivider {
    enabled: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct RawCaps {
    per_block_bytes: Option<u64>,
    per_block_lines: Option<u64>,
    transcript_bytes: Option<u64>,
    transcript_blocks: Option<u64>,
}

impl RawConfig {
    fn into_config(self) -> Result<Config, ConfigError> {
        let leader_char = match self.leader_char {
            Some(s) => {
                let mut chars = s.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => c,
                    _ => {
                        return Err(ConfigError::Value(format!(
                            "leader_char must be exactly one character, got {s:?}"
                        )))
                    }
                }
            }
            None => DEFAULT_LEADER_CHAR,
        };

        let prompt_char = match self.prompt_char {
            Some(s) => {
                let mut chars = s.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => c,
                    _ => {
                        return Err(ConfigError::Value(format!(
                            "prompt_char must be exactly one character, got {s:?}"
                        )))
                    }
                }
            }
            None => DEFAULT_PROMPT_CHAR,
        };

        let prompt_color = match self.prompt_color {
            Some(s) => match Color::from_str(&s) {
                Ok(color) => color,
                Err(_) => {
                    tracing::warn!(value = %s, "unknown prompt_color; using default");
                    DEFAULT_PROMPT_COLOR
                }
            },
            None => DEFAULT_PROMPT_COLOR,
        };

        let defaults = Caps::default();
        let raw_caps = self.caps.unwrap_or_default();
        let mut caps = Caps {
            per_block_bytes: raw_caps.per_block_bytes.unwrap_or(defaults.per_block_bytes),
            per_block_lines: raw_caps.per_block_lines.unwrap_or(defaults.per_block_lines),
            transcript_bytes: raw_caps
                .transcript_bytes
                .unwrap_or(defaults.transcript_bytes),
            transcript_blocks: raw_caps
                .transcript_blocks
                .unwrap_or(defaults.transcript_blocks),
        };

        if caps.per_block_bytes > PER_BLOCK_BYTES_HARD_MAX {
            tracing::warn!(
                requested = caps.per_block_bytes,
                clamped_to = PER_BLOCK_BYTES_HARD_MAX,
                "per_block_bytes exceeds hard maximum; clamping"
            );
            caps.per_block_bytes = PER_BLOCK_BYTES_HARD_MAX;
        }

        Ok(Config {
            shell: self.shell,
            leader_char,
            prompt_char,
            prompt_color,
            caps,
            mouse: {
                let raw = self.mouse.unwrap_or_default();
                let d = Mouse::default();
                Mouse {
                    enabled: raw.enabled.unwrap_or(d.enabled),
                    copy_on_select: raw.copy_on_select.unwrap_or(d.copy_on_select),
                }
            },
            clipboard: {
                let raw = self.clipboard.unwrap_or_default();
                let d = Clipboard::default();
                Clipboard {
                    osc52: raw.osc52.unwrap_or(d.osc52),
                    local_fallback: raw.local_fallback.unwrap_or(d.local_fallback),
                }
            },
            scroll: {
                let raw = self.scroll.unwrap_or_default();
                let d = Scroll::default();
                Scroll {
                    wheel_lines: raw.wheel_lines.unwrap_or(d.wheel_lines),
                    scrollback_lines: raw.scrollback_lines.unwrap_or(d.scrollback_lines),
                    context_lines: raw.context_lines.unwrap_or(d.context_lines),
                }
            },
            status: {
                let raw = self.status.unwrap_or_default();
                let d = Status::default();
                Status {
                    enabled: raw.enabled.unwrap_or(d.enabled),
                }
            },
            divider: {
                let raw = self.divider.unwrap_or_default();
                let d = Divider::default();
                Divider {
                    enabled: raw.enabled.unwrap_or(d.enabled),
                }
            },
        })
    }
}

fn warn_unknown_keys(table: &toml::Table) {
    for key in table.keys() {
        if !TOP_LEVEL_KEYS.contains(&key.as_str()) {
            tracing::warn!(key = %key, "unknown config key ignored");
        }
    }
    if let Some(toml::Value::Table(caps)) = table.get("caps") {
        for key in caps.keys() {
            if !CAPS_KEYS.contains(&key.as_str()) {
                tracing::warn!(key = %key, "unknown config key in [caps] ignored");
            }
        }
    }
    if let Some(toml::Value::Table(mouse)) = table.get("mouse") {
        for key in mouse.keys() {
            if !MOUSE_KEYS.contains(&key.as_str()) {
                tracing::warn!(key = %key, "unknown config key in [mouse] ignored");
            }
        }
    }
    if let Some(toml::Value::Table(clipboard)) = table.get("clipboard") {
        for key in clipboard.keys() {
            if !CLIPBOARD_KEYS.contains(&key.as_str()) {
                tracing::warn!(key = %key, "unknown config key in [clipboard] ignored");
            }
        }
    }
    if let Some(toml::Value::Table(scroll)) = table.get("scroll") {
        for key in scroll.keys() {
            if !SCROLL_KEYS.contains(&key.as_str()) {
                tracing::warn!(key = %key, "unknown config key in [scroll] ignored");
            }
        }
    }
    if let Some(toml::Value::Table(status)) = table.get("status") {
        for key in status.keys() {
            if !STATUS_KEYS.contains(&key.as_str()) {
                tracing::warn!(key = %key, "unknown config key in [status] ignored");
            }
        }
    }
    if let Some(toml::Value::Table(divider)) = table.get("divider") {
        for key in divider.keys() {
            if !DIVIDER_KEYS.contains(&key.as_str()) {
                tracing::warn!(key = %key, "unknown config key in [divider] ignored");
            }
        }
    }
}
