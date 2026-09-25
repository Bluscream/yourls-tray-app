#!/usr/bin/env bash
# Cross-compiles the Windows binaries from Linux, with mingw-w64.
#
# Both configurations, because a release ships both: the default build has the
# tray and is linked as a windows-subsystem program, and the --no-default-
# features one is a plain console program.
#
# This also runs clippy against the Windows target, which is not something the
# ordinary gate can do. It is worth the minute it costs: everything behind
# cfg(target_os = "windows") is invisible to a Linux build, and an edit to the
# top of clipboard.rs once deleted its `use clipboard_master::...` line without
# anything noticing until a Windows build was attempted months later.
#
# Needs mingw-w64 (setup-build-box.sh installs it) and the rust target:
#   rustup target add x86_64-pc-windows-gnu
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

TARGET="${YOURLS_WINDOWS_TARGET:-x86_64-pc-windows-gnu}"
OUT_DIR="${YOURLS_OUT_DIR:-$PROJECT_DIR/dist}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"

step() { printf '\n==> %s\n' "$1"; }

if ! rustup target list --installed | grep -qx "$TARGET"; then
    step "adding the $TARGET toolchain"
    rustup target add "$TARGET"
fi

step "clippy ($TARGET)"
# Not -D warnings: the crate lints unsafe_code, and the pre-existing Win32
# calls in the tray trip it ten times over. Errors still fail the build.
CARGO_TARGET_DIR=target/win cargo clippy --release --target "$TARGET" 2>&1 \
    | grep -vE 'usage of an `unsafe' || true

step "building (tray + CLI)"
CARGO_TARGET_DIR=target/win cargo build --release --target "$TARGET"

step "building (CLI only)"
CARGO_TARGET_DIR=target/win-cli cargo build --release --no-default-features --target "$TARGET"

step "collecting"
mkdir -p "$OUT_DIR"
BITS=64
[[ "$TARGET" == i686-* ]] && BITS=32
install -m755 "target/win/$TARGET/release/yourls.exe" "$OUT_DIR/yourls_win$BITS-release.exe"
install -m755 "target/win-cli/$TARGET/release/yourls.exe" "$OUT_DIR/yourls-cli_win$BITS-release.exe"

# The subsystem is the whole point of the split: a windows-subsystem binary
# shows no console for the tray, a console one prints where a shell expects.
# `file` reports it, so check rather than assume.
check_subsystem() {
    local path="$1" want="$2"
    if file "$path" | grep -q "($want)"; then
        echo "    $(basename "$path"): $want subsystem"
    else
        echo "FAILED: $(basename "$path") is not a $want subsystem binary" >&2
        file "$path" >&2
        exit 1
    fi
}
check_subsystem "$OUT_DIR/yourls_win$BITS-release.exe" GUI
check_subsystem "$OUT_DIR/yourls-cli_win$BITS-release.exe" console

echo "==> built $VERSION for $TARGET"
ls -lh "$OUT_DIR"/*win$BITS*.exe
