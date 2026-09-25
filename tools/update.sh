#!/bin/sh
# The Linux half of the release, run inside WSL by tools/update.ps1.
#
# This used to build on Alpine and hand-roll an AppDir. Both were wrong, and
# every Linux asset released that way was unrunnable on an ordinary desktop:
#
#   * Alpine plus `-C target-feature=-crt-static` yields a dynamically linked
#     *musl* binary, which needs libc.musl-x86_64.so.1 — a file no glibc
#     distribution has. It failed with "required file not found" before
#     reaching main().
#   * The AppDir got the binary, the icon, a .desktop file and AppRun, and no
#     libraries whatsoever, so it also referenced Alpine's libxdo.so.4 while
#     Debian and Fedora still ship libxdo.so.3.
#
# So the build now happens on a glibc distribution and the bundling is left to
# scripts/appimage.sh, which uses linuxdeploy and verifies the result. Run this
# from a Debian/Ubuntu WSL distro, not Alpine.
set -e

REPO="${WSL_REPO:-$HOME/yourls-tray-app}"
cd "$REPO"

if ldd --version 2>&1 | grep -qi musl; then
    echo "refusing to build on musl: the result cannot run on a glibc desktop." >&2
    echo "Use a Debian or Ubuntu WSL distro (see WslDistroX64 in tools/update.ps1)." >&2
    exit 1
fi

echo "=== installing build dependencies ==="
sudo apt-get update -qq
# The tray pulls in GTK 3 and libayatana-appindicator; enigo links -lxdo.
sudo apt-get install -y -qq --no-install-recommends \
    build-essential pkg-config curl ca-certificates perl make file \
    libgtk-3-dev libayatana-appindicator3-dev libxdo-dev \
    libx11-dev libxext-dev

if [ ! -x "$HOME/.cargo/bin/cargo" ]; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain stable
fi
. "$HOME/.cargo/env"
export PATH="$HOME/.cargo/bin:$PATH"

# A stale cross-compilation config makes pkg-config look in the wrong sysroot.
rm -f "$REPO/.cargo/config.toml"

echo "=== compiling ($(uname -m)) ==="
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-5}" cargo build --release --bin yourls

echo "=== packaging the AppImage ==="
# The binary is already built for this glibc, so packaging only — appimage.sh
# would otherwise start a container of its own, which WSL cannot nest.
YOURLS_SKIP_BUILD=1 \
YOURLS_BIN_DIR="$REPO/target/release" \
YOURLS_OUT_DIR="$REPO/dist" \
    ./scripts/appimage.sh --skip-build
