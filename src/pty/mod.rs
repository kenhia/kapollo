//! PTY/process layer: spawns the wrapped shell in a pseudo-terminal and
//! mediates byte I/O, resize, and child-exit. A dedicated reader thread feeds
//! PTY bytes to the single-threaded event loop over an `mpsc` channel
//! (research R1); the event loop never blocks on the shell.

pub mod shell;

pub use shell::{resolve_shell, ShellKind};

use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use portable_pty::{native_pty_system, ChildKiller, MasterPty, PtySize};

use crate::pty::shell::BashRcGuard;

/// How block boundaries are detected for this session (research R2/R3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryMode {
    /// OSC 133 semantic prompt marks (fish/bash hook installed).
    Osc133,
    /// Sentinel-nonce fallback for shells without a hook.
    Sentinel,
}

/// An event produced by the wrapped shell, delivered to the event loop.
#[derive(Debug)]
pub enum PtyEvent {
    /// Raw bytes read from the PTY master.
    Output(Vec<u8>),
    /// The wrapped shell exited; carries its exit code when known (FR-027).
    Exited(Option<i32>),
}

/// A live wrapped-shell session: the PTY master, an input writer, and the
/// receiving end of the reader thread's channel.
pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    receiver: Receiver<PtyEvent>,
    shell_kind: ShellKind,
    boundary_mode: BoundaryMode,
    nonce: String,
    _reader_thread: JoinHandle<()>,
    _bash_rc: Option<BashRcGuard>,
}

impl PtySession {
    /// Spawn the wrapped shell in a fresh PTY and start the reader thread.
    pub fn spawn(shell_override: Option<&str>) -> anyhow::Result<Self> {
        Self::spawn_with_size(shell_override, 24, 80)
    }

    /// Spawn the wrapped shell with an explicit initial terminal size.
    pub fn spawn_with_size(
        shell_override: Option<&str>,
        rows: u16,
        cols: u16,
    ) -> anyhow::Result<Self> {
        let shell_path = resolve_shell(shell_override);
        let shell_kind = ShellKind::detect(&shell_path);
        let nonce = make_nonce();
        let boundary_mode = match shell_kind {
            ShellKind::Fish | ShellKind::Bash => BoundaryMode::Osc133,
            ShellKind::Other => BoundaryMode::Sentinel,
        };

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("failed to open PTY")?;

        let (cmd, bash_rc) = shell::build_command(&shell_path, shell_kind, boundary_mode)?;
        let child = pair
            .slave
            .spawn_command(cmd)
            .context("failed to spawn shell")?;
        // The parent does not need the slave handle once the child holds it.
        drop(pair.slave);

        let killer = child.clone_killer();
        let reader = pair
            .master
            .try_clone_reader()
            .context("failed to clone PTY reader")?;
        let writer = pair
            .master
            .take_writer()
            .context("failed to take PTY writer")?;

        let (tx, rx) = mpsc::channel();
        let reader_thread = thread::spawn(move || reader_loop(reader, child, tx));

        Ok(Self {
            master: pair.master,
            writer,
            killer,
            receiver: rx,
            shell_kind,
            boundary_mode,
            nonce,
            _reader_thread: reader_thread,
            _bash_rc: bash_rc,
        })
    }

    /// The detected shell kind.
    pub fn shell_kind(&self) -> ShellKind {
        self.shell_kind
    }

    /// The active block-boundary detection mode.
    pub fn boundary_mode(&self) -> BoundaryMode {
        self.boundary_mode
    }

    /// The session-unique sentinel nonce (used in [`BoundaryMode::Sentinel`]).
    pub fn nonce(&self) -> &str {
        &self.nonce
    }

    /// Non-blocking poll for the next shell event.
    pub fn try_recv(&self) -> Result<PtyEvent, TryRecvError> {
        self.receiver.try_recv()
    }

    /// Blocking poll bounded by `timeout` (used by tests).
    pub fn recv_timeout(&self, timeout: Duration) -> Result<PtyEvent, RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }

    /// Write raw bytes to the shell's stdin.
    pub fn write_input(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()
    }

    /// Forward an interrupt (Ctrl-C) to the running command by writing the
    /// terminal interrupt character to the PTY (FR-024, research R7). The
    /// PTY's line discipline delivers SIGINT to the foreground process group,
    /// so the running command is interrupted rather than kapollo itself.
    pub fn send_interrupt(&mut self) -> std::io::Result<()> {
        self.write_input(&[0x03])
    }

    /// Submit a command line to the shell. In sentinel mode the command is
    /// wrapped so a nonce + exit status is emitted after it runs (research R3).
    pub fn send_command(&mut self, line: &str) -> std::io::Result<()> {
        match self.boundary_mode {
            BoundaryMode::Osc133 => {
                self.writer.write_all(line.as_bytes())?;
                self.writer.write_all(b"\n")?;
            }
            BoundaryMode::Sentinel => {
                // Best-effort POSIX wrapper (documented limitation, R3).
                let wrapped = format!("{line}\nprintf '%s;%d\\n' '{}' \"$?\"\n", self.nonce);
                self.writer.write_all(wrapped.as_bytes())?;
            }
        }
        self.writer.flush()
    }

    /// Forward a new terminal size to the PTY (FR-017, FR-019).
    pub fn resize(&self, rows: u16, cols: u16) -> std::io::Result<()> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| std::io::Error::other(e.to_string()))
    }

    /// The foreground process group leader, used for SIGINT forwarding (R7).
    pub fn process_group_leader(&self) -> Option<i32> {
        self.master.process_group_leader()
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        // Terminate the wrapped shell so the reader thread's blocking read
        // returns and the process group is cleaned up.
        let _ = self.killer.kill();
    }
}

fn reader_loop(
    mut reader: Box<dyn Read + Send>,
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
    tx: Sender<PtyEvent>,
) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if tx.send(PtyEvent::Output(buf[..n].to_vec())).is_err() {
                    return;
                }
            }
            Err(_) => break,
        }
    }
    let code = child.wait().ok().map(|status| status.exit_code() as i32);
    let _ = tx.send(PtyEvent::Exited(code));
}

fn make_nonce() -> String {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("__KAPOLLO_SENTINEL_{pid:x}_{nanos:x}__")
}
