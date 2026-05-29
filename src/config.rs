//! Configuration loading. kapollo runs entirely on defaults when no config
//! file is present (FR-028). See `contracts/config.md` for the authoritative
//! schema. Unknown keys are logged and ignored; out-of-range caps are clamped.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::ConfigError;

/// Hard maximum for the per-block byte cap (64 MiB). Larger values are clamped.
pub const PER_BLOCK_BYTES_HARD_MAX: u64 = 64 * 1024 * 1024;

const DEFAULT_LEADER_CHAR: char = '/';
const DEFAULT_PER_BLOCK_BYTES: u64 = 1024 * 1024; // 1 MiB
const DEFAULT_PER_BLOCK_LINES: u64 = 50_000;
const DEFAULT_TRANSCRIPT_BYTES: u64 = 128 * 1024 * 1024; // 128 MiB
const DEFAULT_TRANSCRIPT_BLOCKS: u64 = 1_000;

const TOP_LEVEL_KEYS: &[&str] = &["shell", "leader_char", "caps"];
const CAPS_KEYS: &[&str] = &[
    "per_block_bytes",
    "per_block_lines",
    "transcript_bytes",
    "transcript_blocks",
];

/// Effective kapollo configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Shell to wrap. `None` means fall back to `$SHELL` at spawn time.
    pub shell: Option<String>,
    /// Leader character that begins a slash command.
    pub leader_char: char,
    /// Output retention caps.
    pub caps: Caps,
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
            caps: Caps::default(),
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
    caps: Option<RawCaps>,
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
            caps,
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
}
