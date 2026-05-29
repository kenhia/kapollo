//! Library error types. App-level errors use `anyhow`; library boundaries use
//! these `thiserror` types so callers can match on failure modes.

use std::path::PathBuf;

/// Errors that can occur while loading or validating configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The config file exists but could not be read.
    #[error("failed to read config file {path:?}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The config file is not valid TOML.
    #[error("invalid TOML in config file {path:?}: {message}")]
    Parse { path: PathBuf, message: String },
    /// A config value was syntactically valid TOML but semantically invalid.
    #[error("invalid config value: {0}")]
    Value(String),
}
