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
