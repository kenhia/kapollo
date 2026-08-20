//! Config loading tests (T007). Validates defaults, missing-key fallback,
//! unknown-key tolerance, cap clamping, and invalid-TOML errors per FR-028,
//! FR-029 and `contracts/config.md`.

use std::path::Path;

use kapollo::config::{Caps, Config, PER_BLOCK_BYTES_HARD_MAX};

#[test]
fn absent_file_yields_defaults() {
    let cfg = Config::load(Some(Path::new("/nonexistent/kapollo/does-not-exist.toml")))
        .expect("absent file should yield defaults, not an error");
    assert_eq!(cfg, Config::default());
}

#[test]
fn missing_keys_use_defaults() {
    let cfg = Config::from_toml("leader_char = \"#\"\n", Path::new("test.toml"))
        .expect("partial config should parse");
    assert_eq!(cfg.leader_char, '#');
    assert_eq!(cfg.shell, None);
    assert_eq!(cfg.caps, Caps::default());
}

#[test]
fn unknown_keys_are_ignored() {
    let text = "\
bogus_top_level = 1
[caps]
mystery = true
per_block_lines = 10
";
    let cfg = Config::from_toml(text, Path::new("test.toml"))
        .expect("unknown keys must be ignored, not fatal");
    assert_eq!(cfg.caps.per_block_lines, 10);
    // Other caps remain at their defaults.
    assert_eq!(cfg.caps.per_block_bytes, Caps::default().per_block_bytes);
}

#[test]
fn input_prompt_defaults_on_and_can_be_disabled() {
    // kwi #37: the input-pad prompt glyph is opt-out.
    assert!(Config::default().input_prompt);
    let cfg = Config::from_toml("input_prompt = false\n", Path::new("test.toml"))
        .expect("input_prompt should parse");
    assert!(!cfg.input_prompt);
}

#[test]
fn whitespace_suppression_knobs_default_to_trailing_only() {
    // kwi #46: trailing-strip on, all-lines off, both configurable.
    let d = Config::default();
    assert!(!d.suppress_multiline_whitespace);
    assert!(d.suppress_multiline_trailing_whitespace_lines);

    let cfg = Config::from_toml(
        "suppress_multiline_whitespace = true\nsuppress_multiline_trailing_whitespace_lines = false\n",
        Path::new("test.toml"),
    )
    .expect("suppression knobs should parse");
    assert!(cfg.suppress_multiline_whitespace);
    assert!(!cfg.suppress_multiline_trailing_whitespace_lines);
}

#[test]
fn running_color_defaults_yellow_and_parses_names() {
    // kwi #35: the input-pad prompt wears this color while a command runs.
    use ratatui::style::Color;
    assert_eq!(Config::default().running_color, Color::Yellow);
    let cfg = Config::from_toml("running_color = \"cyan\"\n", Path::new("test.toml"))
        .expect("running_color should parse");
    assert_eq!(cfg.running_color, Color::Cyan);
    // An unknown color name warns and keeps the default, like prompt_color.
    let cfg = Config::from_toml("running_color = \"nonsense\"\n", Path::new("test.toml"))
        .expect("unknown color must not be fatal");
    assert_eq!(cfg.running_color, Color::Yellow);
}

#[test]
fn per_block_bytes_clamped_to_hard_max() {
    let text = format!(
        "[caps]\nper_block_bytes = {}\n",
        PER_BLOCK_BYTES_HARD_MAX + 1_000_000
    );
    let cfg = Config::from_toml(&text, Path::new("test.toml")).expect("config should parse");
    assert_eq!(cfg.caps.per_block_bytes, PER_BLOCK_BYTES_HARD_MAX);
}

#[test]
fn invalid_toml_errors() {
    let result = Config::from_toml("this is = = not valid toml", Path::new("bad.toml"));
    assert!(result.is_err(), "invalid TOML must produce an error");
}

#[test]
fn invalid_leader_char_errors() {
    let result = Config::from_toml("leader_char = \"too long\"\n", Path::new("bad.toml"));
    assert!(result.is_err(), "multi-character leader_char must error");
}

#[test]
fn prompt_defaults_when_absent() {
    let cfg = Config::default();
    assert_eq!(cfg.prompt_char, 'λ');
    assert_eq!(cfg.prompt_color, ratatui::style::Color::Red);
}

#[test]
fn prompt_char_and_color_parse() {
    let cfg = Config::from_toml(
        "prompt_char = \"❯\"\nprompt_color = \"cyan\"\n",
        Path::new("test.toml"),
    )
    .expect("prompt keys should parse");
    assert_eq!(cfg.prompt_char, '❯');
    assert_eq!(cfg.prompt_color, ratatui::style::Color::Cyan);
}

#[test]
fn multi_char_prompt_char_errors() {
    let result = Config::from_toml("prompt_char = \">>\"\n", Path::new("bad.toml"));
    assert!(result.is_err(), "multi-character prompt_char must error");
}

#[test]
fn unknown_prompt_color_falls_back_to_default() {
    let cfg = Config::from_toml("prompt_color = \"chartreuse\"\n", Path::new("test.toml"))
        .expect("unknown color must warn-and-default, not error");
    assert_eq!(cfg.prompt_color, ratatui::style::Color::Red);
}

#[test]
fn status_and_context_lines_default() {
    // Sprint 005 surface defaults (FR-026, US3): status bar on, 3 lines overlap.
    let cfg = Config::default();
    assert!(cfg.status.enabled);
    assert_eq!(cfg.scroll.context_lines, 3);
}

#[test]
fn status_and_context_lines_parse() {
    let cfg = Config::from_toml(
        "[status]\nenabled = false\n[scroll]\ncontext_lines = 5\n",
        Path::new("test.toml"),
    )
    .expect("sprint 005 keys should parse");
    assert!(!cfg.status.enabled);
    assert_eq!(cfg.scroll.context_lines, 5);
    // Existing scroll keys keep their defaults (FR-033).
    assert_eq!(cfg.scroll.wheel_lines, 3);
}

#[test]
fn unknown_status_key_is_ignored() {
    let cfg = Config::from_toml(
        "[status]\nenabled = true\nbogus = 1\n",
        Path::new("test.toml"),
    )
    .expect("unknown [status] key must be ignored, not fatal");
    assert!(cfg.status.enabled);
}

#[test]
fn divider_defaults_on_and_parses() {
    // The cosmetic dividing rule is shown by default (Apollo / Domain OS
    // lineage) and can be turned off via [divider].
    assert!(Config::default().divider.enabled);
    let cfg = Config::from_toml("[divider]\nenabled = false\n", Path::new("test.toml"))
        .expect("[divider] should parse");
    assert!(!cfg.divider.enabled);
}

#[test]
fn unknown_divider_key_is_ignored() {
    let cfg = Config::from_toml(
        "[divider]\nenabled = true\nbogus = 1\n",
        Path::new("test.toml"),
    )
    .expect("unknown [divider] key must be ignored, not fatal");
    assert!(cfg.divider.enabled);
}
