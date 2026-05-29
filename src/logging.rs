//! File-sink logging via `tracing`. Logs never go to the TUI surface
//! (Constitution VI, FR-030); they are written to a file under the XDG state
//! directory. Default verbosity is quiet; `--verbose`/`KAPOLLO_LOG` raise it.

use std::path::PathBuf;

use anyhow::Context;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

/// Initialize the global logging subscriber.
///
/// Returns a [`WorkerGuard`] that MUST be kept alive for the duration of the
/// program so the non-blocking appender can flush.
pub fn init(verbose: u8) -> anyhow::Result<WorkerGuard> {
    let dir = log_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create log directory {dir:?}"))?;

    let appender = tracing_appender::rolling::never(&dir, "kapollo.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let default_level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter =
        EnvFilter::try_from_env("KAPOLLO_LOG").unwrap_or_else(|_| EnvFilter::new(default_level));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_ansi(false),
        )
        .init();

    Ok(guard)
}

fn log_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "kapollo")
        .map(|dirs| {
            dirs.state_dir()
                .unwrap_or_else(|| dirs.data_local_dir())
                .to_path_buf()
        })
        .unwrap_or_else(|| PathBuf::from(".kapollo"))
}
