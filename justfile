# Windows: `just` runs recipes through `sh`, which Windows does not ship — put
# Git for Windows' `usr\bin` on PATH (it holds `sh.exe`) or run from Git Bash.
# (Upstream's own requirement: "sh must be available in the PATH".)

# List available recipes
default:
    @just --list

# Run CI gates — mirrors .github/workflows/ci.yml exactly. Needs `fish` on PATH
# for tests/shell_parity.rs; CI apt-installs it for the same reason.
check:
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test

# Apply formatting
fmt:
    cargo fmt
