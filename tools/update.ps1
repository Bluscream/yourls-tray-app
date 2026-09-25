param(
    [string]$Version,
    [string]$CommitMessage
)

$ErrorActionPreference = "Stop"

# ─────────────────────────────────────────────────────────────────────────────
# Constants
# ─────────────────────────────────────────────────────────────────────────────

$Repo         = "Bluscream/yourls-tray-app"
# A glibc distribution on purpose. Alpine produced dynamically linked musl
# binaries that could not start on any ordinary desktop; see tools/update.sh.
$WslDistroX64 = "Debian"
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
    @{ FileName = "yourls-tray-app_lin64-release";           Label = "linux64";          Description = "Linux 64-bit" }
    @{ FileName = "yourls-tray-app_lin64-release.AppImage";  Label = "linux64-appimage"; Description = "Linux 64-bit AppImage" }
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
if ($wslList -notmatch "Alpine\s") {
    Write-Host "Alpine WSL distro not found - bootstrapping..." -ForegroundColor Yellow
    $tarball = "$env:TEMP\alpine-minirootfs.tar.gz"
    Invoke-WebRequest -Uri "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/alpine-minirootfs-3.24.0-x86_64.tar.gz" -OutFile $tarball -UseBasicParsing
    New-Item -ItemType Directory -Force -Path "C:\WSL\Alpine" | Out-Null
    wsl --import Alpine C:\WSL\Alpine $tarball
}

# ─────────────────────────────────────────────────────────────────────────────
# 5. Sync workspace source and build scripts to WSL
# ─────────────────────────────────────────────────────────────────────────────

Step "Syncing workspace to WSL native filesystem..."
InvokeWsl "mkdir -p $WslRepo"
InvokeWsl "rm -rf $WslRepo/src"
InvokeWsl "cp -r '$WslSrc/Cargo.toml' '$WslSrc/Cargo.lock' '$WslSrc/src' $WslRepo/"
InvokeWsl "cp '$WslSrc/tools/update.sh' $WslRepo/update.sh && chmod +x $WslRepo/update.sh"
# update.sh delegates the bundling to scripts/appimage.sh, so that has to be
# there too.
InvokeWsl "rm -rf $WslRepo/scripts"
InvokeWsl "cp -r '$WslSrc/scripts' $WslRepo/ && chmod +x $WslRepo/scripts/*.sh"

# ─────────────────────────────────────────────────────────────────────────────
# 6. Execute compile & packaging steps via update.sh inside WSL
# ─────────────────────────────────────────────────────────────────────────────

Step "Running Linux builds and packaging inside WSL $WslDistroX64..."
InvokeWsl "cd $WslRepo && ./update.sh"

# ─────────────────────────────────────────────────────────────────────────────
# 7. Copy binaries back to Windows host
# ─────────────────────────────────────────────────────────────────────────────

Step "Copying compiled Linux binaries back to host..."
New-Item -ItemType Directory -Force -Path $HostTarget | Out-Null
InvokeWsl "cp ~/yourls-tray-app/target/release/yourls                       '$WslTarget/yourls-tray-app_lin64-release'"
InvokeWsl "cp ~/yourls-tray-app/dist/yourls-*-x86_64.AppImage               '$WslTarget/yourls-tray-app_lin64-release.AppImage'"

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
