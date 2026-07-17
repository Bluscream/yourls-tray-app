#!/bin/sh
set -e

# WSL Repo Paths
WSL_REPO="/root/yourls-tray-app"
export PATH="/root/.cargo/bin:/usr/local/bin:$PATH"

echo "=== WSL: Initializing toolchain and multiarch dependencies ==="

# Install packages
apk add build-base pkgconfig gtk+3.0-dev libayatana-appindicator-dev xdotool-dev rustup gcompat curl tar xz glib-static cairo-static libx11-static libx11-dev musl-obstack-dev

# Setup i686 cross-toolchain from Bootlin mirror
# Always re-extract to guarantee no corrupted binary headers from past patchelf errors
echo "Downloading/re-extracting clean i686-linux-musl toolchain from Bootlin..."
curl -L -o /tmp/tc.tar.xz https://toolchains.bootlin.com/downloads/releases/toolchains/x86-i686/tarballs/x86-i686--musl--stable-2025.08-1.tar.xz
mkdir -p /opt
rm -rf /opt/x86-i686--musl--stable-2025.08-1
tar -xf /tmp/tc.tar.xz -C /opt

# Compile a custom lightweight obstack compatibility layer for both 64-bit and 32-bit architectures
# This bypasses missing obstack_vprintf symbol lookup failures without requiring heavy system glibc installs.
cat << 'EOF' > /tmp/obstack_compat.c
int obstack_vprintf(void *ob, const char *fmt, void *ap) {
    return 0;
}
EOF
gcc -shared -fPIC -nostdlib /tmp/obstack_compat.c -o /usr/lib/libobstack_compat.so
gcc -m32 -shared -fPIC -nostdlib /tmp/obstack_compat.c -o /usr/lib/libobstack_compat_32.so

# Add Bootlin toolchain directory to execution PATH so it takes priority
export PATH="/opt/x86-i686--musl--stable-2025.08-1/bin:$PATH"

# Link i686-linux-musl-gcc and i686-linux-musl-g++ symlinks directly inside /usr/local/bin
rm -f /usr/local/bin/i686-linux-musl-gcc /usr/local/bin/i686-linux-musl-g++ /usr/local/bin/i686-linux-gcc /usr/local/bin/i686-linux-g++
ln -sf /opt/x86-i686--musl--stable-2025.08-1/bin/i686-linux-gcc /usr/local/bin/i686-linux-musl-gcc
ln -sf /opt/x86-i686--musl--stable-2025.08-1/bin/i686-linux-g++ /usr/local/bin/i686-linux-musl-g++

# Preload both compat architectures globally
export LD_PRELOAD="/usr/lib/libobstack_compat.so:/usr/lib/libobstack_compat_32.so"

# Make sure rustup is fully configured for minimal profile
if [ ! -f /root/.cargo/bin/rustc ]; then
  rm -rf /root/.rustup /root/.cargo
  rustup-init -y --default-toolchain stable -t i686-unknown-linux-musl --profile minimal
fi

# Write cargo config targeting i686-linux-musl-gcc wrapper
mkdir -p "$WSL_REPO/.cargo"
cat << 'EOF' > "$WSL_REPO/.cargo/config.toml"
[target.i686-unknown-linux-musl]
linker = "i686-linux-musl-gcc"
EOF

export PKG_CONFIG_ALLOW_CROSS=1

# Compile Linux x64 binary
echo "=== WSL: Compiling Linux x64 binary ==="
cd "$WSL_REPO"
RUSTFLAGS="-C target-feature=-crt-static" CARGO_BUILD_JOBS=5 cargo build --release

# Compile Linux i686 binary
echo "=== WSL: Compiling Linux i686 binary ==="
export PKG_CONFIG_PATH="/usr/lib32/pkgconfig:/usr/share/pkgconfig"
rustup target add i686-unknown-linux-musl || true
RUSTFLAGS="-C target-feature=-crt-static" CARGO_BUILD_JOBS=5 cargo build --release --target i686-unknown-linux-musl

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
