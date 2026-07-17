param(
    [string]$Version,
    [string]$CommitMessage
)

$ErrorActionPreference = "Stop"

# ─────────────────────────────────────────────────────────────────────────────
# Constants
# ─────────────────────────────────────────────────────────────────────────────

$Repo         = "Bluscream/yourls-tray-app"
$WslDistro    = "Alpine"
$WslRepo      = "~/yourls-tray-app"
$HostTarget   = "target\release"
$WslTarget    = "/mnt/d/Projects/Visual Studio/source/repos/target/release"
$WslSrc       = "/mnt/d/Projects/Visual Studio/source/repos"
$BadgeBase    = "https://img.shields.io/github/downloads/$Repo"
$BadgeStyle   = "style=flat-square"

# ─────────────────────────────────────────────────────────────────────────────
# Release asset definitions
# ─────────────────────────────────────────────────────────────────────────────

$ReleaseAssets = @(
    @{ FileName = "yourls-tray-app_win64-release.exe";       Label = "win64";            Description = "Windows 64-bit" }
    @{ FileName = "yourls-tray-app_win32-release.exe";       Label = "win32";            Description = "Windows 32-bit" }
    @{ FileName = "yourls-tray-app_lin64-release";           Label = "linux64";          Description = "Linux 64-bit (musl static)" }
    @{ FileName = "yourls-tray-app_lin64-release.AppImage";  Label = "linux64-appimage"; Description = "Linux 64-bit AppImage" }
    @{ FileName = "yourls-tray-app_lin32-release";           Label = "linux32";          Description = "Linux 32-bit / i686 (musl static)" }
    @{ FileName = "yourls-tray-app_lin32-release.AppImage";  Label = "linux32-appimage"; Description = "Linux 32-bit AppImage" }
)

# ─────────────────────────────────────────────────────────────────────────────
# Helper functions
# ─────────────────────────────────────────────────────────────────────────────

function Step([string]$Msg) {
    Write-Host $Msg -ForegroundColor Cyan
}

function InvokeWsl([string]$Cmd) {
    wsl -d $WslDistro sh -c $Cmd
}

function Get-AssetShield([hashtable]$Asset, [string]$Tag) {
    $fileName = $Asset.FileName
    $label = $Asset.Label
    $url = "${BadgeBase}/${Tag}/${fileName}?${BadgeStyle}&label=${label}"
    return "[![]($url)](https://github.com/${Repo}/releases/tag/${Tag})"
}

function Get-TotalShield {
    return "[![Downloads](${BadgeBase}/total?${BadgeStyle}&label=total+downloads)](https://github.com/${Repo}/releases)"
}

function Get-AssetLine([hashtable]$Asset, [string]$Tag) {
    $shield = Get-AssetShield $Asset $Tag
    $fileName = $Asset.FileName
    $description = $Asset.Description
    # Constructing markdown line without using double backticks inside double quotes
    return "*   ```$fileName``` - $description  $shield"
}

function Build-ReleaseNotes([string]$Tag, [string]$ChangeLog) {
    $totalShield = Get-TotalShield
    $assetLines = @()
    foreach ($asset in $ReleaseAssets) {
        $assetLines += Get-AssetLine $asset $Tag
    }
    $assetLinesJoined = $assetLines -join "`r`n"

    $notes = @'
### Release {TAG}  {TOTAL_SHIELD}

{CHANGELOG}

#### Compiled Binaries:
{ASSET_LINES}

#### Linux Dependency Installation Notes
To run the Linux binary natively, please ensure the following dependencies are installed on your system (depending on your distribution):
* **Wayland Clipboard Support**: `wl-clipboard` (provides `wl-copy` and `wl-paste`)
* **Keyboard Bypass Features**: `xdotool` (provides the `libxdo.so.4` shared library required for the Shift-key bypass)

**Ubuntu / Debian (natively)**:
```bash
sudo apt install wl-clipboard xdotool
```

**Arch Linux (natively)**:
```bash
sudo pacman -S wl-clipboard xdotool
```
'@

    return $notes.Replace('{TAG}', $Tag).Replace('{TOTAL_SHIELD}', $totalShield).Replace('{CHANGELOG}', $ChangeLog).Replace('{ASSET_LINES}', $assetLinesJoined)
}

function Build-AppImage([string]$AppDir, [string]$BinarySrc, [string]$OutputFile, [string]$ToolArch) {
    $tool   = "appimagetool-${ToolArch}.AppImage"
    $sqroot = "squashfs-root-${ToolArch}"

    # Build the shell script using string replacement to avoid any parser errors with $ escaping in double quotes
    $scriptTemplate = @'
cd {WSL_REPO}
mkdir -p {APP_DIR}/usr/bin {APP_DIR}/usr/share/icons/hicolor/256x256/apps
cp {BINARY_SRC} {APP_DIR}/usr/bin/yourls-tray-app
cp src/icon.png {APP_DIR}/yourls-tray-app.png
cp src/icon.png {APP_DIR}/usr/share/icons/hicolor/256x256/apps/yourls-tray-app.png
printf '[Desktop Entry]\nName=YOURLS Shortener\nExec=yourls-tray-app\nIcon=yourls-tray-app\nType=Application\nCategories=Utility;\nTerminal=false\nComment=Shorten links from clipboard automatically\n' > {APP_DIR}/yourls-tray-app.desktop
printf '#!/bin/sh\nSELF=$(readlink -f "$0")\nHERE=$(dirname "$SELF")\nexec "$HERE/usr/bin/yourls-tray-app" "$@"\n' > {APP_DIR}/AppRun
chmod +x {APP_DIR}/AppRun
if [ ! -f {TOOL} ]; then curl -L -o {TOOL} https://github.com/AppImage/appimagetool/releases/download/continuous/{TOOL} && chmod +x {TOOL}; fi
if [ ! -d {SQROOT} ]; then ./{TOOL} --appimage-extract && mv squashfs-root {SQROOT}; fi
ARCH={ARCH_VAR} ./{SQROOT}/AppRun {APP_DIR} {OUTPUT_FILE}
'@

    $sh = $scriptTemplate.Replace('{WSL_REPO}', $WslRepo).Replace('{APP_DIR}', $AppDir).Replace('{BINARY_SRC}', $BinarySrc).Replace('{TOOL}', $tool).Replace('{SQROOT}', $sqroot).Replace('{OUTPUT_FILE}', $OutputFile).Replace('{ARCH_VAR}', $ToolArch)

    wsl -d $WslDistro sh -c $sh.Replace("`r`n", "`n")
}

# ─────────────────────────────────────────────────────────────────────────────
# 1. Resolve version
# ─────────────────────────────────────────────────────────────────────────────

$cargoContent = Get-Content -Path "Cargo.toml" -Raw
if ($cargoContent -match '(?m)^version\s*=\s*"([^"]+)"') {
    $currentVersion = $Matches[1]
} else {
    Write-Error "Could not parse version from Cargo.toml"; exit 1
}

if (-not $Version)       { $Version       = Read-Host "Current version is $currentVersion. Enter new version" }
if (-not $Version)       { Write-Error "Version cannot be empty."; exit 1 }
if (-not $CommitMessage) { $CommitMessage  = Read-Host "Enter commit/release message (optional)" }
if (-not $CommitMessage) { $CommitMessage  = "Release v$Version" }

$Tag = "v$Version"

# ─────────────────────────────────────────────────────────────────────────────
# 2. Bump Cargo.toml
# ─────────────────────────────────────────────────────────────────────────────

Step "Updating Cargo.toml version to $Version..."
$cargoContent = $cargoContent -replace '(?m)^version\s*=\s*"[^"]+"', "version = `"$Version`""
Set-Content -Path "Cargo.toml" -Value $cargoContent -NoNewline

# ─────────────────────────────────────────────────────────────────────────────
# 3. Build Windows binaries
# ─────────────────────────────────────────────────────────────────────────────

Step "Ensuring Rust Windows targets are installed..."
rustup target add x86_64-pc-windows-msvc
rustup target add i686-pc-windows-msvc

Step "Building Windows x64..."
cargo build --release --target x86_64-pc-windows-msvc

Step "Building Windows x86..."
cargo build --release --target i686-pc-windows-msvc

# ─────────────────────────────────────────────────────────────────────────────
# 4. Ensure WSL Alpine is available
# ─────────────────────────────────────────────────────────────────────────────

Step "Verifying WSL Alpine Linux distribution..."
$wslList = (wsl.exe -l -v | Out-String) -replace "\x00", ""
if ($wslList -notmatch "Alpine") {
    Write-Host "Alpine WSL distro not found - bootstrapping..." -ForegroundColor Yellow
    $tarball = "$env:TEMP\alpine-minirootfs.tar.gz"
    Invoke-WebRequest -Uri "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/alpine-minirootfs-3.24.0-x86_64.tar.gz" -OutFile $tarball -UseBasicParsing
    New-Item -ItemType Directory -Force -Path "C:\WSL\Alpine" | Out-Null
    wsl --import Alpine C:\WSL\Alpine $tarball
}

# ─────────────────────────────────────────────────────────────────────────────
# 5. Install Linux build dependencies
# ─────────────────────────────────────────────────────────────────────────────

Step "Installing Linux build dependencies in WSL Alpine..."
InvokeWsl "apk add build-base pkgconfig gtk+3.0-dev libayatana-appindicator-dev xdotool-dev rustup gcompat curl tar xz glib-static cairo-static libx11-static libx11-dev"

Step "Setting up i686-linux-musl cross-toolchain..."
# Download from Bootlin verified mirror for i686 musl toolchain
$setupToolchain = @'
if [ ! -f /usr/local/bin/i686-linux-musl-gcc ]; then
  echo "Downloading i686-linux-musl toolchain from Bootlin..."
  curl -L -o /tmp/tc.tar.xz https://toolchains.bootlin.com/downloads/releases/toolchains/x86-i686/tarballs/x86-i686--musl--stable-2025.08-1.tar.xz
  tar -xf /tmp/tc.tar.xz -C /opt
fi
# Recreate symlinks to ensure they are correct
rm -f /usr/local/bin/i686-linux-musl-gcc /usr/local/bin/i686-linux-musl-g++
ln -sf /opt/x86-i686--musl--stable-2025.08-1/bin/i686-linux-gcc /usr/local/bin/i686-linux-musl-gcc
ln -sf /opt/x86-i686--musl--stable-2025.08-1/bin/i686-linux-g++ /usr/local/bin/i686-linux-musl-g++
echo "i686-linux-musl toolchain setup completed."
# Make sure rustup is fully configured for minimal profile
if [ ! -f /root/.cargo/bin/rustc ]; then
  rm -rf /root/.rustup /root/.cargo
  rustup-init -y --default-toolchain stable -t i686-unknown-linux-musl --profile minimal
fi
'@.Replace("`r`n", "`n")
wsl -d $WslDistro sh -c $setupToolchain.Replace("'", "'\''")

# ─────────────────────────────────────────────────────────────────────────────
# 6. Sync workspace source and build scripts to WSL
# ─────────────────────────────────────────────────────────────────────────────

Step "Syncing workspace to WSL native filesystem..."
InvokeWsl "mkdir -p $WslRepo"
InvokeWsl "rm -rf $WslRepo/src"
InvokeWsl "cp -r '$WslSrc/Cargo.toml' '$WslSrc/Cargo.lock' '$WslSrc/src' $WslRepo/"
InvokeWsl "cp '$WslSrc/tools/update.sh' $WslRepo/update.sh && chmod +x $WslRepo/update.sh"

Step "Writing Cargo cross-compilation config..."
InvokeWsl "mkdir -p $WslRepo/.cargo && printf '[target.i686-unknown-linux-musl]\nlinker = \"i686-linux-musl-gcc\"\n' > $WslRepo/.cargo/config.toml"

# ─────────────────────────────────────────────────────────────────────────────
# 7. Execute compile & packaging steps via update.sh inside WSL
# ─────────────────────────────────────────────────────────────────────────────

Step "Running Linux builds and packaging inside WSL..."
InvokeWsl "cd $WslRepo && ./update.sh"

# ─────────────────────────────────────────────────────────────────────────────
# 8. Copy binaries back to Windows host
# ─────────────────────────────────────────────────────────────────────────────

Step "Copying compiled Linux binaries back to host..."
New-Item -ItemType Directory -Force -Path $HostTarget | Out-Null
InvokeWsl "cp ~/yourls-tray-app/target/release/yourls-tray-app                           '$WslTarget/yourls-tray-app_lin64-release'"
InvokeWsl "cp ~/yourls-tray-app/yourls-tray-app-x86_64.AppImage                          '$WslTarget/yourls-tray-app_lin64-release.AppImage'"
InvokeWsl "cp ~/yourls-tray-app/target/i686-unknown-linux-musl/release/yourls-tray-app   '$WslTarget/yourls-tray-app_lin32-release'"
InvokeWsl "cp ~/yourls-tray-app/yourls-tray-app-i686.AppImage                            '$WslTarget/yourls-tray-app_lin32-release.AppImage'"

# ─────────────────────────────────────────────────────────────────────────────
# 10. Rename Windows binaries
# ─────────────────────────────────────────────────────────────────────────────

Copy-Item "target\x86_64-pc-windows-msvc\release\yourls-tray-app.exe" "$HostTarget\yourls-tray-app_win64-release.exe" -Force
Copy-Item "target\i686-pc-windows-msvc\release\yourls-tray-app.exe"   "$HostTarget\yourls-tray-app_win32-release.exe" -Force

# ─────────────────────────────────────────────────────────────────────────────
# 11. Commit, tag and push
# ─────────────────────────────────────────────────────────────────────────────

Step "Creating Git commit and tag $Tag..."
git add .
git commit -m $CommitMessage
git push origin main
git tag -f $Tag
git push -f origin $Tag

# ─────────────────────────────────────────────────────────────────────────────
# 12. Publish GitHub Release
# ─────────────────────────────────────────────────────────────────────────────

Step "Publishing GitHub Release $Tag..."
$env:GITHUB_TOKEN = ""

$notesFile  = "$env:TEMP\release_notes_$Tag.md"
$assetPaths = $ReleaseAssets | ForEach-Object { "$HostTarget\$($_.FileName)" }

Set-Content -Path $notesFile -Value (Build-ReleaseNotes $Tag $CommitMessage) -NoNewline

gh release create $Tag --title $Tag --notes-file $notesFile @assetPaths

if ($LASTEXITCODE -eq 0) {
    Write-Host "Successfully published Release $Tag!" -ForegroundColor Green
} else {
    Write-Error "Failed to publish release on GitHub."
}
