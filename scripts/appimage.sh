#!/usr/bin/env bash
# Builds a portable AppImage of the yourls binary.
#
# This replaces tools/update.sh, which produced an AppImage that could not run
# on any ordinary desktop. Two separate reasons, both worth recording so they
# do not come back:
#
#   1. It built on Alpine with `-C target-feature=-crt-static`, i.e. a
#      *dynamically linked musl* binary. Such a binary needs
#      libc.musl-x86_64.so.1 at runtime, which no glibc distribution has, so
#      it died with "required file not found" before reaching main().
#
#   2. It bundled nothing at all. The AppDir only ever received the binary,
#      the icon, the .desktop file and AppRun — no libraries — so even the
#      glibc case would have failed on Alpine's libxdo.so.4, which Fedora and
#      Debian do not ship (they are still on libxdo.so.3).
#
# So: build against an old glibc, and bundle the GTK/tray stack with
# linuxdeploy. Ubuntu 20.04 has glibc 2.31 — a binary linked against 2.31
# runs on anything newer, while one linked against the host's 2.43 would run
# only here.
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_IMAGE="${YOURLS_BUILD_IMAGE:-docker.io/library/ubuntu:20.04}"
# podman locally, docker on a CI runner: identical arguments for what is used
# here.
RUNTIME="${YOURLS_CONTAINER_RUNTIME:-podman}"
OUT_DIR="${YOURLS_OUT_DIR:-$PROJECT_DIR/dist}"
ARCH="${ARCH:-x86_64}"
SKIP_BUILD="${YOURLS_SKIP_BUILD:-0}"
# A target directory of its own: the host's target/ holds objects linked
# against the host's glibc, and mixing the two silently yields a binary that
# runs nowhere else.
BIN_DIR="${YOURLS_BIN_DIR:-$PROJECT_DIR/target/appimage/release}"

usage() {
    cat <<'USAGE'
Usage: appimage.sh [--skip-build]

  --skip-build   Package a binary that is already in target/appimage/release
                 instead of compiling one.

Environment:
  YOURLS_BUILD_IMAGE        image to build in (default: ubuntu:20.04)
  YOURLS_CONTAINER_RUNTIME  podman (default) or docker
  YOURLS_OUT_DIR            where to write the AppImage (default: ./dist)
  YOURLS_BIN_DIR            where the binary is
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-build) SKIP_BUILD=1 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
    shift
done

cd "$PROJECT_DIR"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"

if [[ $SKIP_BUILD -eq 0 ]]; then
    echo "==> building in $BUILD_IMAGE"
    mkdir -p "$PROJECT_DIR/target/appimage" "$PROJECT_DIR/target/appimage-cargo"
    "$RUNTIME" run --rm \
        -v "$PROJECT_DIR:/src:z" \
        -v "$PROJECT_DIR/target/appimage-cargo:/cargo:z" \
        -e CARGO_HOME=/cargo \
        -e RUSTUP_HOME=/cargo/rustup \
        -e CARGO_TARGET_DIR=/src/target/appimage \
        -w /src \
        "$BUILD_IMAGE" \
        bash -euc '
            export DEBIAN_FRONTEND=noninteractive
            apt-get update -qq
            # The tray needs the whole GTK/appindicator stack, and enigo
            # links -lxdo. libssl-dev is deliberately absent: OpenSSL is
            # vendored (see Cargo.toml), so it is compiled from source here
            # and the result depends on no system libssl at all. perl and
            # make, above, are what that build needs.
            apt-get install -y -qq --no-install-recommends \
                build-essential curl ca-certificates pkg-config perl make file \
                libgtk-3-dev libayatana-appindicator3-dev libxdo-dev \
                libx11-dev libxext-dev >/dev/null
            # RUSTUP_HOME lives on the cached volume as well: without it the
            # toolchain is discarded with the container and the next run finds
            # a cargo shim with no toolchain behind it.
            if [ ! -x /cargo/bin/cargo ] || [ ! -d /cargo/rustup/toolchains ]; then
                curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs \
                    | sh -s -- -y --profile minimal --default-toolchain stable >/dev/null
            fi
            export PATH=/cargo/bin:$PATH
            cargo build --release --bin yourls
            # The headless build, released alongside the tray one. A separate
            # target directory because the two differ only by feature flags,
            # and cargo would otherwise rebuild over the top of each other.
            CARGO_TARGET_DIR=/src/target/appimage-cli \
                cargo build --release --no-default-features --bin yourls
        '
fi

if [[ ! -x "$BIN_DIR/yourls" ]]; then
    echo "missing $BIN_DIR/yourls — build first, or point YOURLS_BIN_DIR at it" >&2
    exit 1
fi

echo "==> assembling the AppDir"
APPDIR="$PROJECT_DIR/target/AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/256x256/apps"

install -m755 "$BIN_DIR/yourls" "$APPDIR/usr/bin/yourls"
strip "$APPDIR/usr/bin/yourls" 2>/dev/null || true
# src/icon.png is 1024x1024, and linuxdeploy rejects an icon whose real size
# does not match the directory it sits in. ImageMagick 7 renamed the tool, so
# both spellings are around.
if command -v magick >/dev/null; then RASTERISE=(magick); else RASTERISE=(convert); fi
"${RASTERISE[@]}" "$PROJECT_DIR/src/icon.png" -resize 256x256 \
    "$APPDIR/usr/share/icons/hicolor/256x256/apps/yourls.png"

cat > "$APPDIR/usr/share/applications/yourls.desktop" <<'DESKTOP'
[Desktop Entry]
Name=YOURLS Shortener
# The binary is a CLI first; the tray is opt-in behind --tray, so the desktop
# entry — which exists to launch a background tray — has to ask for it.
Exec=yourls --tray
Icon=yourls
Type=Application
Categories=Utility;
Terminal=false
Comment=Shorten links from the clipboard
DESKTOP

echo "==> bundling dependencies"
# linuxdeploy walks ldd and copies in everything that is not part of the
# excludelist of libraries which must come from the host (libc, libGL, the
# X11/Wayland client libraries). The gtk plugin additionally handles what ldd
# cannot see: gdk-pixbuf loaders, GTK modules and GObject typelibs, all loaded
# by path at runtime. Bundling GTK without it produces an AppImage that starts
# and then cannot render an icon.
TOOL_DIR="${YOURLS_TOOL_DIR:-$PROJECT_DIR/target/appimage-tools}"
mkdir -p "$TOOL_DIR"
fetch() {
    [[ -x "$TOOL_DIR/$1" ]] && return 0
    echo "    downloading $1"
    curl -fsSL -o "$TOOL_DIR/$1" "$2"
    chmod +x "$TOOL_DIR/$1"
}
fetch "linuxdeploy-$ARCH.AppImage" \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$ARCH.AppImage"
fetch "linuxdeploy-plugin-gtk.sh" \
    "https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh"

# --appimage-extract-and-run because FUSE is usually unavailable in a
# container and often restricted on an immutable host.
export APPIMAGE_EXTRACT_AND_RUN=1
export PATH="$TOOL_DIR:$PATH"
export DEPLOY_GTK_VERSION=3

mkdir -p "$OUT_DIR"
OUTPUT_PATH="$OUT_DIR/yourls-$VERSION-$ARCH.AppImage"
# Without OUTPUT, linuxdeploy names the file after the desktop entry and
# writes it to whatever it considers the working directory — which turned out
# to be $HOME, not here.
export OUTPUT="$OUTPUT_PATH"

"$TOOL_DIR/linuxdeploy-$ARCH.AppImage" \
    --appdir "$APPDIR" \
    --executable "$APPDIR/usr/bin/yourls" \
    --desktop-file "$APPDIR/usr/share/applications/yourls.desktop" \
    --icon-file "$APPDIR/usr/share/icons/hicolor/256x256/apps/yourls.png" \
    --plugin gtk \
    --output appimage

OUTPUT="$OUTPUT_PATH"
if [[ ! -f "$OUTPUT" ]]; then
    echo "linuxdeploy did not produce $OUTPUT" >&2
    exit 1
fi

echo "==> built $OUTPUT"
ls -lh "$OUTPUT"

# Both binaries ship: the AppImage and the plain tray build for anyone who
# does not want one, plus the CLI-only build for machines with no desktop.
echo "==> collecting the plain binaries"
install -m755 "$BIN_DIR/yourls" "$OUT_DIR/yourls-$VERSION-$ARCH"
strip "$OUT_DIR/yourls-$VERSION-$ARCH" 2>/dev/null || true

CLI_BIN="${YOURLS_CLI_BIN_DIR:-$PROJECT_DIR/target/appimage-cli/release}/yourls"
if [[ -x "$CLI_BIN" ]]; then
    install -m755 "$CLI_BIN" "$OUT_DIR/yourls-cli-$VERSION-$ARCH"
    strip "$OUT_DIR/yourls-cli-$VERSION-$ARCH" 2>/dev/null || true
    # The whole point of that build, so prove it rather than assume it.
    if ldd "$OUT_DIR/yourls-cli-$VERSION-$ARCH" | grep -qiE 'gtk|gdk|appindicator|libxdo'; then
        echo "FAILED: the CLI-only build still links a GUI library" >&2
        exit 1
    fi
    echo "    yourls-cli-$VERSION-$ARCH links $(ldd "$OUT_DIR/yourls-cli-$VERSION-$ARCH" | wc -l) libraries, none of them GUI"
else
    echo "    no CLI-only binary at $CLI_BIN; skipping" >&2
fi

# Fixed names as well as versioned ones. tools/update.ps1 copies these by
# exact name: a `yourls-*` glob would match the CLI build too and copy two
# files onto one destination.
echo "==> release names"
cp -f "$OUTPUT" "$OUT_DIR/yourls_lin64-release.AppImage"
cp -f "$OUT_DIR/yourls-$VERSION-$ARCH" "$OUT_DIR/yourls_lin64-release"
[[ -f "$OUT_DIR/yourls-cli-$VERSION-$ARCH" ]] && \
    cp -f "$OUT_DIR/yourls-cli-$VERSION-$ARCH" "$OUT_DIR/yourls-cli_lin64-release"
ls -1 "$OUT_DIR"

# A bundled AppImage that still needs something from the host is the exact bug
# this script exists to fix, so prove it before calling it done.
echo "==> checking nothing is left unresolved"
"$OUTPUT" --appimage-extract >/dev/null
if ldd squashfs-root/usr/bin/yourls 2>/dev/null | grep 'not found'; then
    echo "FAILED: the bundled binary still has unresolved libraries" >&2
    rm -rf squashfs-root
    exit 1
fi
rm -rf squashfs-root
echo "==> ok"
