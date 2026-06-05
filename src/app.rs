//! The application: top-level state and the single-threaded event loop. The
//! loop is the panic boundary and the only place that mutates UI state
//! (research R1). It interleaves keyboard/resize events with PTY output drained
//! from the reader thread's channel.

use std::time::Duration;

use anyhow::Context;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::backend::Backend;
use ratatui::Terminal;

use crate::config::Config;
use crate::grid::Grid;
use crate::input::router::{self, MouseRoute, Routed};
use crate::input::{InputHistory, InputPad};
use crate::output::{Boundary, OutputProcessor};
use crate::pty::{PtyEvent, PtySession};
use crate::selection::coords::{self, Cell};
use crate::selection::{SelectionController, Trigger};
use crate::session::{BlockId, BlockStore, Transcript};
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
    /// The emulated terminal screen + scrollback — the authoritative source the
    /// transcript pane renders from (D25/D27).
    pub grid: Grid,
    /// Mouse selection FSM, anchored to content rows so it never drifts as new
    /// output scrolls underneath it (FR-007/008, R6).
    pub selection: SelectionController,
    /// A transient one-line notice shown on the status rule (e.g. a copy
    /// failure), cleared on the next successful action (FR-013).
    pub notice: Option<String>,
    processor: OutputProcessor,
    current_block: Option<BlockId>,
    /// The canonical, retained block store — the source of truth for `/save`,
    /// `/filter`, and block-aware copy. Survives grid scrollback eviction (R3).
    pub store: BlockStore,
    /// The store block currently capturing output, paired with `current_block`.
    current_store_block: Option<BlockId>,
    /// Whether a full-screen program currently owns the screen; while set, keys
    /// are encoded and forwarded to the child instead of editing the input pad.
    passthrough: bool,
    should_quit: bool,
}

impl App {
    /// Construct the application, spawning the wrapped shell.
    pub fn new(config: Config) -> anyhow::Result<Self> {
        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
        // The wrapped shell and the emulated grid share the transcript pane's
        // dimensions: full width, with rows reduced by the status rule and the
        // (initially single-line) input pad. `run` keeps them in sync as the
        // terminal or input pad resizes (FR-004/006, SC-008).
        let cols = cols.max(1);
        let grid_rows = rows.saturating_sub(2).max(1);
        let shell = PtySession::spawn_with_size(config.shell.as_deref(), grid_rows, cols)
            .context("failed to start the wrapped shell")?;
        let processor = OutputProcessor::for_mode(shell.boundary_mode(), shell.nonce());
        let transcript = Transcript::new(config.caps.clone());
        let store = BlockStore::new(&config.caps);
        let scrollback = config.scroll.scrollback_lines.min(usize::MAX as u64) as usize;
        let grid = Grid::with_scrollback(grid_rows, cols, scrollback);
        Ok(Self {
            config,
            transcript,
            input: InputPad::new(),
            last_exit: None,
            cwd: std::env::current_dir().unwrap_or_default(),
            history: InputHistory::new(),
            shell,
            grid,
            selection: SelectionController::new(),
            notice: None,
            processor,
            current_block: None,
            store,
            current_store_block: None,
            passthrough: false,
            should_quit: false,
        })
    }

    /// Run the event loop until the user quits or the wrapped shell exits.
    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> anyhow::Result<()>
    where
        B::Error: std::error::Error + Send + Sync + 'static,
    {
        while !self.should_quit {
            // Keep the emulator and PTY sized to the transcript pane so the
            // child's view matches what kapollo renders (FR-004, SC-008).
            self.sync_size(terminal);

            terminal.draw(|frame| crate::ui::render(frame, self))?;

            let output_pending = self.drain_shell();

            // While output is still backing up, poll without blocking so a
            // pending key (e.g. Ctrl-C) is serviced before the next bounded
            // drain pass instead of waiting out the full interval (FR-015).
            let poll_timeout = if output_pending {
                Duration::ZERO
            } else {
                POLL_INTERVAL
            };
            if event::poll(poll_timeout)? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        // While a full-screen program owns the screen, keys are
                        // encoded and forwarded to the child; otherwise kapollo
                        // edits its input pad and drives scrollback (FR-018).
                        if self.passthrough {
                            self.forward_key(key);
                        } else {
                            self.on_key(key);
                        }
                    }
                    Event::Mouse(m) => self.on_mouse(m),
                    // The terminal resized; `sync_size` reflows the grid + PTY on
                    // the next iteration (FR-017, SC-008).
                    Event::Resize(_, _) => {}
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Drain pending PTY events without blocking, up to a bounded per-pass
    /// budget. Feeds raw bytes to the emulator (which owns the authoritative
    /// screen + scrollback) and to the boundary side-tap (OSC 133/7 + mode),
    /// which still captures block text for `/save` (R3/R7). Returns `true` if
    /// the budget was hit and more output may be queued (FR-015; research R5).
    fn drain_shell(&mut self) -> bool {
        let max_before = self.grid.max_scroll();
        let mut drained_bytes = 0usize;
        let mut drained_chunks = 0usize;
        while let Ok(event) = self.shell.try_recv() {
            match event {
                PtyEvent::Output(bytes) => {
                    drained_bytes += bytes.len();
                    drained_chunks += 1;
                    // Side-tap: detect command/cwd/mode boundaries and capture
                    // the block's output text for the store; the emulator, not
                    // this pass, applies SGR/cursor moves (R7).
                    let tx_block_before = self.current_block;
                    let boundaries =
                        self.processor
                            .apply(&bytes, &mut self.transcript, &mut self.current_block);
                    let mut output_started = false;
                    let mut command_ended: Option<Option<i32>> = None;
                    for boundary in boundaries {
                        match boundary {
                            Boundary::CommandEnd { exit_code } => {
                                self.last_exit = exit_code;
                                command_ended = Some(exit_code);
                            }
                            // The shell reported a new working directory via
                            // OSC 7; follow it on the status rule (FR-019).
                            Boundary::Cwd(path) => self.cwd = path,
                            // Output start (OSC 133 `C`) anchors the store
                            // block's first grid row and stamps `started_at`.
                            Boundary::OutputStart => output_started = true,
                            _ => {}
                        }
                    }
                    // Feed the emulator the raw bytes verbatim; it owns the
                    // escape parse, in-place CR updates, and alt-screen state.
                    self.grid.advance_bytes(&bytes);
                    // Anchor the store block's row range to the post-advance
                    // grid cursor and, on the end mark, copy the captured text
                    // into the canonical store and seal it (R3, R7).
                    self.update_store(output_started, command_ended, tx_block_before);
                    // Yield back to the event loop once the per-pass budget is
                    // reached so key input is not starved during a flood.
                    if drained_bytes >= MAX_DRAIN_BYTES || drained_chunks >= MAX_DRAIN_CHUNKS {
                        self.after_drain(max_before);
                        return true;
                    }
                }
                // The wrapped shell exited: terminate cleanly (FR-027).
                PtyEvent::Exited(code) => {
                    if let Some(code) = code {
                        self.last_exit = Some(code);
                    }
                    self.should_quit = true;
                    self.after_drain(max_before);
                    return false;
                }
            }
        }
        self.after_drain(max_before);
        false
    }

    /// `StableRowIndex` of the live cursor row — where freshly emitted output
    /// currently sits — used to anchor store block row ranges (R6).
    fn cursor_stable_row(&self) -> wezterm_term::StableRowIndex {
        self.grid.stable_row_at(0, self.grid.cursor().1)
    }

    /// Reflect a drain pass's boundary marks into the canonical store: anchor
    /// the running block's start row on output start, and on the end mark copy
    /// the captured text from the (now-closed) transcript block into the store
    /// and seal it with its exit code and final row (R3, R7).
    fn update_store(
        &mut self,
        output_started: bool,
        command_ended: Option<Option<i32>>,
        tx_block: Option<BlockId>,
    ) {
        let end_row = self.cursor_stable_row();
        if output_started {
            if let Some(sid) = self.current_store_block {
                self.store.set_start_row(sid, end_row);
            }
        }
        if let Some(exit_code) = command_ended {
            if let Some(sid) = self.current_store_block.take() {
                if let Some(text) = tx_block.and_then(|id| self.transcript.block(id)) {
                    let captured = text.output.to_vec();
                    self.store.push_output(sid, &captured);
                }
                self.store.seal(sid, exit_code, end_row);
            }
        }
    }

    /// After a drain pass: forward any emulator answerback to the PTY, refresh
    /// the alt-screen routing flag from the authoritative emulator state, and
    /// keep a scrolled-back viewport anchored as new history arrives
    /// (follow-tail; FR-022, SC-006).
    fn after_drain(&mut self, max_before: usize) {
        let reply = self.grid.drain_answerback();
        if !reply.is_empty() {
            let _ = self.shell.write_input(&reply);
        }
        self.passthrough = self.grid.is_alt_screen_active();

        // When scrolled back, grow the offset by the rows that fell into history
        // this pass so the visible window stays put; at the live bottom (offset
        // 0) we keep following the newest output (FR-022).
        let offset = self.transcript.scroll_offset();
        if offset > 0 {
            let grew = self.grid.max_scroll().saturating_sub(max_before);
            if grew > 0 {
                let clamped = (offset + grew).min(self.grid.max_scroll());
                self.transcript.set_scroll_offset(clamped);
            }
        }
    }

    /// Keep the emulator and PTY sized to the transcript pane: full width, with
    /// rows reduced by the status rule and the input pad. A full-screen program
    /// (alt-screen) gets the whole area. Resizes only on an actual dimension
    /// change to avoid spurious SIGWINCH churn (FR-004/006, SC-008).
    fn sync_size<B: Backend>(&mut self, terminal: &Terminal<B>)
    where
        B::Error: std::error::Error + Send + Sync + 'static,
    {
        if let Ok(size) = terminal.size() {
            let chrome = if self.passthrough {
                0
            } else {
                1 + crate::ui::input_pad_height(&self.input)
            };
            let cols = size.width.max(1);
            let rows = size.height.saturating_sub(chrome).max(1);
            if self.grid.size() != (rows, cols) {
                self.grid.resize(rows, cols);
                let _ = self.shell.resize(rows, cols);
            }
        }
    }

    /// Encode a key event into the byte sequence a terminal emits and forward it
    /// to the child while a full-screen program owns the screen (FR-018).
    fn forward_key(&mut self, key: KeyEvent) {
        if let Some(bytes) = encode_key(key) {
            let _ = self.shell.write_input(&bytes);
        }
    }

    /// Route a mouse event: shift bypasses to the host terminal's native
    /// selection, a full-screen / mouse-grabbing child receives the encoded
    /// event, and otherwise kapollo consumes it for selection + scrollback
    /// (FR-009, D28).
    fn on_mouse(&mut self, m: MouseEvent) {
        let shift = m.modifiers.contains(KeyModifiers::SHIFT);
        let alt = self.grid.is_alt_screen_active();
        let child_mouse = self.grid.is_mouse_grabbed();
        match router::route_mouse(shift, alt, child_mouse) {
            // Let the host terminal handle native selection; with shift held,
            // most terminals override kapollo's mouse capture themselves.
            MouseRoute::Bypass => {}
            MouseRoute::ToChild => {
                if let Some(bytes) = encode_mouse(m) {
                    let _ = self.shell.write_input(&bytes);
                }
            }
            MouseRoute::Consumed => self.on_mouse_consumed(m),
        }
    }

    /// Drive the selection FSM and wheel scrollback from a consumed mouse event.
    fn on_mouse_consumed(&mut self, m: MouseEvent) {
        let (rows, _cols) = self.grid.size();
        let height = rows as usize;
        let max = self.grid.max_scroll();
        let wheel = (self.config.scroll.wheel_lines as usize).max(1);
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let cell = self.cell_at(m.row, m.column);
                let _ = self.selection.left_press(cell, false);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // Auto-scroll while dragging past the top/bottom edge so the
                // selection can extend beyond the visible viewport (FR-009).
                let off = self.transcript.scroll_offset();
                if m.row == 0 {
                    self.scroll_to(off.saturating_add(1));
                } else if m.row as usize >= height.saturating_sub(1) {
                    self.scroll_to(off.saturating_sub(1));
                }
                let y = (m.row as usize).min(height.saturating_sub(1)) as u16;
                let cell = self.cell_at(y, m.column);
                self.selection.drag_to(cell);
            }
            MouseEventKind::Up(MouseButton::Left) => self.selection.release(),
            MouseEventKind::Down(MouseButton::Right) => {
                let cell = self.cell_at(m.row, m.column);
                match self.selection.right_press() {
                    // With an active selection, right-click copies it (FR-011).
                    Trigger::Copy(a, b) => self.copy_selection(a, b),
                    // With no selection, the primary block-aware affordance is
                    // "copy the block under the cursor, with its command line"
                    // (FR-024, US3-5). The full with/without/current-line menu
                    // arrives with the popup UI; the other variants are exposed
                    // as `copy_block_without_command` / `copy_current_line`.
                    Trigger::ContextMenu => {
                        self.copy_block_with_command(cell.0 as wezterm_term::StableRowIndex)
                    }
                    Trigger::Sigint => {}
                }
            }
            MouseEventKind::ScrollUp => {
                self.scroll_to(self.transcript.scroll_offset().saturating_add(wheel))
            }
            MouseEventKind::ScrollDown => {
                self.scroll_to(self.transcript.scroll_offset().saturating_sub(wheel))
            }
            _ => {}
        }
        // Keep the wheel from scrolling past the oldest scrollback line.
        let _ = max;
    }

    /// Translate a screen cell (row/column within the transcript pane) to a
    /// content cell anchored to the absolute (stable) row, so a selection stays
    /// put as new output scrolls underneath it (FR-007, R6).
    fn cell_at(&self, screen_row: u16, screen_col: u16) -> Cell {
        let off = self.transcript.scroll_offset().min(self.grid.max_scroll());
        let top = self.grid.top_stable_row(off).max(0) as usize;
        (
            coords::screen_to_content(top, screen_row as usize),
            screen_col as usize,
        )
    }

    /// Set the transcript scroll offset, clamped to the available scrollback.
    fn scroll_to(&mut self, offset: usize) {
        let clamped = offset.min(self.grid.max_scroll());
        self.transcript.set_scroll_offset(clamped);
    }

    /// Copy the content covered by `a..=b` to the clipboard, preferring OSC 52
    /// to the host terminal and falling back to the local clipboard per config
    /// (FR-012, FR-013). A failure surfaces as a status-rule notice.
    fn copy_selection(&mut self, a: Cell, b: Cell) {
        let off = self.transcript.scroll_offset().min(self.grid.max_scroll());
        let top = self.grid.top_stable_row(off).max(0) as usize;
        let cells = self.grid.viewport_cells(off);
        let text = crate::selection::extract_text(&cells, top, a, b);
        if self.deliver_copy(&text) {
            self.notice = None;
        }
    }

    /// Copy `text` via OSC 52 (terminal-mediated, works over SSH) with the
    /// configured local fallback. Returns whether the copy succeeded; on
    /// failure a user-visible notice is set (FR-013). Empty text is a no-op.
    fn deliver_copy(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        match crate::clipboard::copy(
            text,
            self.config.clipboard.osc52,
            self.config.clipboard.local_fallback,
        ) {
            Ok(crate::clipboard::CopyMethod::Osc52(frame)) => {
                use std::io::Write;
                let mut out = std::io::stdout();
                if out
                    .write_all(frame.as_bytes())
                    .and_then(|()| out.flush())
                    .is_err()
                {
                    self.notice = Some("copy failed: could not write to terminal".into());
                    false
                } else {
                    true
                }
            }
            Ok(crate::clipboard::CopyMethod::Local) => true,
            Err(_) => {
                self.notice = Some("copy failed: no clipboard available".into());
                false
            }
        }
    }

    /// Copy the output of the block covering stable `row`, including its command
    /// line, via the canonical store accessor (FR-024, US3-5). An evicted/unknown
    /// row yields an explicit "unavailable" notice (FR-025).
    pub fn copy_block_with_command(&mut self, row: wezterm_term::StableRowIndex) {
        match self
            .store
            .block_at_row(row)
            .and_then(|id| self.store.text_with_command(id))
        {
            Some(text) if self.deliver_copy(&text) => {
                self.notice = Some("copied block (with command)".into());
            }
            Some(_) => {}
            None => self.notice = Some("copy failed: block unavailable".into()),
        }
    }

    /// Copy the output of the block covering stable `row`, excluding its command
    /// line (FR-024, US3-5).
    pub fn copy_block_without_command(&mut self, row: wezterm_term::StableRowIndex) {
        match self
            .store
            .block_at_row(row)
            .and_then(|id| self.store.text(id))
        {
            Some(text) if self.deliver_copy(&text) => {
                self.notice = Some("copied block output".into());
            }
            Some(_) => {}
            None => self.notice = Some("copy failed: block unavailable".into()),
        }
    }

    /// Copy the single transcript line at viewport `screen_row` (FR-024, US3-5).
    pub fn copy_current_line(&mut self, screen_row: u16) {
        let off = self.transcript.scroll_offset().min(self.grid.max_scroll());
        let cells = self.grid.viewport_cells(off);
        if let Some(row) = cells.get(screen_row as usize) {
            let line = row.concat();
            let line = line.trim_end().to_string();
            if self.deliver_copy(&line) {
                self.notice = Some("copied line".into());
            }
        }
    }

    fn on_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            // Ctrl-C copies an active selection; otherwise it interrupts the
            // running command via the PTY line discipline (FR-013, FR-024).
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => match self.selection.ctrl_c() {
                Trigger::Copy(a, b) => self.copy_selection(a, b),
                Trigger::Sigint => {
                    let _ = self.shell.send_interrupt();
                }
                Trigger::ContextMenu => {}
            },
            // Esc clears any active selection (FR-008).
            (KeyCode::Esc, _) => {
                self.selection.cancel();
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
        // A fresh submission scrolls the transcript back to the newest output
        // and clears any lingering selection (FR-008, FR-021).
        self.transcript.set_scroll_offset(0);
        self.selection.on_command_submit();

        match router::route(&line, self.config.leader_char) {
            Routed::Slash(command) => self.run_slash(&command),
            Routed::Shell(literal) => self.run_shell(literal),
        }
    }

    /// Dispatch a slash command. Slash commands act on kapollo state; `/help`
    /// and errors render as synthetic blocks injected into the grid so they
    /// scroll, select, and copy exactly like real command output (D25; see
    /// contracts/slash-commands.md).
    fn run_slash(&mut self, command: &str) {
        match slash::dispatch(command) {
            Dispatch::Command(SlashCommand::Help) => {
                let text = builtins::help_text(self.config.leader_char);
                self.synthetic_block(format!("{}help", self.config.leader_char), &text);
            }
            Dispatch::Command(SlashCommand::Clear) => {
                // Clear the visible grid (screen + scrollback) and the block
                // model together so the pane and the transcript stay in sync
                // (FR-023). `transcript.clear` also resets the scroll offset.
                self.grid.clear();
                self.transcript.clear();
            }
            // `/quit` triggers the same clean-teardown path as shell exit
            // (FR-025): the loop ends and the RAII terminal guard restores.
            Dispatch::Command(SlashCommand::Quit) => self.should_quit = true,
            Dispatch::Unknown(name) => {
                let text = builtins::unknown_text(&name, self.config.leader_char);
                self.synthetic_block(format!("{}{}", self.config.leader_char, name), &text);
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
        // Mirror the boundary in the canonical store (OSC 133 `B`); its row
        // range is anchored as output arrives (R3, R7).
        self.current_store_block = Some(self.store.begin(line.clone(), Some(self.cwd.clone())));
        self.processor.begin_command();
        let _ = self.shell.send_command(&line);
    }

    /// Render a kapollo-generated block (e.g. `/help`, errors) by injecting it
    /// into the emulator grid so it appears inline with shell output — and thus
    /// scrolls, selects, and copies identically (D25, option 2). It is also
    /// recorded as a closed, `synthetic` block so later features (`/save`,
    /// `/filter`) can distinguish it from a typed command.
    fn synthetic_block(&mut self, command: String, output: &str) {
        // 1. Paint it into the grid. The prompt echo wears kapollo's prompt
        //    glyph so it reads like a command the user ran.
        let max_before = self.grid.max_scroll();
        let bytes = self.render_synthetic(&command, output);
        self.grid.advance_bytes(&bytes);
        self.after_drain(max_before);

        // 2. Record the block (marked synthetic) for the transcript model.
        let id = self.transcript.begin_block(command);
        if let Some(block) = self.transcript.block_mut(id) {
            block.synthetic = true;
            block.push_output(output.as_bytes());
        }
        self.transcript.close_block(id, Some(0));
    }

    /// Build the raw terminal bytes for a synthetic block: a fresh line, the
    /// prompt-glyph command echo, then the output. Lines use `\r\n` so the
    /// emulator returns the carriage as well as advancing (FR-002).
    fn render_synthetic(&self, command: &str, output: &str) -> Vec<u8> {
        let (cx, _) = self.grid.cursor();
        synthetic_bytes(
            self.config.prompt_char,
            self.config.prompt_color,
            crate::ui::color_enabled(),
            cx == 0,
            command,
            output,
        )
    }
}

/// Build the raw terminal bytes for a synthetic block (pure; see
/// [`App::render_synthetic`]). `at_col0` skips the leading newline when the
/// cursor already sits at column 0; `color` gates the prompt-glyph styling.
fn synthetic_bytes(
    prompt_char: char,
    prompt_color: ratatui::style::Color,
    color: bool,
    at_col0: bool,
    command: &str,
    output: &str,
) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    // Start on a fresh row unless the cursor already sits at column 0.
    if !at_col0 {
        buf.extend_from_slice(b"\r\n");
    }
    // Prompt glyph (colored like kapollo's prompt) + the command echo.
    if color {
        buf.extend_from_slice(color_to_sgr(prompt_color).as_bytes());
    }
    let mut echo = String::new();
    echo.push(prompt_char);
    buf.extend_from_slice(echo.as_bytes());
    if color {
        buf.extend_from_slice(b"\x1b[0m");
    }
    buf.extend_from_slice(format!(" {command}\r\n").as_bytes());
    // Output body, normalized to a single trailing newline.
    for line in output.trim_end_matches('\n').split('\n') {
        buf.extend_from_slice(line.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }
    buf
}

/// Encode a crossterm key event into the byte sequence a terminal emits, for
/// forwarding to a full-screen child program while it owns the screen (FR-018).
/// Returns `None` for keys with no terminal encoding.
fn encode_key(key: KeyEvent) -> Option<Vec<u8>> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char(c) => {
            if ctrl && c.is_ascii_alphabetic() {
                // Control codes: Ctrl-A..Ctrl-Z map to 0x01..0x1a.
                Some(vec![(c.to_ascii_uppercase() as u8) & 0x1f])
            } else {
                Some(c.to_string().into_bytes())
            }
        }
        KeyCode::Enter => Some(vec![b'\r']),
        KeyCode::Tab => Some(vec![b'\t']),
        KeyCode::Backspace => Some(vec![0x7f]),
        KeyCode::Esc => Some(vec![0x1b]),
        KeyCode::Left => Some(b"\x1b[D".to_vec()),
        KeyCode::Right => Some(b"\x1b[C".to_vec()),
        KeyCode::Up => Some(b"\x1b[A".to_vec()),
        KeyCode::Down => Some(b"\x1b[B".to_vec()),
        KeyCode::Home => Some(b"\x1b[H".to_vec()),
        KeyCode::End => Some(b"\x1b[F".to_vec()),
        KeyCode::PageUp => Some(b"\x1b[5~".to_vec()),
        KeyCode::PageDown => Some(b"\x1b[6~".to_vec()),
        KeyCode::Delete => Some(b"\x1b[3~".to_vec()),
        _ => None,
    }
}

/// Encode a crossterm mouse event as an SGR (1006) mouse report for forwarding
/// to a full-screen / mouse-grabbing child program (FR-018, D28). Returns
/// `None` for events with no SGR encoding.
fn encode_mouse(m: MouseEvent) -> Option<Vec<u8>> {
    let col = m.column as u32 + 1;
    let row = m.row as u32 + 1;
    let (btn, kind) = match m.kind {
        MouseEventKind::Down(b) => (mouse_btn(b), 'M'),
        MouseEventKind::Up(b) => (mouse_btn(b), 'm'),
        MouseEventKind::Drag(b) => (mouse_btn(b) + 32, 'M'),
        MouseEventKind::ScrollUp => (64, 'M'),
        MouseEventKind::ScrollDown => (65, 'M'),
        _ => return None,
    };
    Some(format!("\x1b[<{btn};{col};{row}{kind}").into_bytes())
}

/// Map a crossterm mouse button to its SGR button number.
fn mouse_btn(b: MouseButton) -> u32 {
    match b {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    }
}

/// Build the SGR foreground-color escape for a ratatui color, for styling
/// synthetic block output injected into the grid. Returns an empty string for
/// `Reset`/default (no color applied).
fn color_to_sgr(color: ratatui::style::Color) -> String {
    use ratatui::style::Color;
    let code = match color {
        Color::Reset => return String::new(),
        Color::Black => "30".to_string(),
        Color::Red => "31".to_string(),
        Color::Green => "32".to_string(),
        Color::Yellow => "33".to_string(),
        Color::Blue => "34".to_string(),
        Color::Magenta => "35".to_string(),
        Color::Cyan => "36".to_string(),
        Color::Gray => "37".to_string(),
        Color::DarkGray => "90".to_string(),
        Color::LightRed => "91".to_string(),
        Color::LightGreen => "92".to_string(),
        Color::LightYellow => "93".to_string(),
        Color::LightBlue => "94".to_string(),
        Color::LightMagenta => "95".to_string(),
        Color::LightCyan => "96".to_string(),
        Color::White => "97".to_string(),
        Color::Indexed(n) => format!("38;5;{n}"),
        Color::Rgb(r, g, b) => format!("38;2;{r};{g};{b}"),
    };
    format!("\x1b[{code}m")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn color_to_sgr_maps_named_and_special() {
        assert_eq!(color_to_sgr(Color::Red), "\x1b[31m");
        assert_eq!(color_to_sgr(Color::LightBlue), "\x1b[94m");
        assert_eq!(color_to_sgr(Color::Indexed(200)), "\x1b[38;5;200m");
        assert_eq!(color_to_sgr(Color::Rgb(1, 2, 3)), "\x1b[38;2;1;2;3m");
        // Reset/default applies no color.
        assert_eq!(color_to_sgr(Color::Reset), "");
    }

    #[test]
    fn synthetic_bytes_echoes_prompt_and_output_with_crlf() {
        let bytes = synthetic_bytes('λ', Color::Red, false, true, "/help", "line1\nline2");
        let s = String::from_utf8(bytes).unwrap();
        // No color escapes when color is off, fresh line skipped at column 0,
        // CRLF line endings, single trailing newline.
        assert_eq!(s, "λ /help\r\nline1\r\nline2\r\n");
    }

    #[test]
    fn synthetic_bytes_prepends_newline_when_not_at_col0() {
        let bytes = synthetic_bytes('λ', Color::Red, false, false, "/x", "out");
        let s = String::from_utf8(bytes).unwrap();
        assert_eq!(s, "\r\nλ /x\r\nout\r\n");
    }

    #[test]
    fn synthetic_bytes_styles_prompt_when_color_enabled() {
        let bytes = synthetic_bytes('λ', Color::Red, true, true, "/help", "out");
        let s = String::from_utf8(bytes).unwrap();
        // Prompt glyph wrapped in its color, reset before the command echo.
        assert_eq!(s, "\x1b[31mλ\x1b[0m /help\r\nout\r\n");
    }

    #[test]
    fn synthetic_bytes_normalizes_trailing_newlines() {
        let bytes = synthetic_bytes('λ', Color::Red, false, true, "/help", "out\n\n");
        let s = String::from_utf8(bytes).unwrap();
        assert_eq!(s, "λ /help\r\nout\r\n");
    }
}
