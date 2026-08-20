//! Output processing: routes raw PTY bytes through the active boundary
//! detector (OSC 133 or sentinel) and gates output capture, yielding an
//! ordered event stream (FR-004, FR-006, FR-009; research R6). Associating
//! those events with blocks is the [`BlockAssembler`]'s job — it owns the
//! in-flight command FIFO that makes typed-ahead submissions safe (kwi #34).

pub mod assembler;
pub mod parser;
pub mod sentinel;

pub use assembler::{BlockAssembler, ClosedBlock};
pub use parser::{Boundary, Osc133Parser, ProcessorEvent};
pub use sentinel::SentinelScanner;

use crate::pty::BoundaryMode;

/// Drives block-boundary detection and capture gating over the PTY stream.
pub struct OutputProcessor {
    osc: Option<Osc133Parser>,
    sentinel: Option<SentinelScanner>,
    alt_screen: bool,
    capturing: bool,
}

impl OutputProcessor {
    /// Construct a processor matching the session's boundary mode.
    pub fn for_mode(mode: BoundaryMode, nonce: &str) -> Self {
        match mode {
            BoundaryMode::Osc133 => Self::osc133(),
            BoundaryMode::Sentinel => Self::sentinel(nonce),
        }
    }

    /// OSC 133 mode: output is captured only between the `C` and `D` marks, so
    /// the echoed command line and prompt are excluded.
    pub fn osc133() -> Self {
        Self {
            osc: Some(Osc133Parser::new()),
            sentinel: None,
            alt_screen: false,
            capturing: false,
        }
    }

    /// Sentinel mode: there is no output-start mark, so capture runs for the
    /// whole span between command submit and the nonce.
    pub fn sentinel(nonce: &str) -> Self {
        Self {
            osc: None,
            sentinel: Some(SentinelScanner::new(nonce)),
            alt_screen: false,
            capturing: true,
        }
    }

    /// Whether the wrapped program is currently in the alternate screen.
    pub fn in_alt_screen(&self) -> bool {
        self.alt_screen
    }

    /// Reset capture state for a command submitted while nothing is in flight.
    /// In OSC 133 mode wait for the `C` mark; in sentinel mode capture now.
    /// Callers must NOT invoke this while a command is still running — it would
    /// stop the running command's capture (kwi #34); the queued command's own
    /// capture opens at its `C` mark (or, in sentinel mode, is already on).
    pub fn begin_command(&mut self) {
        self.capturing = self.sentinel.is_some();
    }

    fn parse(&mut self, bytes: &[u8], out: &mut Vec<ProcessorEvent>) {
        if let Some(osc) = self.osc.as_mut() {
            osc.feed(bytes, out);
        } else if let Some(sentinel) = self.sentinel.as_mut() {
            sentinel.feed(bytes, out);
        }
    }

    /// Parse `bytes` into ordered [`ProcessorEvent`]s, dropping output spans
    /// that arrive while capture is off (before a command's `C` mark, between
    /// commands, or inside an alt-screen program). The caller feeds the
    /// surviving events to a [`BlockAssembler`] for block association.
    pub fn process(&mut self, bytes: &[u8]) -> Vec<ProcessorEvent> {
        let mut events = Vec::new();
        self.parse(bytes, &mut events);
        let mut out = Vec::with_capacity(events.len());
        for event in events {
            match event {
                ProcessorEvent::Output(data) => {
                    if self.capturing {
                        out.push(ProcessorEvent::Output(data));
                    }
                }
                ProcessorEvent::Boundary(boundary) => {
                    self.update_capture(&boundary);
                    out.push(ProcessorEvent::Boundary(boundary));
                }
            }
        }
        out
    }

    fn update_capture(&mut self, boundary: &Boundary) {
        match boundary {
            Boundary::OutputStart => self.capturing = true,
            Boundary::CommandEnd { .. } => {
                // OSC mode: capture resumes at the next command's `C` mark, so
                // the prompt and a queued command's echo are excluded. Sentinel
                // mode has no `C` mark: capture stays on so a typed-ahead
                // command's output is not lost (kwi #34); spans with nothing in
                // flight are dropped by the assembler instead.
                self.capturing = self.sentinel.is_some();
            }
            Boundary::AltScreenEnter => {
                self.alt_screen = true;
                // Suspend capture: the full-screen program owns the terminal
                // and its raw output is passed through, not recorded (FR-018).
                self.capturing = false;
            }
            Boundary::AltScreenLeave => {
                self.alt_screen = false;
                // Resume capture for any trailing output before the end mark.
                self.capturing = true;
            }
            // Prompt marks, mouse-tracking mode changes, and OSC 7 cwd reports
            // are surfaced to the caller; they do not affect capture state.
            Boundary::PromptStart
            | Boundary::CommandStart
            | Boundary::MouseTrackingEnable(_)
            | Boundary::MouseTrackingDisable(_)
            | Boundary::Cwd(_) => {}
        }
    }
}
