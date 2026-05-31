//! OSC 7 cwd tracking (T029): the shell reports its working directory with an
//! `ESC]7;file://host/abs-path ST` sequence, which the parser surfaces as a
//! `Boundary::Cwd` so the status rule can follow `cd` (FR-019).

use std::path::PathBuf;

use kapollo::config::Caps;
use kapollo::output::{Boundary, OutputProcessor};
use kapollo::session::Transcript;

fn apply(payload: &[u8]) -> Vec<Boundary> {
    let mut processor = OutputProcessor::osc133();
    let mut tx = Transcript::new(Caps::default());
    let mut current = None;
    processor.apply(payload, &mut tx, &mut current)
}

#[test]
fn osc7_reports_new_cwd_dropping_the_host() {
    let boundaries = apply(b"\x1b]7;file://myhost/tmp\x1b\\");
    assert!(
        boundaries.contains(&Boundary::Cwd(PathBuf::from("/tmp"))),
        "got {boundaries:?}"
    );
}

#[test]
fn osc7_with_empty_host_parses_absolute_path() {
    let boundaries = apply(b"\x1b]7;file:///home/ken/src\x1b\\");
    assert!(
        boundaries.contains(&Boundary::Cwd(PathBuf::from("/home/ken/src"))),
        "got {boundaries:?}"
    );
}

#[test]
fn osc7_percent_decodes_the_path() {
    let boundaries = apply(b"\x1b]7;file://host/tmp/a%20b\x1b\\");
    assert!(
        boundaries.contains(&Boundary::Cwd(PathBuf::from("/tmp/a b"))),
        "got {boundaries:?}"
    );
}

#[test]
fn malformed_osc7_yields_no_cwd_boundary() {
    let boundaries = apply(b"\x1b]7;not-a-uri\x1b\\");
    assert!(
        !boundaries.iter().any(|b| matches!(b, Boundary::Cwd(_))),
        "got {boundaries:?}"
    );
}
