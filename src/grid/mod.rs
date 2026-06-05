//! Grid backend: a thin wrapper over the embedded `wezterm-term` emulator that
//! models the main screen as a grid of styled cells with scrollback (D25/D27,
//! sprint 004).
//!
//! The wrapper owns a `wezterm_term::Terminal` and feeds it raw PTY bytes via
//! [`Grid::advance_bytes`]. The emulator owns the escape parse, the cell grid,
//! scrollback, and `StableRowIndex` (an absolute, eviction-proof row id). kapollo
//! owns only the scroll position, which is why the read accessors take a
//! `scroll_offset` (rows scrolled up from the live bottom).

pub mod render;

use std::ops::Range;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use wezterm_term::color::ColorPalette;
use wezterm_term::{Line, StableRowIndex, Terminal, TerminalConfiguration, TerminalSize};

/// Sequence number type used by `wezterm-term` for damage tracking
/// (`wezterm_surface::SequenceNo`, a `usize`). Not re-exported by the crate, so
/// we mirror the alias here.
type SequenceNo = usize;

/// Default scrollback depth (rows). Overridable via config (`scroll.scrollback_lines`).
pub const DEFAULT_SCROLLBACK_LEN: usize = 10_000;

/// Terminal configuration handed to `wezterm-term`. Only `color_palette` is
/// required; we also raise the scrollback to a useful depth.
#[derive(Debug)]
struct GridConfig {
    scrollback: usize,
    palette: ColorPalette,
}

impl TerminalConfiguration for GridConfig {
    fn scrollback_size(&self) -> usize {
        self.scrollback
    }
    fn color_palette(&self) -> ColorPalette {
        self.palette.clone()
    }
}

/// Funnels the emulator's answerback bytes (DSR replies, device attributes, …)
/// into a channel so the event loop can forward them to the PTY.
struct ChannelWriter {
    tx: Sender<Vec<u8>>,
}

impl std::io::Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = self.tx.send(buf.to_vec());
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The main-screen grid: a `wezterm-term` emulator plus the answerback channel.
pub struct Grid {
    term: Terminal,
    answerback: Receiver<Vec<u8>>,
    rows: u16,
    cols: u16,
    /// Sequence number captured just before the most recent [`Grid::advance_bytes`],
    /// used by [`Grid::changed_rows`] to report damage since that feed.
    seqno_before_advance: SequenceNo,
}

fn make_size(rows: u16, cols: u16) -> TerminalSize {
    TerminalSize {
        rows: rows as usize,
        cols: cols as usize,
        pixel_width: 0,
        pixel_height: 0,
        dpi: 0,
    }
}

impl Grid {
    /// Construct a grid sized `rows × cols` with the default scrollback depth.
    pub fn new(rows: u16, cols: u16) -> Self {
        Self::with_scrollback(rows, cols, DEFAULT_SCROLLBACK_LEN)
    }

    /// Construct a grid with an explicit scrollback depth (rows).
    pub fn with_scrollback(rows: u16, cols: u16, scrollback: usize) -> Self {
        let (tx, answerback) = channel();
        let config: Arc<dyn TerminalConfiguration + Send + Sync> = Arc::new(GridConfig {
            scrollback,
            palette: ColorPalette::default(),
        });
        let term = Terminal::new(
            make_size(rows, cols),
            config,
            "kapollo",
            env!("CARGO_PKG_VERSION"),
            Box::new(ChannelWriter { tx }),
        );
        let seqno_before_advance = term.current_seqno();
        Self {
            term,
            answerback,
            rows,
            cols,
            seqno_before_advance,
        }
    }

    /// Feed a chunk of raw PTY output to the emulator. The emulator applies all
    /// escape sequences (cursor moves, `\r`, SGR, …) so in-place redraws happen
    /// natively (FR-003).
    pub fn advance_bytes(&mut self, bytes: &[u8]) {
        self.seqno_before_advance = self.term.current_seqno();
        self.term.advance_bytes(bytes);
    }

    /// Resize the emulator to a new PTY winsize.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.rows = rows;
        self.cols = cols;
        self.term.resize(make_size(rows, cols));
    }

    /// Clear the visible screen **and** the scrollback history, homing the
    /// cursor — the full reset behind `/clear` (FR-023). Injects the standard
    /// erase sequences (`ESC[3J` clears scrollback, `ESC[2J` the screen,
    /// `ESC[H` homes the cursor) so the emulator performs the clear natively.
    pub fn clear(&mut self) {
        self.advance_bytes(b"\x1b[H\x1b[2J\x1b[3J");
    }

    /// Current viewport dimensions (`rows`, `cols`).
    pub fn size(&self) -> (u16, u16) {
        (self.rows, self.cols)
    }

    /// Total rows held by the emulator, including scrollback history.
    pub fn scrollback_rows(&self) -> usize {
        self.term.screen().scrollback_rows()
    }

    /// Maximum scroll offset: how many rows of history sit above the live
    /// viewport. Scroll offsets are clamped to `[0, max_scroll()]`.
    pub fn max_scroll(&self) -> usize {
        self.scrollback_rows()
            .saturating_sub(self.term.screen().physical_rows)
    }

    /// `StableRowIndex` of the top visible line for a given scroll offset.
    pub fn top_stable_row(&self, scroll_offset: usize) -> StableRowIndex {
        let screen = self.term.screen();
        let len = screen.scrollback_rows();
        let phys = screen.physical_rows;
        let off = scroll_offset.min(len.saturating_sub(phys));
        let top_phys = len.saturating_sub(phys + off);
        screen.phys_to_stable_row_index(top_phys)
    }

    /// `StableRowIndex` of a given viewport row for a given scroll offset.
    pub fn stable_row_at(&self, scroll_offset: usize, viewport_row: u16) -> StableRowIndex {
        self.top_stable_row(scroll_offset) + viewport_row as StableRowIndex
    }

    /// The `physical_rows` visible lines for a given scroll offset, oldest first.
    pub fn viewport_lines(&self, scroll_offset: usize) -> Vec<Line> {
        let screen = self.term.screen();
        let len = screen.scrollback_rows();
        let phys = screen.physical_rows;
        let off = scroll_offset.min(len.saturating_sub(phys));
        let top_phys = len.saturating_sub(phys + off);
        screen.lines_in_phys_range(top_phys..top_phys + phys)
    }

    /// The visible viewport as a row-major grid of single-cell strings, padded to
    /// `cols`. Wide-cell continuation columns are empty strings so column slices
    /// stay aligned. Used by the selection layer to extract copied text and to
    /// place the highlight (FR-007/011).
    pub fn viewport_cells(&self, scroll_offset: usize) -> Vec<Vec<String>> {
        let width = self.cols as usize;
        self.viewport_lines(scroll_offset)
            .iter()
            .map(|line| {
                let mut row = vec![" ".to_string(); width];
                for cell in line.visible_cells() {
                    let x = cell.cell_index();
                    if x >= width {
                        continue;
                    }
                    let s = cell.str();
                    row[x] = if s.is_empty() {
                        " ".to_string()
                    } else {
                        s.to_string()
                    };
                    for k in 1..cell.width() {
                        if x + k < width {
                            row[x + k] = String::new();
                        }
                    }
                }
                row
            })
            .collect()
    }

    /// Range of `StableRowIndex` damaged since the most recent
    /// [`Grid::advance_bytes`]. Empty (`start == end`) when nothing changed (SC-003).
    pub fn changed_rows(&self) -> Range<StableRowIndex> {
        let screen = self.term.screen();
        let total = screen.scrollback_rows();
        let top = screen.phys_to_stable_row_index(0);
        let bottom = top + total as StableRowIndex;
        let changed = screen.get_changed_stable_rows(top..bottom, self.seqno_before_advance);
        match (changed.first(), changed.last()) {
            (Some(&lo), Some(&hi)) => lo..hi + 1,
            _ => 0..0,
        }
    }

    /// Whether the wrapped program currently owns the alternate screen.
    pub fn is_alt_screen_active(&self) -> bool {
        self.term.is_alt_screen_active()
    }

    /// Whether the wrapped program has enabled mouse tracking (mouse events
    /// should route to the child while this is true; D28).
    pub fn is_mouse_grabbed(&self) -> bool {
        self.term.is_mouse_grabbed()
    }

    /// Cursor position as `(col, row)` within the visible viewport, clamped to
    /// non-negative coordinates.
    pub fn cursor(&self) -> (u16, u16) {
        let pos = self.term.cursor_pos();
        let x = pos.x.min(u16::MAX as usize) as u16;
        let y = pos.y.max(0).min(u16::MAX as i64) as u16;
        (x, y)
    }

    /// Drain any answerback bytes the emulator produced (device-status replies,
    /// etc.) for forwarding to the PTY.
    pub fn drain_answerback(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        while let Ok(bytes) = self.answerback.try_recv() {
            out.extend_from_slice(&bytes);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell_text(grid: &Grid, scroll_offset: usize, row: usize) -> String {
        let lines = grid.viewport_lines(scroll_offset);
        lines[row]
            .visible_cells()
            .map(|c| c.str().to_string())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn carriage_return_updates_one_row_in_place() {
        // "a\rb" must leave a single row reading "b", not two rows (FR-003).
        let mut grid = Grid::new(24, 80);
        grid.advance_bytes(b"a\rb");
        assert_eq!(cell_text(&grid, 0, 0), "b");
    }

    #[test]
    fn newline_advances_to_next_row() {
        let mut grid = Grid::new(24, 80);
        grid.advance_bytes(b"one\r\ntwo");
        assert_eq!(cell_text(&grid, 0, 0), "one");
        assert_eq!(cell_text(&grid, 0, 1), "two");
    }

    #[test]
    fn changed_rows_reports_single_row_after_one_update() {
        let mut grid = Grid::new(24, 80);
        grid.advance_bytes(b"hello");
        let changed = grid.changed_rows();
        assert_eq!(changed.end - changed.start, 1);
    }

    #[test]
    fn alt_screen_state_tracks_dec_mode() {
        let mut grid = Grid::new(24, 80);
        assert!(!grid.is_alt_screen_active());
        grid.advance_bytes(b"\x1b[?1049h");
        assert!(grid.is_alt_screen_active());
        grid.advance_bytes(b"\x1b[?1049l");
        assert!(!grid.is_alt_screen_active());
    }

    #[test]
    fn clear_erases_screen_and_scrollback() {
        let mut grid = Grid::new(4, 80);
        // Fill well past the viewport so rows fall into scrollback.
        for i in 0..20 {
            grid.advance_bytes(format!("line{i}\r\n").as_bytes());
        }
        assert!(grid.max_scroll() > 0, "expected scrollback to accumulate");

        grid.clear();

        // Scrollback is gone and the visible viewport is blank.
        assert_eq!(grid.max_scroll(), 0);
        assert_eq!(cell_text(&grid, 0, 0), "");
        assert_eq!(grid.cursor(), (0, 0));
    }

    #[test]
    fn stable_row_at_increases_down_the_viewport() {
        let mut grid = Grid::new(24, 80);
        grid.advance_bytes(b"x");
        let r0 = grid.stable_row_at(0, 0);
        let r1 = grid.stable_row_at(0, 1);
        assert_eq!(r1, r0 + 1);
    }
}
