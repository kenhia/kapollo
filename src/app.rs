//! The application: top-level state and the single-threaded event loop. The
//! loop is the panic boundary and the only place that mutates UI state
//! (research R1). It interleaves keyboard/resize events with PTY output drained
//! from the reader thread's channel.

use std::time::Duration;

use anyhow::Context;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::backend::Backend;
use ratatui::Terminal;

use crate::config::Config;
use crate::input::router::{self, Routed};
use crate::input::{InputHistory, InputPad};
use crate::output::{Boundary, OutputProcessor};
use crate::pty::{PtyEvent, PtySession};
use crate::session::{BlockId, Transcript};
use crate::slash::{self, builtins, Dispatch, SlashCommand};

/// How long to wait for a keyboard event before draining the PTY and redrawing.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Per-drain-pass budget so the event loop returns to service key input
/// (including Ctrl-C) promptly during heavy output (FR-015, FR-017; research R5).
const MAX_DRAIN_BYTES: usize = 256 * 1024;
const MAX_DRAIN_CHUNKS: usize = 64;

/// Top-level application state.
pub struct App {
    pub config: Config,
    pub transcript: Transcript,
    pub input: InputPad,
    pub last_exit: Option<i32>,
    /// Current working directory shown on the status rule, updated from OSC 7
    /// (FR-019); initialized from the process's startup directory.
    pub cwd: std::path::PathBuf,
    history: InputHistory,
    shell: PtySession,
    processor: OutputProcessor,
    current_block: Option<BlockId>,
    /// Whether a full-screen program currently owns the terminal (US3).
    passthrough: bool,
    should_quit: bool,
}

impl App {
    /// Construct the application, spawning the wrapped shell.
    pub fn new(config: Config) -> anyhow::Result<Self> {
        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
        let shell = PtySession::spawn_with_size(config.shell.as_deref(), rows, cols)
            .context("failed to start the wrapped shell")?;
        let processor = OutputProcessor::for_mode(shell.boundary_mode(), shell.nonce());
        let transcript = Transcript::new(config.caps.clone());
        Ok(Self {
            config,
            transcript,
            input: InputPad::new(),
            last_exit: None,
            cwd: std::env::current_dir().unwrap_or_default(),
            history: InputHistory::new(),
            shell,
            processor,
            current_block: None,
            passthrough: false,
            should_quit: false,
        })
    }

    /// Run the event loop until the user quits or the wrapped shell exits.
    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> anyhow::Result<()> {
        let mut output_pending = false;
        let mut prev_passthrough = false;
        while !self.should_quit {
            // On entering passthrough, switch stdin to non-blocking so terminal
            // query/responses can be forwarded to the program verbatim (FR-012).
            if self.passthrough && !prev_passthrough {
                let _ = crate::ui::passthrough::set_stdin_nonblocking(true);
            }

            // While a full-screen program owns the terminal, kapollo suspends
            // its own rendering; the program's raw output is passed through in
            // `drain_shell` (FR-018).
            if !self.passthrough {
                terminal.draw(|frame| crate::ui::render(frame, self))?;
            }

            if self.passthrough {
                // Forward raw stdin to the child verbatim, bypassing KeyEvent
                // decoding so OSC/cursor/DA responses are not mangled (FR-012).
                self.forward_passthrough_input(output_pending)?;
            } else {
                // When output is still backing up, poll without blocking so a
                // pending key (e.g. Ctrl-C) is read before the next bounded
                // drain pass instead of waiting out the full interval
                // (FR-015, FR-017).
                let poll_timeout = if output_pending {
                    Duration::ZERO
                } else {
                    POLL_INTERVAL
                };
                if event::poll(poll_timeout)? {
                    match event::read()? {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            self.on_key(key);
                        }
                        // Forward resize to the PTY so full-screen programs
                        // reflow live (FR-017, FR-019, SC-008).
                        Event::Resize(cols, rows) => {
                            let _ = self.shell.resize(rows, cols);
                        }
                        _ => {}
                    }
                }
            }

            let was_passthrough = self.passthrough;
            output_pending = self.drain_shell();
            // On returning from a full-screen program, reset the host terminal
            // (clear residual SGR/cursor state) and repaint the split-pad UI
            // with the prior transcript intact (FR-013, FR-020).
            if was_passthrough && !self.passthrough {
                let _ = crate::ui::passthrough::reset_on_exit();
                let _ = crate::ui::passthrough::set_stdin_nonblocking(false);
                terminal.clear()?;
            }
            prev_passthrough = self.passthrough;
        }
        // Restore blocking stdin if we exit while still in passthrough.
        let _ = crate::ui::passthrough::set_stdin_nonblocking(false);
        Ok(())
    }

    /// Drain any bytes the terminal has sent on stdin and forward them to the
    /// child verbatim, then service a pending resize. stdin is drained before
    /// `event::read` so terminal query/responses are never parsed into
    /// `KeyEvent`s; a wake with no stdin bytes is a resize signal (FR-012).
    fn forward_passthrough_input(&mut self, output_pending: bool) -> anyhow::Result<()> {
        self.drain_passthrough_stdin();

        // Block briefly (or not at all while output is flooding) so input and
        // resize stay responsive without busy-looping.
        let poll_timeout = if output_pending {
            Duration::ZERO
        } else {
            POLL_INTERVAL
        };
        if event::poll(poll_timeout)? {
            // The wake may be fresh stdin bytes or a resize signal. Drain stdin
            // first; if nothing was pending there, it was a resize.
            if self.drain_passthrough_stdin() == 0 {
                if let Event::Resize(cols, rows) = event::read()? {
                    let _ = self.shell.resize(rows, cols);
                }
            }
        }
        Ok(())
    }

    /// Read all currently-available stdin bytes and forward them verbatim to the
    /// child. Returns the total number of bytes forwarded.
    fn drain_passthrough_stdin(&mut self) -> usize {
        let mut total = 0usize;
        let mut buf = [0u8; 4096];
        loop {
            match crate::ui::passthrough::read_available_stdin(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let _ = self
                        .shell
                        .write_input(crate::ui::passthrough::forward_stdin(&buf[..n]));
                    total += n;
                    if n < buf.len() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        total
    }

    /// Drain pending PTY events without blocking, up to a bounded per-pass
    /// budget. Returns `true` if the budget was hit and more output may still
    /// be queued, so the caller can keep draining without blocking on input
    /// (FR-015; research R5).
    fn drain_shell(&mut self) -> bool {
        let mut drained_bytes = 0usize;
        let mut drained_chunks = 0usize;
        while let Ok(event) = self.shell.try_recv() {
            match event {
                PtyEvent::Output(bytes) => {
                    drained_bytes += bytes.len();
                    drained_chunks += 1;
                    let was_passthrough = self.passthrough;
                    let boundaries =
                        self.processor
                            .apply(&bytes, &mut self.transcript, &mut self.current_block);
                    for boundary in boundaries {
                        match boundary {
                            Boundary::CommandEnd { exit_code } => self.last_exit = exit_code,
                            // The shell reported a new working directory via
                            // OSC 7; follow it on the status rule (FR-019).
                            Boundary::Cwd(path) => self.cwd = path,
                            _ => {}
                        }
                    }
                    self.passthrough = self.processor.in_alt_screen();
                    // Pass raw bytes straight to the terminal whenever the
                    // program owns the screen, including the buffer that flips
                    // the alt-screen state in either direction (FR-018).
                    if was_passthrough || self.passthrough {
                        let _ = crate::ui::passthrough::write_output(&bytes);
                    }
                    // Yield back to the event loop once the per-pass budget is
                    // reached so key input is not starved during a flood.
                    if drained_bytes >= MAX_DRAIN_BYTES || drained_chunks >= MAX_DRAIN_CHUNKS {
                        return true;
                    }
                }
                // The wrapped shell exited: terminate cleanly (FR-027).
                PtyEvent::Exited(code) => {
                    if let Some(code) = code {
                        self.last_exit = Some(code);
                    }
                    self.should_quit = true;
                    return false;
                }
            }
        }
        false
    }

    fn on_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            // Forward Ctrl-C to the running command's process group via the PTY
            // line discipline; it interrupts the command, not kapollo (FR-024).
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                let _ = self.shell.send_interrupt();
            }
            // Shift+Enter / Alt+Enter insert a newline without submitting
            // (FR-010, FR-011). Shift requires the Kitty protocol; Alt is the
            // universal fallback (research R5).
            (KeyCode::Enter, m) if m.contains(KeyModifiers::SHIFT) => self.input.insert_newline(),
            (KeyCode::Enter, m) if m.contains(KeyModifiers::ALT) => self.input.insert_newline(),
            (KeyCode::Enter, _) => self.submit(),
            (KeyCode::Backspace, _) => self.input.backspace(),
            (KeyCode::Left, _) => self.input.move_left(),
            (KeyCode::Right, _) => self.input.move_right(),
            // Up/Down recall kapollo's own input history (FR-013).
            (KeyCode::Up, _) => {
                if let Some(text) = self.history.recall_older() {
                    let text = text.to_string();
                    self.input.set_contents(text);
                }
            }
            (KeyCode::Down, _) => {
                if let Some(text) = self.history.recall_newer() {
                    let text = text.to_string();
                    self.input.set_contents(text);
                }
            }
            // PageUp/PageDown scroll the transcript by a page; Home/End jump to
            // the oldest/newest output (FR-021).
            (KeyCode::PageUp, _) => self.transcript.page_up(),
            (KeyCode::PageDown, _) => self.transcript.page_down(),
            (KeyCode::Home, _) => self.transcript.scroll_to_top(),
            (KeyCode::End, _) => self.transcript.scroll_to_bottom(),
            (KeyCode::Char(c), _) => self.input.insert_char(c),
            _ => {}
        }
    }

    fn submit(&mut self) {
        let line = self.input.take_submit();
        self.history.push(line.clone());
        // A fresh submission scrolls the transcript back to the newest output.
        self.transcript.set_scroll_offset(0);

        match router::route(&line, self.config.leader_char) {
            Routed::Slash(command) => self.run_slash(&command),
            Routed::Shell(literal) => self.run_shell(literal),
        }
    }

    /// Dispatch a slash command. Slash commands act on kapollo state and never
    /// create a shell block, though `/help` and errors render as info blocks
    /// (contracts/slash-commands.md).
    fn run_slash(&mut self, command: &str) {
        match slash::dispatch(command) {
            Dispatch::Command(SlashCommand::Help) => {
                let text = builtins::help_text(self.config.leader_char);
                self.info_block(format!("{}help", self.config.leader_char), &text);
            }
            Dispatch::Command(SlashCommand::Clear) => self.transcript.clear(),
            // `/quit` triggers the same clean-teardown path as shell exit
            // (FR-025): the loop ends and the RAII terminal guard restores.
            Dispatch::Command(SlashCommand::Quit) => self.should_quit = true,
            Dispatch::Unknown(name) => {
                let text = builtins::unknown_text(&name, self.config.leader_char);
                self.info_block(format!("{}{}", self.config.leader_char, name), &text);
            }
        }
    }

    /// Send literal input to the wrapped shell, recording it as a block.
    fn run_shell(&mut self, line: String) {
        if line.is_empty() {
            // Advance the shell's prompt without creating a block.
            let _ = self.shell.write_input(b"\n");
            return;
        }

        let id = self.transcript.begin_block(line.clone());
        self.current_block = Some(id);
        self.processor.begin_command();
        let _ = self.shell.send_command(&line);
    }

    /// Append a closed, kapollo-generated block (e.g. `/help` output, errors)
    /// that is not associated with a shell command.
    fn info_block(&mut self, command: String, output: &str) {
        let id = self.transcript.begin_block(command);
        if let Some(block) = self.transcript.block_mut(id) {
            block.push_output(output.as_bytes());
        }
        self.transcript.close_block(id, Some(0));
    }
}
