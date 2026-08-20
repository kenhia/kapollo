//! The dividing rule between the output pad and the input pad (sprint 005) —
//! kapollo's visual lineage back to the Apollo / Domain OS display manager.
//! Optionally folds the wrapped shell's captured prompt into the rule (sprint
//! 010, kwi #47): `── ken@host /tmp ────────`, with the prompt's tail brought
//! down into the input pad as its prefix.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// The glyph the rule is drawn with (U+2500 BOX DRAWINGS LIGHT HORIZONTAL).
const RULE: char = '─';

/// Rule columns drawn before the folded prompt head.
const LEAD_RULE: usize = 2;

/// Build the rule's text filling exactly `width` columns.
pub fn rule(width: usize) -> String {
    RULE.to_string().repeat(width)
}

/// Split a captured prompt into `(divider head, input-pad prefix)` (kwi #47).
/// A multi-line prompt (e.g. starship) puts its last non-empty line into the
/// input pad and the rest — joined with a space — into the divider. A one-line
/// prompt brings its trailing `bring_down` characters down (e.g. `> `) and
/// folds the remainder into the divider. An all-whitespace prompt yields two
/// empty strings (callers render the plain rule).
pub fn split_prompt(prompt: &str, bring_down: usize) -> (String, String) {
    let lines: Vec<&str> = prompt
        .split('\n')
        .filter(|line| !line.trim().is_empty())
        .collect();
    match lines.as_slice() {
        [] => (String::new(), String::new()),
        [one] => {
            let chars: Vec<char> = one.chars().collect();
            let cut = chars.len().saturating_sub(bring_down);
            (
                chars[..cut].iter().collect::<String>().trim_end().into(),
                chars[cut..].iter().collect(),
            )
        }
        [head @ .., tail] => (head.join(" "), (*tail).to_string()),
    }
}

/// Render the dividing rule into `area` (a single row above the input pad).
/// With a folded prompt head (kwi #47), the rule reads
/// `── {head} ─────…`; otherwise it is the plain full-width line.
pub fn render(frame: &mut Frame, area: Rect, color: bool, prompt_head: Option<&str>) {
    let rule_style = if color {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    };
    let width = area.width as usize;
    let line = match prompt_head.filter(|head| !head.is_empty()) {
        Some(head) => {
            // Truncate an oversize head so at least one trailing rule glyph
            // survives to read as a rule.
            let max_head = width.saturating_sub(LEAD_RULE + 3);
            let head: String = head.chars().take(max_head).collect();
            let used = LEAD_RULE + 1 + head.chars().count() + 1;
            Line::from(vec![
                Span::styled(format!("{} ", rule(LEAD_RULE)), rule_style),
                Span::raw(head),
                Span::styled(format!(" {}", rule(width.saturating_sub(used))), rule_style),
            ])
        }
        None => Line::from(rule(width)).style(rule_style),
    };
    frame.render_widget(Paragraph::new(line), area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_prompt_splits_at_the_bring_down_count() {
        // The kwi #47 example: trailing "> " comes down, the rest folds up.
        let (head, tail) = split_prompt("ken@host /tmp/log > ", 2);
        assert_eq!(head, "ken@host /tmp/log");
        assert_eq!(tail, "> ");
    }

    #[test]
    fn multi_line_prompt_brings_its_last_line_down() {
        // A starship-shaped capture: leading blank separator, info line, λ line.
        let (head, tail) = split_prompt("\nken@kai .../kapollo via v1.97\nλ ", 2);
        assert_eq!(head, "ken@kai .../kapollo via v1.97");
        assert_eq!(tail, "λ ");
    }

    #[test]
    fn bring_down_zero_folds_the_whole_prompt_into_the_divider() {
        let (head, tail) = split_prompt("host$ ", 0);
        assert_eq!(head, "host$");
        assert_eq!(tail, "");
    }

    #[test]
    fn oversize_bring_down_brings_the_whole_line_down() {
        let (head, tail) = split_prompt("$ ", 10);
        assert_eq!(head, "");
        assert_eq!(tail, "$ ");
    }

    #[test]
    fn whitespace_only_prompt_yields_nothing() {
        assert_eq!(split_prompt("  \n\t\n", 2), (String::new(), String::new()));
    }
}
