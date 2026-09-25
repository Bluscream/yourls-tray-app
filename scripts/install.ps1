<#
.SYNOPSIS
    Installs yourls for the current user: the binary, a Start Menu entry, and
    optionally an autostart entry for the tray.

.DESCRIPTION
    Everything goes under the user's profile. No admin rights, no Program
    Files, no registry beyond the shortcut the Startup folder already is.

    Both shortcuts pass --tray. Without it the binary is a shortener with no
    URL to shorten, which exits immediately — a Start Menu entry that appears
    to do nothing.

.PARAMETER Autostart
    Also start the tray when you log in.

.PARAMETER Binary
    Install this .exe instead of building one.

.PARAMETER Uninstall
    Remove everything this script installed.
#>
[CmdletBinding()]
param(
    [switch]$Autostart,
    [string]$Binary,
    [switch]$Uninstall
)

$ErrorActionPreference = 'Stop'

$InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\yourls'
$StartMenu  = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
$StartupDir = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Startup'
$LinkName   = 'YOURLS Shortener.lnk'
$ExePath    = Join-Path $InstallDir 'yourls.exe'

function New-Shortcut([string]$Path, [string]$Target, [string]$Arguments) {
    $shell = New-Object -ComObject WScript.Shell
    $link = $shell.CreateShortcut($Path)
    $link.TargetPath = $Target
    $link.Arguments = $Arguments
    $link.WorkingDirectory = Split-Path $Target -Parent
    $link.IconLocation = $Target
    $link.Description = 'Shorten links from the clipboard'
    $link.Save()
}

if ($Uninstall) {
    $removed = $false
    foreach ($path in @((Join-Path $StartMenu $LinkName), (Join-Path $StartupDir $LinkName), $InstallDir)) {
        if (Test-Path $path) {
            Remove-Item $path -Recurse -Force
            Write-Host "removed $path"
            $removed = $true
        }
    }
    if (-not $removed) { Write-Host 'nothing was installed' }
    return
}

$projectDir = Split-Path $PSScriptRoot -Parent

if (-not $Binary) {
    $Binary = Join-Path $projectDir 'target\release\yourls.exe'
    if (-not (Test-Path $Binary)) {
        Write-Host "==> building (no binary at $Binary)"
        Push-Location $projectDir
        try { cargo build --release --bin yourls } finally { Pop-Location }
    }
}
if (-not (Test-Path $Binary)) {
    throw "no binary at $Binary"
}

Write-Host '==> installing'
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item $Binary $ExePath -Force
Write-Host "    $ExePath"

New-Shortcut (Join-Path $StartMenu $LinkName) $ExePath '--tray'
Write-Host "    $(Join-Path $StartMenu $LinkName)"

if ($Autostart) {
    New-Shortcut (Join-Path $StartupDir $LinkName) $ExePath '--tray'
    Write-Host "    $(Join-Path $StartupDir $LinkName) (starts the tray at login)"
} else {
    Write-Host '    (no autostart; pass -Autostart for that)'
}

# The CLI is only useful if a shell can find it.
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable('Path', "$userPath;$InstallDir", 'User')
    Write-Host "    added $InstallDir to your PATH (open a new terminal for it)"
}

Write-Host '==> done. Try: yourls https://example.com'
