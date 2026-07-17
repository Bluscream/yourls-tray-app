#!/bin/sh
set -e

# WSL Repo Paths
WSL_REPO="/root/yourls-tray-app"
export PATH="/root/.cargo/bin:/usr/local/bin:$PATH"

echo "=== WSL: Initializing toolchain and multiarch dependencies ==="

# Alpine does not have native multiarch. Instead, to build 32-bit (i686) binaries with GTK
# dependencies, we must compile them using the native i686 musl environment via chroot, 
# or download pre-compiled i686 development libraries. 
# Alternatively, since we have the i686-linux-musl toolchain, we can tell pkg-config 
# to allow cross compiling and fallback to system libraries where ABI matches.
export PKG_CONFIG_ALLOW_CROSS=1

# Compile Linux x64 binary
echo "=== WSL: Compiling Linux x64 binary ==="
cd "$WSL_REPO"
RUSTFLAGS="-C target-feature=-crt-static" CARGO_BUILD_JOBS=20 cargo build --release

# Compile Linux i686 binary
echo "=== WSL: Compiling Linux i686 binary ==="
# We must point cargo to use the correct i686 linker, and use pkg-config to locate i686 libraries
# Alpine standard paths for i686 (if installed via lib32 compatibility packages)
export PKG_CONFIG_PATH="/usr/lib32/pkgconfig:/usr/share/pkgconfig"
rustup target add i686-unknown-linux-musl || true
RUSTFLAGS="-C target-feature=-crt-static" CARGO_BUILD_JOBS=20 cargo build --release --target i686-unknown-linux-musl

# Package Linux x64 AppImage
echo "=== WSL: Packaging Linux x64 AppImage ==="
cd "$WSL_REPO"
mkdir -p AppDir64/usr/bin AppDir64/usr/share/icons/hicolor/256x256/apps
cp target/release/yourls-tray-app AppDir64/usr/bin/yourls-tray-app
cp src/icon.png AppDir64/yourls-tray-app.png
cp src/icon.png AppDir64/usr/share/icons/hicolor/256x256/apps/yourls-tray-app.png

cat << 'EOF' > AppDir64/yourls-tray-app.desktop
[Desktop Entry]
Name=YOURLS Shortener
Exec=yourls-tray-app
Icon=yourls-tray-app
Type=Application
Categories=Utility;
Terminal=false
Comment=Shorten links from clipboard automatically
EOF

cat << 'EOF' > AppDir64/AppRun
#!/bin/sh
SELF=$(readlink -f "$0")
HERE=$(dirname "$SELF")
exec "$HERE/usr/bin/yourls-tray-app" "$@"
EOF
chmod +x AppDir64/AppRun

if [ ! -f appimagetool-x86_64.AppImage ]; then
  curl -L -o appimagetool-x86_64.AppImage https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x appimagetool-x86_64.AppImage
fi
if [ ! -d squashfs-root-x86_64 ]; then
  ./appimagetool-x86_64.AppImage --appimage-extract
  mv squashfs-root squashfs-root-x86_64
fi
ARCH=x86_64 ./squashfs-root-x86_64/AppRun AppDir64 yourls-tray-app-x86_64.AppImage


# Package Linux i686 AppImage
echo "=== WSL: Packaging Linux i686 AppImage ==="
mkdir -p AppDir32/usr/bin AppDir32/usr/share/icons/hicolor/256x256/apps
cp target/i686-unknown-linux-musl/release/yourls-tray-app AppDir32/usr/bin/yourls-tray-app
cp src/icon.png AppDir32/yourls-tray-app.png
cp src/icon.png AppDir32/usr/share/icons/hicolor/256x256/apps/yourls-tray-app.png

cat << 'EOF' > AppDir32/yourls-tray-app.desktop
[Desktop Entry]
Name=YOURLS Shortener
Exec=yourls-tray-app
Icon=yourls-tray-app
Type=Application
Categories=Utility;
Terminal=false
Comment=Shorten links from clipboard automatically
EOF

cat << 'EOF' > AppDir32/AppRun
#!/bin/sh
SELF=$(readlink -f "$0")
HERE=$(dirname "$SELF")
exec "$HERE/usr/bin/yourls-tray-app" "$@"
EOF
chmod +x AppDir32/AppRun

if [ ! -f appimagetool-i686.AppImage ]; then
  curl -L -o appimagetool-i686.AppImage https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-i686.AppImage
  chmod +x appimagetool-i686.AppImage
fi
if [ ! -d squashfs-root-i686 ]; then
  ./appimagetool-i686.AppImage --appimage-extract
  mv squashfs-root squashfs-root-i686
fi
ARCH=i686 ./squashfs-root-i686/AppRun AppDir32 yourls-tray-app-i686.AppImage
