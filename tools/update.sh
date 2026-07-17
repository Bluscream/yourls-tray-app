#!/bin/sh
set -e

# WSL Repo Paths
WSL_REPO="/root/yourls-tray-app"
export PATH="/root/.cargo/bin:/usr/local/bin:$PATH"

echo "=== WSL: Initializing toolchain and multiarch dependencies ==="

# Install packages
apk add build-base pkgconfig gtk+3.0-dev libayatana-appindicator-dev xdotool-dev rustup gcompat curl tar xz glib-static cairo-static libx11-static libx11-dev musl-obstack-dev

# Setup i686 cross-toolchain from Bootlin mirror
if [ ! -f /usr/local/bin/i686-linux-musl-gcc ]; then
  echo "Downloading i686-linux-musl toolchain from Bootlin..."
  curl -L -o /tmp/tc.tar.xz https://toolchains.bootlin.com/downloads/releases/toolchains/x86-i686/tarballs/x86-i686--musl--stable-2025.08-1.tar.xz
  tar -xf /tmp/tc.tar.xz -C /opt
fi

# Install patchelf package to modify binary dependencies list
apk add patchelf

# Ensure Alpine can run the Bootlin binaries by patching the dynamic link interpreters to native musl
patchelf --set-interpreter /lib/ld-musl-x86_64.so.1 /opt/x86-i686--musl--stable-2025.08-1/bin/toolchain-wrapper || true
patchelf --set-interpreter /lib/ld-musl-x86_64.so.1 /opt/x86-i686--musl--stable-2025.08-1/bin/i686-buildroot-linux-musl-gcc.br_real || true
patchelf --set-interpreter /lib/ld-musl-x86_64.so.1 /opt/x86-i686--musl--stable-2025.08-1/bin/i686-buildroot-linux-musl-g++.br_real || true

# Add libobstack.so.1 natively to dynamic dependencies lists
patchelf --add-needed /usr/lib/libobstack.so.1 /opt/x86-i686--musl--stable-2025.08-1/bin/toolchain-wrapper || true
patchelf --add-needed /usr/lib/libobstack.so.1 /opt/x86-i686--musl--stable-2025.08-1/bin/i686-buildroot-linux-musl-gcc.br_real || true
patchelf --add-needed /usr/lib/libobstack.so.1 /opt/x86-i686--musl--stable-2025.08-1/bin/i686-buildroot-linux-musl-g++.br_real || true

# Patch the toolchain's shared libraries to load libobstack as well
patchelf --add-needed /usr/lib/libobstack.so.1 /opt/x86-i686--musl--stable-2025.08-1/lib/libgmp.so.10 || true
patchelf --add-needed /usr/lib/libobstack.so.1 /opt/x86-i686--musl--stable-2025.08-1/lib/libmpfr.so.6 || true
patchelf --add-needed /usr/lib/libobstack.so.1 /opt/x86-i686--musl--stable-2025.08-1/lib/libmpc.so.3 || true

# Link i686-linux-musl-gcc and i686-linux-musl-g++ symlinks directly so Cargo can call them by their expected names
rm -f /usr/local/bin/i686-linux-musl-gcc /usr/local/bin/i686-linux-musl-g++ /usr/local/bin/i686-linux-gcc /usr/local/bin/i686-linux-g++
ln -sf /opt/x86-i686--musl--stable-2025.08-1/bin/i686-linux-gcc /usr/local/bin/i686-linux-musl-gcc
ln -sf /opt/x86-i686--musl--stable-2025.08-1/bin/i686-linux-g++ /usr/local/bin/i686-linux-musl-g++

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
