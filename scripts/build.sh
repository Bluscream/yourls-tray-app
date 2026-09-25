#!/usr/bin/env bash
# The verification gate. Everything must pass before anything is committed or
# released.
#
# Run it as `./scripts/build.sh` and check its exit status directly:
#
#     ./scripts/build.sh > /tmp/gate.log 2>&1; echo $?
#
# Never pipe it into grep and test *that* — `cmd | grep ok && git commit`
# reports grep's status, not the gate's, so a failing build commits anyway.
# This has actually happened on a sibling project; hence the warning.
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

step() { printf '\n==> %s\n' "$1"; }

step "formatting"
cargo fmt --all -- --check

step "clippy"
# --all-targets covers tests and benches too, which is where unwrap_used
# violations otherwise hide. Warnings are errors: a lint nobody fixes is a
# lint nobody reads.
cargo clippy --all-targets --all-features -- -D warnings

step "clippy (CLI only)"
# The tray is a default feature, so nothing above ever compiles the build that
# has it switched off. Without this, a tray-only item losing its #[cfg] shows
# up as dead code in a headless build that nobody runs until it breaks.
cargo clippy --all-targets --no-default-features -- -D warnings

step "tests"
cargo test --all-features

step "tests (CLI only)"
cargo test --no-default-features

step "release build"
cargo build --release --all-targets

step "release build (CLI only)"
# Proves the headless build still links: the point of the tray feature is that
# this one has no GTK, appindicator or libxdo in it at all.
cargo build --release --no-default-features

step "gate passed"
