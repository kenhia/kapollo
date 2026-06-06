//! Fixed status bar (sprint 005, US4): a single non-scrolling row pinned
//! *beneath* the input pad carrying the mode, working directory, a transient
//! message, and the last exit code (FR-017..FR-024).
//!
//! Layout is `mode | cwd<greedypad>| message | exit`: the 4-column mode field
//! and its `" | "` separator are always present, a greedy pad sits between the
//! cwd and the `|` so the right cluster hugs the right edge, and the message is
//! right-justified against the exit field. Under width pressure the message is
//! truncated first (trailing ellipsis), then the cwd (middle ellipsis keeping
//! the trailing component); mode and exit are never broken and the bar never
//! wraps (FR-019, FR-024).

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;

/// The mode field is a fixed four columns.
const MODE_WIDTH: usize = 4;
/// The default mode literal until richer modes exist.
pub const DEFAULT_MODE: &str = "norm";
const ELLIPSIS: char = '…';
/// Below this terminal height the bar is hidden so a short window keeps every
/// row for the transcript and input (FR-022).
pub const MIN_ROWS_FOR_BAR: u16 = 10;

/// Whether the fixed status bar should be shown: it is opt-out via config and
/// suppressed on a short terminal (FR-022, FR-026).
pub fn is_visible(enabled: bool, rows: u16) -> bool {
    enabled && rows >= MIN_ROWS_FOR_BAR
}

/// Compose the status bar's single line to exactly `width` columns.
///
/// `mode` is normalized to four columns. `message` is an optional transient
/// notice. `exit` is shown only when `Some` and is never broken (FR-023).
pub fn fit(
    width: usize,
    mode: &str,
    cwd: &str,
    message: Option<&str>,
    exit: Option<i32>,
) -> String {
    if width == 0 {
        return String::new();
    }
    let mode4 = fit_mode(mode);
    let prefix = format!("{mode4} | ");
    let prefix_len = prefix.chars().count();
    let exit_str = exit.map(|c| c.to_string());
    let exit_ref = exit_str.as_deref();
    let body = width.saturating_sub(prefix_len);

    let cwd_full_len = cwd.chars().count();
    let pad_min = |rw: usize| usize::from(rw > 0);

    let mut cwd_str = cwd.to_string();
    let mut msg = message.unwrap_or("").to_string();
    let mut rw = right_text(&msg, exit_ref).chars().count();

    if cwd_full_len + pad_min(rw) + rw > body {
        // Step 1: shrink the message, keeping the cwd intact (FR-019 ordering).
        let avail_right = body.saturating_sub(cwd_full_len + 1);
        let msg_max = match exit_ref {
            // right = "| " + msg + " | " + exit → overhead 5 + exit width.
            Some(e) => avail_right.saturating_sub(5 + e.chars().count()),
            // right = "| " + msg → overhead 2.
            None => avail_right.saturating_sub(2),
        };
        msg = if msg_max >= 2 {
            truncate_end(&msg, msg_max)
        } else {
            String::new()
        };
        rw = right_text(&msg, exit_ref).chars().count();

        // Step 2: if the cwd alone still overflows, middle-ellipsis it.
        if cwd_full_len + pad_min(rw) + rw > body {
            let cwd_max = body.saturating_sub(pad_min(rw) + rw);
            cwd_str = truncate_cwd_middle(cwd, cwd_max);
        }
    }

    let right = right_text(&msg, exit_ref);
    let rw = right.chars().count();
    let cwd_len = cwd_str.chars().count();
    let pad = body.saturating_sub(cwd_len + rw).max(pad_min(rw));

    let mut out = String::with_capacity(width);
    out.push_str(&prefix);
    out.push_str(&cwd_str);
    out.push_str(&" ".repeat(pad));
    out.push_str(&right);

    let outlen = out.chars().count();
    if outlen > width {
        out.chars().take(width).collect()
    } else {
        out.push_str(&" ".repeat(width - outlen));
        out
    }
}

/// The right cluster (`| message | exit`, absent pieces elided), introduced by
/// its `"| "` divider; empty when there is neither a message nor an exit code.
fn right_text(msg: &str, exit: Option<&str>) -> String {
    match (msg.is_empty(), exit) {
        (false, Some(e)) => format!("| {msg} | {e}"),
        (false, None) => format!("| {msg}"),
        (true, Some(e)) => format!("| {e}"),
        (true, None) => String::new(),
    }
}

fn fit_mode(mode: &str) -> String {
    let mut s: String = mode.chars().take(MODE_WIDTH).collect();
    while s.chars().count() < MODE_WIDTH {
        s.push(' ');
    }
    s
}

/// Truncate `s` to at most `max` columns, appending an ellipsis (and trimming a
/// trailing space before it) when shortened.
fn truncate_end(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let keep: String = s.chars().take(max - 1).collect();
    let mut out = keep.trim_end().to_string();
    out.push(ELLIPSIS);
    out
}

/// Truncate `cwd` to at most `max` columns with a middle ellipsis, favoring the
/// trailing path component so the current directory stays legible.
fn truncate_cwd_middle(cwd: &str, max: usize) -> String {
    let n = cwd.chars().count();
    if n <= max {
        return cwd.to_string();
    }
    if max <= 1 {
        return ELLIPSIS.to_string();
    }
    let avail = max - 1;
    let tail = avail * 2 / 3;
    let head = avail - tail;
    let head_str: String = cwd.chars().take(head).collect();
    let tail_str: String = cwd.chars().skip(n - tail).collect();
    format!("{head_str}{ELLIPSIS}{tail_str}")
}

/// Render the fixed status bar into `area` (a single row beneath the input).
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let text = fit(
        area.width as usize,
        DEFAULT_MODE,
        &app.cwd.display().to_string(),
        app.notice.as_deref(),
        app.last_exit,
    );
    let mut line = Line::from(text);
    if super::color_enabled() {
        line = line.style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        );
    }
    frame.render_widget(Paragraph::new(line), area);
}
