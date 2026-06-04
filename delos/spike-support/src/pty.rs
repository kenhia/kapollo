//! PTY plumbing: spawn a shell under a pseudo-terminal, stream its output over a
//! channel, and expose a writer + resize handle. Mirrors kapollo's approach but is
//! self-contained (R7). This is an I/O boundary — no unit tests; it is smoke-validated
//! by the first stage slice.

use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver};
use std::thread;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

pub use portable_pty::PtySize as PtySizeReexport;

/// A running shell attached to a PTY. Output bytes arrive on `output`; input is sent
/// via [`PtyShell::write`]; the window can be resized with [`PtyShell::resize`].
pub struct PtyShell {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    /// Receives raw byte chunks read from the child. Closed when the child exits.
    pub output: Receiver<Vec<u8>>,
}

impl PtyShell {
    /// Spawn `shell` under a fresh PTY of the given size, starting a reader thread that
    /// forwards output chunks to the `output` channel.
    pub fn spawn(shell: &str, size: PtySize) -> anyhow::Result<Self> {
        let pair = native_pty_system().openpty(size)?;
        let cmd = CommandBuilder::new(shell);
        let child = pair.slave.spawn_command(cmd)?;
        // Drop the slave once the child holds it, so EOF propagates on child exit.
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let (tx, output) = channel::<Vec<u8>>();

        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            master: pair.master,
            writer,
            child,
            output,
        })
    }

    /// Write input bytes to the child.
    pub fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(data)?;
        self.writer.flush()
    }

    /// Resize the PTY window, propagating SIGWINCH to the child.
    pub fn resize(&self, size: PtySize) -> anyhow::Result<()> {
        self.master.resize(size)?;
        Ok(())
    }

    /// Non-blocking check whether the child has exited.
    pub fn try_wait(&mut self) -> anyhow::Result<bool> {
        Ok(self.child.try_wait()?.is_some())
    }
}
