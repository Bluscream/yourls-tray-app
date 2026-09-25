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

# A target directory of its own for every CLI-only step. The two
# configurations differ only by feature flags, so they share target/release
# and whichever ran last wins — which silently left a CLI-only binary at
# target/release/yourls and made `--tray` report that it had no tray.
CLI_TARGET="$PROJECT_DIR/target/cli-only"

step "clippy (CLI only)"
# The tray is a default feature, so nothing above ever compiles the build that
# has it switched off. Without this, a tray-only item losing its #[cfg] shows
# up as dead code in a headless build that nobody runs until it breaks.
CARGO_TARGET_DIR="$CLI_TARGET" cargo clippy --all-targets --no-default-features -- -D warnings

step "tests"
cargo test --all-features

step "tests (CLI only)"
CARGO_TARGET_DIR="$CLI_TARGET" cargo test --no-default-features

step "release build"
cargo build --release --all-targets

step "release build (CLI only)"
# Proves the headless build still links: the point of the tray feature is that
# this one has no GTK, appindicator or libxdo in it at all.
CARGO_TARGET_DIR="$CLI_TARGET" cargo build --release --no-default-features

# The gate's own output is used for testing and deployment, so prove each
# binary is the build it claims to be rather than trusting the layout.
step "checking the two builds did not overwrite each other"
if ldd "$PROJECT_DIR/target/release/yourls" | grep -qiE 'gtk|appindicator'; then
    echo "    target/release/yourls has the tray, as it should"
else
    echo "FAILED: target/release/yourls has no GUI libraries; the CLI-only build overwrote it" >&2
    exit 1
fi
if ldd "$CLI_TARGET/release/yourls" | grep -qiE 'gtk|appindicator|libxdo'; then
    echo "FAILED: the CLI-only build links a GUI library" >&2
    exit 1
fi
echo "    $CLI_TARGET/release/yourls is GUI-free"

step "gate passed"
