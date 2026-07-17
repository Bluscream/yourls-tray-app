param(
    [string]$Version,
    [string]$CommitMessage
)

$ErrorActionPreference = "Stop"

# ─────────────────────────────────────────────────────────────────────────────
# Constants
# ─────────────────────────────────────────────────────────────────────────────

$Repo         = "Bluscream/yourls-tray-app"
$WslDistroX64 = "Alpine"
$WslDistroX86 = "Alpine32"
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
    wsl -d $WslDistroX64 sh -c $Cmd
}

function InvokeWslDistro([string]$Distro, [string]$Cmd) {
    wsl -d $Distro sh -c $Cmd
}

function Get-AssetShield([hashtable]$Asset, [string]$Tag) {
    $fileName = $Asset.FileName
    $label = [uri]::EscapeDataString($Asset.Description)
    $url = "${BadgeBase}/${Tag}/${fileName}?${BadgeStyle}&label=${label}"
    return "[![]($url)](https://github.com/${Repo}/releases/download/${Tag}/${fileName})"
}

function Get-TotalShield {
    return "[![Downloads](${BadgeBase}/total?${BadgeStyle}&label=total+downloads)](https://github.com/${Repo}/releases)"
}

function Get-AssetLine([hashtable]$Asset, [string]$Tag) {
    return "* $(Get-AssetShield $Asset $Tag)"
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

Step "Verifying WSL Alpine Linux distributions (x64 and i686)..."
$wslList = (wsl.exe -l -v | Out-String) -replace "\x00", ""
if ($wslList -notmatch "Alpine\s") {
    Write-Host "Alpine x64 WSL distro not found - bootstrapping..." -ForegroundColor Yellow
    $tarball64 = "$env:TEMP\alpine-minirootfs-64.tar.gz"
    Invoke-WebRequest -Uri "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/alpine-minirootfs-3.24.0-x86_64.tar.gz" -OutFile $tarball64 -UseBasicParsing
    New-Item -ItemType Directory -Force -Path "C:\WSL\Alpine" | Out-Null
    wsl --import Alpine C:\WSL\Alpine $tarball64
}
if ($wslList -notmatch "Alpine32") {
    Write-Host "Alpine32 (i686) WSL distro not found - bootstrapping..." -ForegroundColor Yellow
    $tarball32 = "$env:TEMP\alpine-minirootfs-32.tar.gz"
    # Download the official Alpine x86 (i686 / 32-bit) minirootfs release
    Invoke-WebRequest -Uri "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86/alpine-minirootfs-3.24.0-x86.tar.gz" -OutFile $tarball32 -UseBasicParsing
    New-Item -ItemType Directory -Force -Path "C:\WSL\Alpine32" | Out-Null
    wsl --import Alpine32 C:\WSL\Alpine32 $tarball32
}

# Remove duplicate helper (defined in top helpers section)

# ─────────────────────────────────────────────────────────────────────────────
# 5. Sync workspace source and build scripts to both WSL containers
# ─────────────────────────────────────────────────────────────────────────────

Step "Syncing workspace to both WSL native filesystems..."
foreach ($distro in @($WslDistroX64, $WslDistroX86)) {
    InvokeWslDistro $distro "mkdir -p $WslRepo"
    InvokeWslDistro $distro "rm -rf $WslRepo/src"
    InvokeWslDistro $distro "cp -r '$WslSrc/Cargo.toml' '$WslSrc/Cargo.lock' '$WslSrc/src' $WslRepo/"
    InvokeWslDistro $distro "cp '$WslSrc/tools/update.sh' $WslRepo/update.sh && chmod +x $WslRepo/update.sh"
}

# ─────────────────────────────────────────────────────────────────────────────
# 6. Execute compile & packaging steps via update.sh inside WSL containers
# ─────────────────────────────────────────────────────────────────────────────

Step "Running Linux builds and packaging natively inside WSL $WslDistroX64 (x64)..."
InvokeWslDistro $WslDistroX64 "cd $WslRepo && ./update.sh"

Step "Running Linux builds and packaging natively inside WSL $WslDistroX86 (i686)..."
InvokeWslDistro $WslDistroX86 "cd $WslRepo && ./update.sh"

# ─────────────────────────────────────────────────────────────────────────────
# 7. Copy binaries back to Windows host
# ─────────────────────────────────────────────────────────────────────────────

Step "Copying compiled Linux binaries back to host..."
New-Item -ItemType Directory -Force -Path $HostTarget | Out-Null
InvokeWslDistro $WslDistroX64 "cp ~/yourls-tray-app/target/release/yourls-tray-app              '$WslTarget/yourls-tray-app_lin64-release'"
InvokeWslDistro $WslDistroX64 "cp ~/yourls-tray-app/yourls-tray-app-x86_64.AppImage             '$WslTarget/yourls-tray-app_lin64-release.AppImage'"
InvokeWslDistro $WslDistroX86 "cp ~/yourls-tray-app/target/release/yourls-tray-app              '$WslTarget/yourls-tray-app_lin32-release'"
InvokeWslDistro $WslDistroX86 "cp ~/yourls-tray-app/yourls-tray-app-i686.AppImage               '$WslTarget/yourls-tray-app_lin32-release.AppImage'"

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
