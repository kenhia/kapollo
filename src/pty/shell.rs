//! Shell detection and integration-hook construction. kapollo wraps the user's
//! real shell (FR-002) and, for fish/bash, injects an OSC 133 hook so block
//! boundaries and exit codes can be parsed from the byte stream (FR-007,
//! research R2). The wrapped shell always gets `KAPOLLO_ACTIVE=1` and
//! `KAPOLLO_VERSION` (FR-008, research R12).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::Context;
use portable_pty::CommandBuilder;

use crate::pty::BoundaryMode;

/// Which shell kapollo is wrapping. Drives integration-hook selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Fish,
    Bash,
    Other,
}

impl ShellKind {
    /// Classify a shell from its executable path.
    pub fn detect(path: &str) -> Self {
        let name = Path::new(path)
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("");
        if name.contains("fish") {
            ShellKind::Fish
        } else if name.contains("bash") {
            ShellKind::Bash
        } else {
            ShellKind::Other
        }
    }
}

/// fish integration: define `fish_preexec`/`fish_postexec` event functions that
/// emit OSC 133 `C` (output start) and `D;<status>` (command end + exit code),
/// a `fish_prompt` event hook that emits OSC 133 `A` (prompt start) and OSC 7
/// (`file://host/cwd`, FR-019), and a wrapper around the user's `fish_prompt`
/// that emits OSC 133 `B` after the prompt renders — bracketing the prompt
/// text so kapollo can capture it (kwi #47). Event handlers run before the
/// prompt function, so the stream is `A`, OSC 7, prompt text, `B`. User config
/// is preserved because fish runs `--init-command` after config (fish 3.7
/// emits no OSC 133 marks of its own, verified by PTY probe).
const FISH_HOOK: &str = "function __kapollo_preexec --on-event fish_preexec; \
printf '\\e]133;C\\e\\\\'; end; \
function __kapollo_postexec --on-event fish_postexec; \
set -l __kapollo_status $status; printf '\\e]133;D;%s\\e\\\\' $__kapollo_status; end; \
function __kapollo_prompt_start --on-event fish_prompt; \
printf '\\e]133;A\\e\\\\'; end; \
function __kapollo_cwd --on-event fish_prompt; \
printf '\\e]7;file://%s%s\\e\\\\' (hostname) \"$PWD\"; end; \
functions -q fish_prompt; and functions -c fish_prompt __kapollo_user_prompt; \
function fish_prompt; \
functions -q __kapollo_user_prompt; and __kapollo_user_prompt; \
printf '\\e]133;B\\e\\\\'; end";

/// bash integration sourced from a generated rcfile. Preserves the user's
/// `~/.bashrc`, then arranges OSC 133 `C` via `PS0` (printed after a command is
/// read, before it runs) and `D;<exit>` plus OSC 7 (`file://host/cwd`) via
/// `PROMPT_COMMAND` (run before each prompt, capturing the just-finished
/// command's status and reporting the working directory; FR-019). The prompt
/// itself is bracketed by OSC 133 `A`/`B` marks wrapped around `PS1` (in
/// `\[`/`\]` so readline's width math is untouched), re-applied at the END of
/// each `PROMPT_COMMAND` cycle so a prompt manager that rewrites `PS1` every
/// prompt (starship et al.) cannot strip them (kwi #47).
const BASH_HOOK: &str = "\
[ -f ~/.bashrc ] && . ~/.bashrc
__kapollo_cmd_end() { local __kapollo_e=$?; printf '\\e]133;D;%s\\e\\\\' \"$__kapollo_e\"; printf '\\e]7;file://%s%s\\e\\\\' \"${HOSTNAME:-localhost}\" \"$PWD\"; }
__kapollo_prompt_marks() { case \"$PS1\" in *'133;A'*) ;; *) PS1=\"\\[\\e]133;A\\e\\\\\\\\\\]$PS1\\[\\e]133;B\\e\\\\\\\\\\]\" ;; esac }
PS0=$'\\e]133;C\\e\\\\'
case \"${PROMPT_COMMAND:-}\" in
  *__kapollo_cmd_end*) ;;
  *) PROMPT_COMMAND=\"__kapollo_cmd_end${PROMPT_COMMAND:+; $PROMPT_COMMAND}; __kapollo_prompt_marks\" ;;
esac
";

/// A generated bash rcfile that is removed when the session ends.
#[derive(Debug)]
pub struct BashRcGuard {
    path: PathBuf,
}

impl Drop for BashRcGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Resolve the shell to wrap: explicit override, then `$SHELL`, then `/bin/sh`.
pub fn resolve_shell(shell_override: Option<&str>) -> String {
    shell_override
        .map(str::to_owned)
        .or_else(|| std::env::var("SHELL").ok())
        .unwrap_or_else(|| "/bin/sh".to_string())
}

/// Build the [`CommandBuilder`] for the wrapped shell, installing the OSC 133
/// hook for fish/bash. Returns an optional rcfile guard that must be kept alive
/// (and dropped to clean up) for bash.
pub fn build_command(
    shell_path: &str,
    kind: ShellKind,
    mode: BoundaryMode,
) -> anyhow::Result<(CommandBuilder, Option<BashRcGuard>)> {
    let mut cmd = CommandBuilder::new(shell_path);
    cmd.env("KAPOLLO_ACTIVE", "1");
    cmd.env("KAPOLLO_VERSION", env!("CARGO_PKG_VERSION"));
    if let Ok(cwd) = std::env::current_dir() {
        cmd.cwd(cwd.as_os_str());
    }

    let mut rc_guard = None;
    if mode == BoundaryMode::Osc133 {
        match kind {
            ShellKind::Fish => {
                cmd.arg("--init-command");
                cmd.arg(FISH_HOOK);
            }
            ShellKind::Bash => {
                let path =
                    write_bash_rcfile().context("failed to write bash integration rcfile")?;
                cmd.arg("--rcfile");
                cmd.arg(path.as_os_str());
                cmd.arg("-i");
                rc_guard = Some(BashRcGuard { path });
            }
            ShellKind::Other => {}
        }
    }

    Ok((cmd, rc_guard))
}

fn write_bash_rcfile() -> std::io::Result<PathBuf> {
    let mut path = std::env::temp_dir();
    let unique = format!("kapollo-bashrc-{}-{}", std::process::id(), nonce_suffix());
    path.push(unique);
    std::fs::write(&path, BASH_HOOK)?;
    Ok(path)
}

fn nonce_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}
