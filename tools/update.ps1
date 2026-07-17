param(
    [string]$Version,
    [string]$CommitMessage
)

$ErrorActionPreference = "Stop"

# 1. Parse current version from Cargo.toml if not passed as an argument
$cargoContent = Get-Content -Path "Cargo.toml" -Raw
$versionRegex = '(?m)^version\s*=\s*"([^"]+)"'
if ($cargoContent -match $versionRegex) {
    $currentVersion = $Matches[1]
} else {
    Write-Error "Could not parse current version from Cargo.toml"
    exit 1
}

if (-not $Version) {
    $Version = Read-Host "Current version is $currentVersion. Enter new version"
}

if (-not $Version) {
    Write-Error "Version cannot be empty."
    exit 1
}

if (-not $CommitMessage) {
    $CommitMessage = Read-Host "Enter commit/release message (optional)"
}
if (-not $CommitMessage) {
    $CommitMessage = "Release v$Version"
}

# Update Cargo.toml with the new version
Write-Host "Updating Cargo.toml version to $Version..." -ForegroundColor Cyan
$cargoContent = $cargoContent -replace '(?m)^version\s*=\s*"[^"]+"', "version = `"$Version`""
Set-Content -Path "Cargo.toml" -Value $cargoContent -NoNewline

# 2. Ensure Rust compilation targets are installed
Write-Host "Ensuring Rust compilation targets are installed..." -ForegroundColor Cyan
rustup target add x86_64-pc-windows-msvc
rustup target add i686-pc-windows-msvc

# 3. Compile Windows Binaries
Write-Host "Building Windows x64 release binary..." -ForegroundColor Cyan
cargo build --release --target x86_64-pc-windows-msvc

Write-Host "Building Windows x86 release binary..." -ForegroundColor Cyan
cargo build --release --target i686-pc-windows-msvc

# 4. Verify WSL Alpine setup
Write-Host "Verifying WSL Alpine Linux distribution..." -ForegroundColor Cyan
$wslDistros = wsl.exe -l -v | Out-String
$wslDistros = $wslDistros -replace "\x00", ""
if ($wslDistros -notmatch "Alpine") {
    Write-Host "Alpine WSL distro not found. Setting it up automatically..." -ForegroundColor Yellow
    $url = "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/alpine-minirootfs-3.24.0-x86_64.tar.gz"
    $downloadPath = "$env:TEMP\alpine-minirootfs.tar.gz"
    Write-Host "Downloading $url..."
    Invoke-WebRequest -Uri $url -OutFile $downloadPath -UseBasicParsing
    New-Item -ItemType Directory -Force -Path "C:\WSL\Alpine" | Out-Null
    wsl --import Alpine C:\WSL\Alpine $downloadPath
}

Write-Host "Installing Linux build dependencies inside WSL Alpine..." -ForegroundColor Cyan
wsl -d Alpine apk add build-base pkgconfig gtk+3.0-dev libayatana-appindicator-dev xdotool-dev rust cargo gcompat curl

# 5. Sync workspace to WSL native filesystem
Write-Host "Syncing workspace files to WSL native filesystem..." -ForegroundColor Cyan
wsl -d Alpine sh -c "mkdir -p ~/yourls-tray-app"
wsl -d Alpine sh -c "rm -rf ~/yourls-tray-app/src"
wsl -d Alpine sh -c "cp -r '/mnt/d/Projects/Visual Studio/source/repos/Cargo.toml' '/mnt/d/Projects/Visual Studio/source/repos/Cargo.lock' '/mnt/d/Projects/Visual Studio/source/repos/src' ~/yourls-tray-app/"

# 6. Compile Linux Binary inside WSL
Write-Host "Compiling native Linux binary in WSL..." -ForegroundColor Cyan
wsl -d Alpine sh -c "cd ~/yourls-tray-app && CARGO_BUILD_JOBS=20 cargo build --release"

# 7. Package Linux AppImage inside WSL
Write-Host "Packaging Linux AppImage in WSL..." -ForegroundColor Cyan
$appImageScript = @'
cd ~/yourls-tray-app
mkdir -p AppDir/usr/bin
mkdir -p AppDir/usr/share/icons/hicolor/256x256/apps

# Copy compiled binary
cp target/release/yourls-tray-app AppDir/usr/bin/yourls-tray-app

# Copy app icon
cp src/icon.png AppDir/yourls-tray-app.png
cp src/icon.png AppDir/usr/share/icons/hicolor/256x256/apps/yourls-tray-app.png

# Create desktop description file
cat << 'EOF' > AppDir/yourls-tray-app.desktop
[Desktop Entry]
Name=YOURLS Shortener
Exec=yourls-tray-app
Icon=yourls-tray-app
Type=Application
Categories=Utility;
Terminal=false
Comment=Shorten links from clipboard automatically
EOF

# Create launcher AppRun wrapper
cat << 'EOF' > AppDir/AppRun
#!/bin/sh
SELF=$(readlink -f "$0")
HERE=$(dirname "$SELF")
exec "$HERE/usr/bin/yourls-tray-app" "$@"
EOF
chmod +x AppDir/AppRun

# Download appimagetool if not exists
if [ ! -f appimagetool-x86_64.AppImage ]; then
  curl -L -o appimagetool-x86_64.AppImage https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x appimagetool-x86_64.AppImage
fi

# Extract appimagetool to bypass FUSE requirements inside WSL
if [ ! -d squashfs-root ]; then
  ./appimagetool-x86_64.AppImage --appimage-extract
fi

# Build AppImage
./squashfs-root/AppRun AppDir yourls-tray-app-x86_64.AppImage
'@.Replace("`r`n", "`n")

wsl -d Alpine sh -c $appImageScript

# 8. Copy Linux ELF + AppImage back to host Windows workspace
Write-Host "Copying compiled Linux binaries back to host..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path "target\release" | Out-Null
wsl -d Alpine cp ~/yourls-tray-app/target/release/yourls-tray-app "/mnt/d/Projects/Visual Studio/source/repos/target/release/yourls-tray-app_lin64-release"
wsl -d Alpine cp ~/yourls-tray-app/yourls-tray-app-x86_64.AppImage "/mnt/d/Projects/Visual Studio/source/repos/target/release/yourls-tray-app_lin64-release.AppImage"

# 9. Commit, Tag & Push to GitHub
Write-Host "Creating Git commit and tag v$Version..." -ForegroundColor Cyan
git add .
git commit -m "$CommitMessage"
git push origin main
git tag "v$Version"
git push origin "v$Version"

# 10. Copy and Rename Windows binaries for release packaging
Copy-Item -Path "target\x86_64-pc-windows-msvc\release\yourls-tray-app.exe" -Destination "target\release\yourls-tray-app_win64-release.exe" -Force
Copy-Item -Path "target\i686-pc-windows-msvc\release\yourls-tray-app.exe" -Destination "target\release\yourls-tray-app_win32-release.exe" -Force

# 11. Publish GitHub Release via gh CLI
Write-Host "Creating GitHub Release v$Version..." -ForegroundColor Cyan
$env:GITHUB_TOKEN=""

$repo = "Bluscream/yourls-tray-app"
$tagEncoded = "v$Version"
$badgeBase = "https://img.shields.io/github/downloads/$repo"
$badgeStyle = "style=flat-square"

$shieldTotal  = "[![Downloads](${badgeBase}/total?${badgeStyle}`&label=total+downloads)](https://github.com/${repo}/releases)"
$shieldWin64  = "[![](${badgeBase}/${tagEncoded}/yourls-tray-app_win64-release.exe?${badgeStyle}`&label=win64)](https://github.com/${repo}/releases/tag/${tagEncoded})"
$shieldWin32  = "[![](${badgeBase}/${tagEncoded}/yourls-tray-app_win32-release.exe?${badgeStyle}`&label=win32)](https://github.com/${repo}/releases/tag/${tagEncoded})"
$shieldLin64  = "[![](${badgeBase}/${tagEncoded}/yourls-tray-app_lin64-release?${badgeStyle}`&label=linux64)](https://github.com/${repo}/releases/tag/${tagEncoded})"
$shieldAppImg = "[![](${badgeBase}/${tagEncoded}/yourls-tray-app_lin64-release.AppImage?${badgeStyle}`&label=AppImage)](https://github.com/${repo}/releases/tag/${tagEncoded})"

$notes = @"
### Release v$Version  $shieldTotal

$CommitMessage

#### Compiled Binaries:
*   ``yourls-tray-app_win64-release.exe`` — Windows 64-bit  $shieldWin64
*   ``yourls-tray-app_win32-release.exe`` — Windows 32-bit  $shieldWin32
*   ``yourls-tray-app_lin64-release`` — Native Linux 64-bit (musl static)  $shieldLin64
*   ``yourls-tray-app_lin64-release.AppImage`` — Standalone Linux AppImage  $shieldAppImg

#### Linux Dependency Installation Notes
To run the Linux binary natively, please ensure the following dependencies are installed on your system (depending on your distribution):
* **Wayland Clipboard Support**: ``wl-clipboard`` (provides ``wl-copy`` and ``wl-paste``)
* **Keyboard Bypass Features**: ``xdotool`` (provides the ``libxdo.so.4`` shared library required for the Shift-key bypass)

**Ubuntu / Debian (natively)**:
``````bash
sudo apt install wl-clipboard xdotool
``````

**Arch Linux (natively)**:
``````bash
sudo pacman -S wl-clipboard xdotool
``````
"@

$notesFile = "$env:TEMP\release_notes_v$Version.md"
Set-Content -Path $notesFile -Value $notes -NoNewline

gh release create "v$Version" --title "v$Version" --notes-file $notesFile `
  "target\release\yourls-tray-app_win64-release.exe" `
  "target\release\yourls-tray-app_win32-release.exe" `
  "target\release\yourls-tray-app_lin64-release" `
  "target\release\yourls-tray-app_lin64-release.AppImage"

if ($LASTEXITCODE -eq 0) {
    Write-Host "Successfully created and published Release v$Version!" -ForegroundColor Green
} else {
    Write-Error "Failed to publish release on GitHub."
}
