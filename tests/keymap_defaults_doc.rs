//! Example-config sync test (T032): `docs/keymap-defaults.toml` must rebuild the
//! exact built-in default map, so the published reference can never drift from
//! the code (Constitution III + V; FR-019). See
//! `specs/006-keymap-engine/contracts/keymap-config.md`.

use std::path::Path;

use kapollo::action::Keymap;
use kapollo::config::Config;

#[test]
fn example_keymap_doc_matches_default_map() {
    let toml = include_str!("../docs/keymap-defaults.toml");
    let cfg = Config::from_toml(toml, Path::new("docs/keymap-defaults.toml"))
        .expect("docs/keymap-defaults.toml parses");
    assert_eq!(
        *cfg.keymaps.default(),
        Keymap::default_map(),
        "docs/keymap-defaults.toml drifted from Keymap::default_map(); regenerate it"
    );
}
