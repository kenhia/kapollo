//! kapollo — an Apollo-DM-style split-pad terminal REPL that wraps the user's
//! real shell. This crate exposes the application as a library so both the
//! `kapollo` and `kap` binaries can share the same wiring.

pub mod app;
pub mod clipboard;
pub mod config;
pub mod error;
pub mod grid;
pub mod input;
pub mod logging;
pub mod output;
pub mod pty;
pub mod selection;
pub mod session;
pub mod slash;
pub mod ui;

use std::io::{self, IsTerminal};
use std::path::PathBuf;

use anyhow::Context;

use crate::app::App;
use crate::config::Config;

/// Parsed command-line invocation.
enum Cli {
    Run(RunOpts),
    Help,
    Version,
}

/// Options that configure a run of the TUI.
#[derive(Debug, Default)]
struct RunOpts {
    shell: Option<String>,
    config: Option<PathBuf>,
    verbose: u8,
}

/// Shared entry point used by both the `kapollo` and `kap` binaries.
pub fn run_cli() -> anyhow::Result<()> {
    match parse_args(std::env::args().skip(1))? {
        Cli::Help => {
            print_help();
            Ok(())
        }
        Cli::Version => {
            println!("kapollo {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Cli::Run(opts) => run_app(opts),
    }
}

fn run_app(opts: RunOpts) -> anyhow::Result<()> {
    // Logging must be initialized before anything that might log. The guard
    // keeps the background appender thread alive for the duration of the run.
    let _log_guard = logging::init(opts.verbose).context("failed to initialize logging")?;

    let mut config =
        Config::load(opts.config.as_deref()).context("failed to load configuration")?;
    if let Some(shell) = opts.shell {
        config.shell = Some(shell);
    }

    // Refuse to draw a TUI when stdout is not a terminal (FR-032).
    if !io::stdout().is_terminal() {
        eprintln!("kapollo: stdout is not a TTY; refusing to start the TUI.");
        anyhow::bail!("stdout is not a TTY");
    }

    // RAII guard restores the terminal on every exit path; the panic hook
    // restores it during unwinding as well (FR-025, FR-026).
    let _terminal_guard = ui::TerminalGuard::enter().context("failed to set up terminal")?;
    ui::install_panic_hook();

    let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
    let mut terminal = ratatui::Terminal::new(backend).context("failed to create terminal")?;

    let mut app = App::new(config).context("failed to start kapollo")?;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.run(&mut terminal)));

    match outcome {
        Ok(result) => result,
        Err(_) => anyhow::bail!("kapollo panicked; terminal restored, see log for details"),
    }
}

fn parse_args<I: Iterator<Item = String>>(args: I) -> anyhow::Result<Cli> {
    let mut opts = RunOpts::default();
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Cli::Help),
            "-V" | "--version" => return Ok(Cli::Version),
            "-v" | "--verbose" => opts.verbose = opts.verbose.saturating_add(1),
            "--shell" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--shell requires a path argument"))?;
                opts.shell = Some(value);
            }
            "--config" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--config requires a path argument"))?;
                opts.config = Some(PathBuf::from(value));
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
    }
    Ok(Cli::Run(opts))
}

fn print_help() {
    println!(
        "kapollo {} — split-pad shell REPL\n\n\
         USAGE:\n    kap [OPTIONS]\n    kapollo [OPTIONS]\n\n\
         OPTIONS:\n\
         \x20   --shell <PATH>     Shell to wrap (overrides config and $SHELL)\n\
         \x20   --config <PATH>    Use an alternate config file\n\
         \x20   -v, --verbose      Raise log verbosity (repeatable)\n\
         \x20   -V, --version      Print version and exit\n\
         \x20   -h, --help         Print this help and exit",
        env!("CARGO_PKG_VERSION")
    );
}
