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
use crate::input::{InputHistory, InputMode, InputPad, LaatState};
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
    /// The resolved path the config was loaded from, retained so `/reload-config`
    /// can re-read it (research R6). `None` when no config file location resolved.
    config_path: Option<std::path::PathBuf>,
    pub transcript: Transcript,
    pub input: InputPad,
    pub last_exit: Option<i32>,
    /// The current input editing mode, surfaced in the status bar's mode field
    /// (sprint 007). `Norm` recalls history on `Up`/`Down`; `Mult`/`Laat` move
    /// the caret with chat-style edge recall.
    pub mode: InputMode,
    /// LAAT stepping state, present only while `mode == Laat` (sprint 007): the
    /// highlight, probable-failure flags, and the line awaiting completion.
    pub laat: Option<LaatState>,
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
    /// True when the immediately preceding key was `Esc`, so the next `Esc`
    /// completes the `Esc Esc` gesture. Reset by any non-Esc key — a keypress
    /// flag, never a wall clock (FR-026/FR-029, research R6).
    esc_pending: bool,
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
    /// A pending `/save` overwrite prompt (sprint 007, FR-023): while set,
    /// `on_key` consumes the next key to resolve overwrite/append/cancel.
    pending_prompt: Option<PendingPrompt>,
    /// True while a `/filter` shell round-trip is in flight, so its completion
    /// can surface a `filter non-zero exit` status message (sprint 007, FR-027).
    filter_active: bool,
    /// The one-item input push/pop stack (sprint 007, FR-018…FR-020): a pushed
    /// snapshot is restored on the next submit. `None` means the slot is empty.
    pushed: Option<crate::input::InputSnapshot>,
    should_quit: bool,
}

/// A deferred `/save` to an existing file, awaiting the user's
/// overwrite/append/cancel choice (sprint 007, FR-023). Holds the resolved
/// target path and the exact bytes to write.
struct PendingPrompt {
    path: std::path::PathBuf,
    bytes: Vec<u8>,
}

impl App {
    /// Construct the application, spawning the wrapped shell.
    pub fn new(config: Config, config_path: Option<std::path::PathBuf>) -> anyhow::Result<Self> {
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
            config_path,
            transcript,
            input: InputPad::new(),
            last_exit: None,
            mode: InputMode::Norm,
            laat: None,
            cwd: std::env::current_dir().unwrap_or_default(),
            history: InputHistory::new(),
            shell,
            grid,
            selection: SelectionController::new(),
            notice: None,
            esc_pending: false,
            processor,
            current_block: None,
            store,
            current_store_block: None,
            passthrough: false,
            pending_prompt: None,
            filter_active: false,
            pushed: None,
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
                    // Bracketed paste arrives as one unit: insert it into the
                    // input pad without submitting (FR-010/011/012). While a
                    // full-screen child owns the screen, forward it instead.
                    Event::Paste(text) => {
                        if self.passthrough {
                            let _ = self.shell.write_input(text.as_bytes());
                        } else {
                            self.input.insert_paste(&text);
                        }
                    }
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
                                // LAAT stepping: gate the highlight on the exit
                                // code of the line just submitted (FR-004).
                                self.apply_laat_gating(exit_code);
                                // A completed `/filter` round-trip surfaces a
                                // non-zero exit as a status message (FR-027).
                                if self.filter_active {
                                    self.filter_active = false;
                                    if exit_code.is_some_and(|c| c != 0) {
                                        self.notice = Some("filter non-zero exit".into());
                                    }
                                }
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
                let status_rows = u16::from(crate::ui::status::is_visible(
                    self.config.status.enabled,
                    size.height,
                ));
                let divider_rows = u16::from(self.config.divider.enabled);
                crate::ui::input_pad_height(&self.input) + status_rows + divider_rows
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
                // Starting a transcript selection clears any input-pad selection
                // so at most one pad is ever selected (FR-027).
                self.input.cancel_selection();
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
        use crate::action::{KeyChord, KeySpec};
        // A pending `/save` overwrite prompt consumes the next key first
        // (FR-023): O/A/C(/Esc) resolve it; any other key keeps it up.
        if self.pending_prompt.is_some() {
            self.resolve_save_prompt(key.code);
            return;
        }
        // `Esc Esc` is a contextual two-key gesture tracked by a keypress flag,
        // not a timer (FR-026/FR-029): any non-Esc key resets it.
        let was_esc_pending = self.esc_pending;
        if !matches!(key.code, KeyCode::Esc) {
            self.esc_pending = false;
        }
        match (key.code, key.modifiers) {
            // Ctrl-C copies an active selection — input pad first, then the
            // transcript — and otherwise interrupts the running command via the
            // PTY line discipline (FR-024, FR-028). Copy never shadows interrupt.
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                if self.input.has_selection() {
                    if let Some(text) = self.input.selected_text() {
                        if self.deliver_copy(&text) {
                            self.notice = Some("copied selection".into());
                        }
                    }
                    self.input.cancel_selection();
                } else {
                    match self.selection.ctrl_c() {
                        Trigger::Copy(a, b) => self.copy_selection(a, b),
                        Trigger::Sigint => {
                            let _ = self.shell.send_interrupt();
                        }
                        Trigger::ContextMenu => {}
                    }
                }
            }
            // Esc is contextual (FR-029): cancel a selection, else clear the
            // current line; `Esc Esc` clears a multi-line buffer and the status
            // message (FR-026).
            (KeyCode::Esc, _) => self.on_esc(was_esc_pending),
            // Plain Enter submits the line; Shift+Enter / Alt+Enter fall through
            // to the keymap and resolve to `InsertNewline` (FR-004/FR-018,
            // research R5). Shift requires the Kitty protocol; Alt is the
            // universal fallback.
            (KeyCode::Enter, m) if !m.intersects(KeyModifiers::SHIFT | KeyModifiers::ALT) => {
                self.submit()
            }
            (KeyCode::Backspace, _) => {
                self.input.backspace();
                self.reconcile_mode_after_edit();
            }
            // Plain Left/Right move the cursor; Shift/Ctrl variants resolve to
            // named selection/word-motion actions below (FR-002/003/004).
            (KeyCode::Left, KeyModifiers::NONE) => self.input.move_left(),
            (KeyCode::Right, KeyModifiers::NONE) => self.input.move_right(),
            // Up/Down are mode-aware (sprint 007): Norm recalls history; Mult and
            // Laat move the caret between lines (edge recall added in US2).
            (KeyCode::Up, _) => self.on_up(),
            (KeyCode::Down, _) => self.on_down(),
            // Everything else flows through the configurable keymap (FR-001):
            // Home/End line motion, Ctrl+arrow word motion, Shift selections,
            // Ctrl+U/K/W kills, the page/line scroll bindings, newline insertion,
            // and the keyboard copy variants. Only the default mode is populated
            // this sprint (a real mode selector lands in a later sprint).
            (code, mods) => {
                let chord = KeyChord::new(code, mods);
                if let Some(action) = self
                    .config
                    .keymaps
                    .default()
                    .resolve(KeySpec::Single(chord))
                {
                    self.dispatch_action(action);
                } else if let KeyCode::Char(c) = code {
                    if !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
                        self.input.insert_char(c);
                    }
                }
            }
        }
    }

    /// Execute a resolved named [`crate::action::Action`] (FR-008). Input-line
    /// actions edit the pad; scroll actions drive the transcript. The
    /// named-but-unmapped reserved motions and the contextual
    /// `clear_status_message` gesture never arrive here via `resolve`.
    fn dispatch_action(&mut self, action: crate::action::Action) {
        use crate::action::Action;
        match action {
            Action::LineMoveStart => self.input.line_move_start(),
            Action::LineMoveEnd => self.input.line_move_end(),
            Action::WordMoveLeft => self.input.word_move_left(),
            Action::WordMoveRight => self.input.word_move_right(),
            // Starting an input-pad selection clears any transcript selection so
            // at most one pad is ever selected (FR-027).
            Action::SelectCharLeft => {
                self.selection.cancel();
                self.input.select_char_left();
            }
            Action::SelectCharRight => {
                self.selection.cancel();
                self.input.select_char_right();
            }
            Action::SelectWordLeft => {
                self.selection.cancel();
                self.input.select_word_left();
            }
            Action::SelectWordRight => {
                self.selection.cancel();
                self.input.select_word_right();
            }
            Action::KillToLineStart => {
                self.input.kill_to_line_start();
                self.reconcile_mode_after_edit();
            }
            Action::KillToLineEnd => {
                self.input.kill_to_line_end();
                self.reconcile_mode_after_edit();
            }
            Action::DeleteWordBefore => {
                self.input.delete_word_before();
                self.reconcile_mode_after_edit();
            }
            // Insert a newline into the input buffer without submitting (FR-004).
            // Growing a `Norm` buffer to a second line auto-enters `Mult` (FR-008).
            Action::InsertNewline => {
                self.input.insert_newline();
                self.reconcile_mode_after_edit();
            }
            // Scrollback bindings (FR-013/015/016): page scroll preserves
            // `context_lines` of overlap, line scroll moves one line, and the
            // top/bottom jumps are reachable via Shift+Home/End.
            Action::ScrollPageUp => self
                .transcript
                .scroll_page_up(self.config.scroll.context_lines),
            Action::ScrollPageDown => self
                .transcript
                .scroll_page_down(self.config.scroll.context_lines),
            Action::ScrollLineUp => self.transcript.scroll_line_up(),
            Action::ScrollLineDown => self.transcript.scroll_line_down(),
            Action::ScrollToTop => self.transcript.scroll_to_top(),
            Action::ScrollToBottom => self.transcript.scroll_to_bottom(),
            // Keyboard copy variants (FR-005): with no mouse position, both act
            // on the bottom-most transcript output (research R4) — the newest
            // visible line, and the most recently completed block's output.
            Action::CopyCurrentLine => {
                let target = {
                    let off = self.transcript.scroll_offset().min(self.grid.max_scroll());
                    let cells = self.grid.viewport_cells(off);
                    cells
                        .iter()
                        .rposition(|row| !row.concat().trim_end().is_empty())
                };
                match target {
                    Some(idx) => self.copy_current_line(idx as u16),
                    None => self.notice = Some("copy failed: no visible line".into()),
                }
            }
            Action::CopyBlockWithoutCommand => {
                let target = self
                    .store
                    .iter()
                    .filter(|b| !b.row_range.is_empty())
                    .last()
                    .map(|b| b.row_range.end - 1);
                match target {
                    Some(row) => self.copy_block_without_command(row),
                    None => self.notice = Some("copy failed: no block output".into()),
                }
            }
            Action::ClearStatusMessage
            | Action::MultilineMoveStartBuffer
            | Action::MultilineMoveEndBuffer => {}
            // Toggle Mult/LAAT (sprint 007, FR-015/FR-016).
            Action::ToggleMultLaat => self.toggle_mult_laat(),
            // Push the input buffer for an ad-hoc command (sprint 007, FR-018).
            Action::PushInput => self.push_input(),
        }
    }

    /// Handle an `Esc` press (FR-029). The first `Esc` cancels an active
    /// selection (either pad) or clears the caret's current line; a second
    /// consecutive `Esc` clears a multi-line buffer and the status message
    /// (FR-026). `was_pending` is whether the immediately preceding key was also
    /// `Esc` — the double-Esc gesture is a keypress flag, not a timer.
    fn on_esc(&mut self, was_pending: bool) {
        use crate::input::selection::{esc_action, EscAction};
        // The double-Esc gesture also clears the status message, independent of
        // the buffer effect (FR-026).
        if was_pending {
            self.notice = None;
        }
        let has_selection = self.selection.range().is_some() || self.input.has_selection();
        let multiline = self.input.line_count() > 1;
        match esc_action(was_pending, has_selection, multiline) {
            EscAction::CancelSelection => {
                self.selection.cancel();
                self.input.cancel_selection();
            }
            EscAction::ClearCurrentLine => self.input.clear_current_line(),
            EscAction::ClearWholeBuffer => {
                self.input.clear();
                // Leaving `Mult`/`Laat` via `Esc Esc` returns to `Norm` and, for
                // `Laat`, discards the stepping state with the buffer (FR-007/FR-014).
                if self.mode != InputMode::Norm {
                    self.set_mode(InputMode::Norm);
                }
            }
            EscAction::None => {}
        }
        // Toggle the pending flag: this `Esc` arms the gesture; the next `Esc`
        // completes it.
        self.esc_pending = !was_pending;
    }

    /// Mode-aware `Up` (sprint 007): in `Norm`, recall the previous history
    /// entry; in `Mult`/`Laat`, move the caret up one line, or — when already on
    /// the first line — perform chat-style edge recall (stash the draft + recall
    /// the previous entry, FR-010; [contracts/input-modes.md] §2/§3).
    fn on_up(&mut self) {
        match self.mode {
            InputMode::Norm => {
                if let Some(text) = self.history.recall_older() {
                    let text = text.to_string();
                    self.input.set_contents(text);
                }
            }
            InputMode::Mult | InputMode::Laat => {
                if self.input.caret_on_first_line() {
                    let draft = self.input.as_str().to_string();
                    if let Some(text) = self.history.edge_recall_older(&draft) {
                        let text = text.to_string();
                        self.input.set_contents(text);
                    }
                } else {
                    self.input.caret_line_up();
                }
                self.sync_laat_highlight();
            }
        }
    }

    /// Mode-aware `Down` (sprint 007): in `Norm`, recall the next history entry;
    /// in `Mult`/`Laat`, move the caret down one line, or — when on the last line
    /// while recalling — restore the stashed draft (FR-011;
    /// [contracts/input-modes.md] §2/§3). `Down` never recalls older entries.
    fn on_down(&mut self) {
        match self.mode {
            InputMode::Norm => {
                if let Some(text) = self.history.recall_newer() {
                    let text = text.to_string();
                    self.input.set_contents(text);
                }
            }
            InputMode::Mult | InputMode::Laat => {
                if self.input.caret_on_last_line() {
                    if let Some(text) = self.history.edge_recall_newer() {
                        self.input.set_contents(text);
                    }
                } else {
                    self.input.caret_line_down();
                }
                self.sync_laat_highlight();
            }
        }
    }

    /// In `Laat`, keep the highlight on the caret's line (FR-002/FR-006).
    fn sync_laat_highlight(&mut self) {
        let row = self.input.cursor_row_col().0;
        if let Some(laat) = self.laat.as_mut() {
            laat.highlight = row;
        }
    }

    /// Apply LAAT exit-code gating when a submitted line completes (FR-004): on
    /// success the highlight advances and the caret follows to the next line; a
    /// non-zero exit flags the line and holds. A no-op when nothing is pending.
    fn apply_laat_gating(&mut self, exit_code: Option<i32>) {
        let outcome = self
            .laat
            .as_mut()
            .and_then(|laat| laat.apply_exit_code(exit_code));
        if matches!(outcome, Some(crate::input::LaatOutcome::Advance)) {
            let row = self.laat.as_ref().map_or(0, |laat| laat.highlight);
            self.input.set_caret_line_start(row);
        }
    }

    /// Toggle the input mode via `Ctrl+1` (sprint 007, FR-015/FR-016):
    /// `Norm → Mult` (even empty/single line), and `Mult ↔ Laat` only when the
    /// buffer is multi-line.
    fn toggle_mult_laat(&mut self) {
        let multiline = self.input.line_count() > 1;
        let new_mode = self.mode.toggled_mult_laat(multiline);
        self.set_mode(new_mode);
    }

    /// Switch the input mode, managing LAAT stepping state: entering `Laat`
    /// starts a fresh highlight on line 0; leaving `Laat` discards the stepping
    /// state (the buffer itself is cleared only on `Esc Esc`/submit/push, FR-007).
    fn set_mode(&mut self, new_mode: InputMode) {
        match new_mode {
            InputMode::Laat if self.mode != InputMode::Laat => {
                self.laat = Some(LaatState::new());
            }
            InputMode::Laat => {}
            _ => self.laat = None,
        }
        self.mode = new_mode;
    }

    /// Reconcile the mode with the buffer's line count after an edit (sprint
    /// 007): a `Norm` buffer that grows past one line enters `Mult` (FR-008); a
    /// `Mult` buffer deleted back to a single line returns to `Norm` (FR-012).
    /// `Laat` is left untouched (it ends only via `Esc Esc`/submit/push).
    fn reconcile_mode_after_edit(&mut self) {
        let lines = self.input.line_count();
        match self.mode {
            InputMode::Norm if lines > 1 => self.set_mode(InputMode::Mult),
            InputMode::Mult if lines <= 1 => self.set_mode(InputMode::Norm),
            _ => {}
        }
    }

    /// Push the composing input onto the one-item stack (sprint 007, FR-018):
    /// snapshot the buffer, caret, mode, stash, and LAAT state, then reset the
    /// pad to an empty `Norm` for an ad-hoc command. A push while the slot is
    /// occupied is a no-op so the first saved state is never lost (FR-020).
    fn push_input(&mut self) {
        if self.pushed.is_some() {
            return;
        }
        let snapshot = crate::input::InputSnapshot::capture(
            &self.input,
            self.mode,
            self.history.stash().map(str::to_string),
            self.laat.clone(),
        );
        self.pushed = Some(snapshot);
        self.input.clear();
        self.history.set_stash(None);
        self.set_mode(InputMode::Norm);
    }

    /// Restore a pushed input snapshot after an ad-hoc submission (sprint 007,
    /// FR-019): reinstate the buffer, caret, mode, stashed draft, and LAAT state,
    /// then clear the slot. A no-op when nothing is pushed.
    fn pop_input(&mut self) {
        if let Some(snapshot) = self.pushed.take() {
            let (mode, stash, laat) = snapshot.restore(&mut self.input);
            self.history.set_stash(stash);
            self.mode = mode;
            self.laat = laat;
        }
    }

    fn submit(&mut self) {
        self.submit_line();
        // Any submitted line pops a pushed snapshot, restoring the composing
        // input the user set aside (sprint 007, FR-019).
        self.pop_input();
    }

    fn submit_line(&mut self) {
        // The status message persists until the next submission or `Esc Esc`,
        // never a timeout (FR-025/FR-026): a fresh submit clears it.
        self.notice = None;

        // A multi-line selection submits as one combined submission in any mode,
        // overriding LAAT line-stepping (FR-017; selection overrides the
        // highlight). The buffer is left intact (the selection is the unit).
        if let Some(text) = self.input.selected_text() {
            if text.contains('\n') {
                self.input.cancel_selection();
                self.run_submission(text);
                return;
            }
        }

        // LAAT: submit only the highlighted line and arm `pending` for exit-code
        // gating, keeping the buffer for further stepping (FR-003/FR-005).
        if self.mode == InputMode::Laat {
            let row = self.laat.as_ref().map_or(0, |l| l.highlight);
            let line = self.input.line_text(row);
            if let Some(laat) = self.laat.as_mut() {
                laat.submit_line(row);
            }
            self.run_submission(line);
            return;
        }

        // Norm / Mult: submit the whole buffer as one unit, clearing the pad.
        let line = self.input.take_submit();
        self.run_submission(line);
        // Submitting a `Mult` buffer returns to `Norm` (FR-014).
        if self.mode == InputMode::Mult {
            self.set_mode(InputMode::Norm);
        }
    }

    /// Route and run one submitted line: record it in history, reset the
    /// transcript scroll/selection, and dispatch it as a slash or shell command.
    /// Shared by the `Norm`/`Mult` whole-buffer submit and the `Laat` line submit.
    fn run_submission(&mut self, line: String) {
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
            // `/status` toggles the fixed status bar config flag (FR-026). The
            // bar is rendered in a later sprint phase; the flag is honored then.
            Dispatch::Command(SlashCommand::Status) => {
                self.config.status.enabled = !self.config.status.enabled;
                let state = if self.config.status.enabled {
                    "enabled"
                } else {
                    "disabled"
                };
                self.synthetic_block(
                    format!("{}status", self.config.leader_char),
                    &format!("Status bar {state}.\n"),
                );
            }
            // `/keys` lists the live effective key bindings (FR-014). Only the
            // default (`norm`) mode is populated this sprint.
            Dispatch::Command(SlashCommand::Keys) => {
                let mut text = String::from("Active key bindings:\n\n");
                for (name, keys) in self
                    .config
                    .keymaps
                    .for_mode(crate::action::DEFAULT_MODE)
                    .listing()
                {
                    text.push_str(&format!("  {keys:<14}  {name}\n"));
                }
                self.synthetic_block(format!("{}keys", self.config.leader_char), &text);
            }
            // `/reload-config` re-reads configuration on demand and swaps the
            // effective config + keymaps only on success, never touching the
            // in-progress input buffer (FR-015/FR-016/FR-017).
            Dispatch::Command(SlashCommand::ReloadConfig) => self.reload_config(),
            // `/save <path>` writes the previous block's exact output to a file,
            // prompting before overwriting an existing file (sprint 007, FR-021).
            Dispatch::Command(SlashCommand::Save(path)) => self.run_save(&path),
            // `/filter <cmd>` pipes the previous block's output through `<cmd>`
            // via the shell, chaining into a new block (sprint 007, FR-025).
            Dispatch::Command(SlashCommand::Filter(cmd)) => self.run_filter(&cmd),
            // `/load <path>` loads a file's lines into the buffer and enters
            // `Laat` with the first line highlighted (sprint 007, FR-028).
            Dispatch::Command(SlashCommand::Load(path)) => self.run_load(&path),
            Dispatch::Unknown(name) => {
                let text = builtins::unknown_text(&name, self.config.leader_char);
                self.synthetic_block(format!("{}{}", self.config.leader_char, name), &text);
            }
        }
    }

    /// The most recent sealed, non-synthetic block's stored output — the
    /// "previous buffer" that `/save` and `/filter` act on (sprint 007,
    /// FR-021/FR-024/FR-025). `None` when no real command output is retained.
    fn previous_block_text(&self) -> Option<String> {
        self.store
            .iter()
            .filter(|b| matches!(b.state, crate::session::BlockState::Closed) && !b.synthetic)
            .last()
            .map(|b| b.output_lossy())
    }

    /// Resolve a slash-command path argument relative to `App.cwd`, expanding a
    /// leading `~`/`~/` to the home directory (sprint 007, FR-021/FR-028).
    fn resolve_path(&self, arg: &str) -> std::path::PathBuf {
        use std::path::PathBuf;
        let expanded: PathBuf = if arg == "~" {
            std::env::var_os("HOME").map_or_else(|| PathBuf::from(arg), PathBuf::from)
        } else if let Some(rest) = arg.strip_prefix("~/") {
            match std::env::var_os("HOME") {
                Some(home) => PathBuf::from(home).join(rest),
                None => PathBuf::from(arg),
            }
        } else {
            PathBuf::from(arg)
        };
        if expanded.is_absolute() {
            expanded
        } else {
            self.cwd.join(expanded)
        }
    }

    /// Handle `/save <path>` (FR-021…FR-024): an empty path leaves the buffer
    /// intact and reports the requirement; a missing previous block reports the
    /// failure; an existing target defers to an overwrite prompt; otherwise the
    /// previous block's exact bytes are written.
    fn run_save(&mut self, arg: &str) {
        if arg.is_empty() {
            self.notice = Some("'/save' requires path".into());
            return;
        }
        let Some(text) = self.previous_block_text() else {
            self.notice = Some("Save failed, previous buffer not found".into());
            return;
        };
        let path = self.resolve_path(arg);
        let bytes = text.into_bytes();
        if path.exists() {
            self.notice = Some("File exists, [O]verwrite, [A]ppend, [C]ancel?".into());
            self.pending_prompt = Some(PendingPrompt { path, bytes });
        } else {
            self.write_save(&path, &bytes, false);
        }
    }

    /// Resolve the pending `/save` overwrite prompt from the next key (FR-023):
    /// `O` overwrites, `A` appends, `C`/`Esc` cancels; any other key keeps the
    /// prompt up.
    fn resolve_save_prompt(&mut self, code: KeyCode) {
        let Some(prompt) = self.pending_prompt.take() else {
            return;
        };
        match code {
            KeyCode::Char('o') | KeyCode::Char('O') => {
                self.write_save(&prompt.path, &prompt.bytes, false)
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.write_save(&prompt.path, &prompt.bytes, true)
            }
            KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => {
                self.notice = Some("save canceled".into());
            }
            // Any other key keeps the prompt up for a deliberate choice.
            _ => self.pending_prompt = Some(prompt),
        }
    }

    /// Write (or append) `bytes` to `path`, surfacing any filesystem error as a
    /// status message rather than a panic (system boundary, Constitution VII).
    fn write_save(&mut self, path: &std::path::Path, bytes: &[u8], append: bool) {
        use std::io::Write;
        let result = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(append)
            .truncate(!append)
            .open(path)
            .and_then(|mut file| file.write_all(bytes));
        self.notice = Some(match result {
            Ok(()) if append => format!("appended to {}", path.display()),
            Ok(()) => format!("saved to {}", path.display()),
            Err(err) => format!("save failed: {err}"),
        });
    }

    /// Handle `/filter <cmd>` (FR-025…FR-027): write the previous block's output
    /// to a temp file and submit `cat <temp> | <cmd>` to the shell as a normal
    /// block titled `{leader}filter <cmd>`, so it chains as the new previous
    /// output. A non-zero exit is surfaced when the block completes.
    fn run_filter(&mut self, cmd: &str) {
        if cmd.is_empty() {
            self.notice = Some("'/filter' requires a command".into());
            return;
        }
        let Some(text) = self.previous_block_text() else {
            self.notice = Some("previous buffer not found".into());
            return;
        };
        let path = filter_temp_path();
        if let Err(err) = std::fs::write(&path, text.as_bytes()) {
            self.notice = Some(format!("filter failed: {err}"));
            return;
        }
        let label = format!("{}filter {}", self.config.leader_char, cmd);
        let command = format!("cat {} | {}", shell_single_quote(&path), cmd);
        self.filter_active = true;
        self.run_shell_labeled(label, command);
    }

    /// Handle `/load <path>` (FR-028): read the file's lines into the input
    /// buffer and enter `Laat` with the first line highlighted. A missing or
    /// unreadable file reports a status message and does not enter `Laat` with a
    /// partial buffer.
    fn run_load(&mut self, arg: &str) {
        if arg.is_empty() {
            self.notice = Some("'/load' requires path".into());
            return;
        }
        let path = self.resolve_path(arg);
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                // Drop a single trailing newline so the buffer has no empty
                // final line to step onto.
                let body = contents.strip_suffix('\n').unwrap_or(&contents);
                self.input.set_contents(body.to_string());
                self.input.set_caret_line_start(0);
                self.set_mode(InputMode::Laat);
            }
            Err(err) => self.notice = Some(format!("load failed: {err}")),
        }
    }

    /// Re-read configuration on demand for `/reload-config` (FR-015). On success,
    /// swap the effective `config` (which carries the `keymaps`) so new bindings
    /// take effect immediately; on a malformed config, report the error and keep
    /// the previous configuration (FR-016). The in-progress input buffer and any
    /// active selection are never touched (FR-017).
    fn reload_config(&mut self) {
        let command = format!("{}reload-config", self.config.leader_char);
        let Some(path) = self.config_path.clone() else {
            self.synthetic_block(
                command,
                "No configuration file to reload; running on defaults.\n",
            );
            return;
        };
        match Config::load(Some(&path)) {
            Ok(mut new_config) => {
                // The wrapped shell is already running and is not re-spawned by a
                // reload, so keep the currently-effective shell rather than the
                // file's (possibly changed) value.
                new_config.shell = self.config.shell.clone();
                self.config = new_config;
                self.synthetic_block(
                    command,
                    &format!("Configuration reloaded from {}.\n", path.display()),
                );
            }
            Err(e) => {
                self.synthetic_block(
                    command,
                    &format!("Reload failed: {e}\nKeeping the previous configuration.\n"),
                );
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
        self.run_shell_labeled(line.clone(), line);
    }

    /// Run `command` in the shell while showing `label` as the block's title.
    /// `/filter` uses this to run a composed `cat <temp> | <cmd>` pipeline while
    /// the transcript shows the friendly `{leader}filter <cmd>` (sprint 007).
    fn run_shell_labeled(&mut self, label: String, command: String) {
        let id = self.transcript.begin_block(label.clone());
        self.current_block = Some(id);
        // Mirror the boundary in the canonical store (OSC 133 `B`); its row
        // range is anchored as output arrives (R3, R7).
        self.current_store_block = Some(self.store.begin(label, Some(self.cwd.clone())));
        self.processor.begin_command();
        let _ = self.shell.send_command(&command);
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

/// A unique temp-file path under the system temp dir for a `/filter` payload
/// (sprint 007). Combines the process id with a per-process counter so back-to-
/// back filters never collide.
fn filter_temp_path() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut path = std::env::temp_dir();
    path.push(format!("kapollo-filter-{}-{}.txt", std::process::id(), n));
    path
}

/// Single-quote a path for safe inclusion in a shell command line, escaping any
/// embedded single quotes (sprint 007 `/filter`).
fn shell_single_quote(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    format!("'{}'", s.replace('\'', "'\\''"))
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
