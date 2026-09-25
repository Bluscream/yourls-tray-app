<#
.SYNOPSIS
    Installs yourls for the current user.

.DESCRIPTION
    Works two ways: from a clone, where it installs the local build, and piped
    straight from the web, where it downloads the latest release.

        irm https://raw.githubusercontent.com/Bluscream/yourls-tray-app/main/scripts/install.ps1 | iex

    With options, which `iex` cannot pass on its own:

        & ([scriptblock]::Create((irm https://raw.githubusercontent.com/Bluscream/yourls-tray-app/main/scripts/install.ps1))) -AutostartTray -StartMenuShortcut

    Everything goes under the user's profile. No admin rights and no Program
    Files.

.PARAMETER AutostartTray
    Start the tray when you log in.

.PARAMETER DesktopShortcut
    Put a shortcut on the desktop.

.PARAMETER StartMenuShortcut
    Put an entry in the Start Menu.

.PARAMETER Binary
    Install this .exe instead of downloading one.

.PARAMETER Uninstall
    Remove everything this script installed.
#>
[CmdletBinding()]
param(
    [switch]$AutostartTray,
    [switch]$DesktopShortcut,
    [switch]$StartMenuShortcut,
    [string]$Binary,
    [switch]$Uninstall
)

$ErrorActionPreference = 'Stop'

$Repo       = 'Bluscream/yourls-tray-app'
$Issues     = "https://github.com/$Repo/issues/new"
$InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\yourls'
$StartMenu  = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
$StartupDir = Join-Path $StartMenu 'Startup'
$DesktopDir = [Environment]::GetFolderPath('Desktop')
$LinkName   = 'YOURLS Shortener.lnk'
$ExePath    = Join-Path $InstallDir 'yourls.exe'

function Stop-Unsupported([string]$Reason) {
    Write-Host ''
    Write-Host 'yourls has no build that runs here.'
    Write-Host ''
    Write-Host "  system:  Windows $([Environment]::OSVersion.Version) $env:PROCESSOR_ARCHITECTURE"
    Write-Host "  reason:  $Reason"
    Write-Host ''
    Write-Host 'If this machine should be supported, please say so — include the two lines above:'
    Write-Host "  $Issues"
    Write-Host ''
    Write-Host "You can still build it yourself: https://github.com/$Repo#building-from-source"
    exit 1
}

function New-Shortcut([string]$Path, [string]$Target, [string]$Arguments) {
    $parent = Split-Path $Path -Parent
    if (-not (Test-Path $parent)) { New-Item -ItemType Directory -Force -Path $parent | Out-Null }
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
    $paths = @(
        (Join-Path $StartMenu $LinkName),
        (Join-Path $StartupDir $LinkName),
        (Join-Path $DesktopDir $LinkName),
        $InstallDir
    )
    foreach ($path in $paths) {
        if (Test-Path $path) {
            Remove-Item $path -Recurse -Force
            Write-Host "removed $path"
            $removed = $true
        }
    }

    # Installed by an older run of this script, so it is removed too.
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($userPath -and $userPath.Contains($InstallDir)) {
        $kept = ($userPath -split ';' | Where-Object { $_ -and $_ -ne $InstallDir }) -join ';'
        [Environment]::SetEnvironmentVariable('Path', $kept, 'User')
        Write-Host "removed $InstallDir from your PATH"
        $removed = $true
    }

    if (-not $removed) { Write-Host 'nothing was installed' }
    Write-Host ''
    Write-Host 'Your configuration was left alone: %USERPROFILE%\.yourls-clipboard-shortener\'
    return
}

if ($env:PROCESSOR_ARCHITECTURE -notin @('AMD64', 'x86')) {
    Stop-Unsupported 'only x86_64 is released today'
}

# A local build when this is a clone, the release otherwise.
$temp = $null
if (-not $Binary) {
    $projectDir = if ($PSScriptRoot) { Split-Path $PSScriptRoot -Parent } else { $null }
    $local = if ($projectDir) { Join-Path $projectDir 'target\release\yourls.exe' } else { $null }

    if ($local -and (Test-Path $local)) {
        Write-Host '==> installing the local build'
        $Binary = $local
    } else {
        # The tray build, which is the CLI as well: Windows always has a
        # desktop, so the CLI-only asset would only take features away.
        $asset = 'yourls_win64-release.exe'
        $url = "https://github.com/$Repo/releases/latest/download/$asset"
        $temp = Join-Path ([System.IO.Path]::GetTempPath()) "yourls-$([guid]::NewGuid()).exe"
        Write-Host "==> downloading $asset"
        try {
            Invoke-WebRequest -Uri $url -OutFile $temp -UseBasicParsing
        } catch {
            Stop-Unsupported "could not download $url ($($_.Exception.Message))"
        }
        $Binary = $temp
    }
}
if (-not (Test-Path $Binary)) { throw "no binary at $Binary" }

Write-Host '==> installing'
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item $Binary $ExePath -Force
if ($temp) { Remove-Item $temp -Force -ErrorAction SilentlyContinue }
Write-Host "    $ExePath"

# Every shortcut passes --tray. Without it the shortcut runs a shortener with
# no URL, which exits at once and looks like a shortcut that does nothing.
if ($StartMenuShortcut) {
    New-Shortcut (Join-Path $StartMenu $LinkName) $ExePath '--tray'
    Write-Host "    $(Join-Path $StartMenu $LinkName)"
}
if ($DesktopShortcut) {
    New-Shortcut (Join-Path $DesktopDir $LinkName) $ExePath '--tray'
    Write-Host "    $(Join-Path $DesktopDir $LinkName)"
}
if ($AutostartTray) {
    New-Shortcut (Join-Path $StartupDir $LinkName) $ExePath '--tray'
    Write-Host "    $(Join-Path $StartupDir $LinkName) (starts the tray at login)"
}

# The CLI is only useful if a shell can find it.
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable('Path', "$userPath;$InstallDir", 'User')
    Write-Host "    added $InstallDir to your PATH (open a new terminal for it)"
}

Write-Host '==> done. Try: yourls https://example.com'
