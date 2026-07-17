#!/bin/sh
set -e

WSL_REPO="/root/yourls-tray-app"

echo "=== WSL: Initializing dependencies ==="

apk add build-base pkgconfig gtk+3.0-dev libayatana-appindicator-dev xdotool-dev rustup gcompat curl tar xz

# Setup rustup natively
if [ ! -f /root/.cargo/bin/rustc ]; then
  rm -rf /root/.rustup /root/.cargo
  rustup-init -y --default-toolchain stable --profile minimal
fi

# Always source the cargo env to make cargo available
. "$HOME/.cargo/env"
export PATH="$HOME/.cargo/bin:$PATH"

export PKG_CONFIG_ALLOW_CROSS=0

# Remove stale cross-compilation cargo config if present
rm -f "$WSL_REPO/.cargo/config.toml"

ARCH="$(uname -m)"

echo "=== WSL: Compiling native binary ($ARCH) ==="
cd "$WSL_REPO"
RUSTFLAGS="-C target-feature=-crt-static" CARGO_BUILD_JOBS=5 cargo build --release

# Package AppImage
echo "=== WSL: Packaging $ARCH AppImage ==="
APPDIR="$WSL_REPO/AppDir-$ARCH"
APPIMAGE_OUT="$WSL_REPO/yourls-tray-app-$ARCH.AppImage"
APPTOOL="appimagetool-$ARCH.AppImage"

mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/icons/hicolor/256x256/apps"
cp "$WSL_REPO/target/release/yourls-tray-app" "$APPDIR/usr/bin/yourls-tray-app"
cp "$WSL_REPO/src/icon.png" "$APPDIR/yourls-tray-app.png"
cp "$WSL_REPO/src/icon.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/yourls-tray-app.png"

cat << 'DESKTOP' > "$APPDIR/yourls-tray-app.desktop"
[Desktop Entry]
Name=YOURLS Shortener
Exec=yourls-tray-app
Icon=yourls-tray-app
Type=Application
Categories=Utility;
Terminal=false
Comment=Shorten links from clipboard automatically
DESKTOP

cat << 'APPRUN' > "$APPDIR/AppRun"
#!/bin/sh
SELF=$(readlink -f "$0")
HERE=$(dirname "$SELF")
exec "$HERE/usr/bin/yourls-tray-app" "$@"
APPRUN
chmod +x "$APPDIR/AppRun"

if [ ! -f "$WSL_REPO/$APPTOOL" ]; then
  curl -L -o "$WSL_REPO/$APPTOOL" "https://github.com/AppImage/appimagetool/releases/download/continuous/$APPTOOL"
  chmod +x "$WSL_REPO/$APPTOOL"
fi

SQUASH_DIR="$WSL_REPO/squashfs-root-$ARCH"
if [ ! -d "$SQUASH_DIR" ]; then
  cd "$WSL_REPO"
  "./$APPTOOL" --appimage-extract
  mv squashfs-root "$SQUASH_DIR"
fi

ARCH="$ARCH" "$SQUASH_DIR/AppRun" "$APPDIR" "$APPIMAGE_OUT"
