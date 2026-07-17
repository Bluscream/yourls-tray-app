# YOURLS Tray App [![Latest Release](https://img.shields.io/github/v/release/Bluscream/yourls-tray-app?label=version&style=flat-square)](https://github.com/Bluscream/yourls-tray-app/releases/latest) [![Total Downloads](https://img.shields.io/github/downloads/Bluscream/yourls-tray-app/total?style=flat-square&label=total%20downloads)](https://github.com/Bluscream/yourls-tray-app/releases)

A lightweight, cross-platform system tray application written in Rust for **Windows** and **Linux** (Wayland/X11) that monitors your clipboard for URLs and automatically shortens them using your personal [YOURLS](https://yourls.org) server.

## Features

- **Automated URL Shortening**: Instantly detects absolute URLs copied to the clipboard and replaces them with a shortened link from your YOURLS server.
- **Multiple YOURLS Servers**: Configure multiple servers and switch between them from the tray menu. Choose a specific server or let the app pick one at random.
- **Shorten on All Servers**: Optionally broadcast every shortening request to all configured servers simultaneously.
- **Copy Bypasses** — Fine-grained control over when shortening is skipped:
  - **Double-Copy**: Copy the same URL twice consecutively to bypass shortening.
  - **Shift Key**: Hold `Shift` while copying to bypass shortening for that copy event.
  - **Scroll Lock**: Activate keyboard `Scroll Lock` to globally pause all shortening.
- **Undo Hotkey** (`Ctrl+Backspace`): Instantly restore the last original (pre-shortened) URL back to your clipboard.
- **Regex URL Blacklist**: Block specific URLs or domains from being shortened using custom regular expressions.
- **Recent Links History**: Access a submenu of your recently shortened links to quickly re-copy any short URL.
- **Desktop Notifications**: Toast notifications (Windows) or desktop notifications (Linux) when a link is shortened or an error occurs.
- **Update Checker**: Check for new releases from the tray menu or automatically on startup (`check_update_on_startup`). Opens the GitHub release page in your browser when an update is found.
- **Internationalization (i18n)**: All UI strings are fully localized. Supported languages: 🇩🇪 German, 🇷🇺 Russian, 🇯🇵 Japanese, 🇨🇳 Chinese, 🇫🇷 French, 🇪🇸 Spanish, 🇬🇧 English (fallback). Auto-detects system language.
- **Configurable Log Filename**: Customize the log file name with optional timestamp patterns (e.g. `yourls-%Y-%m-%d.log`).
- **Single Instance Guard**: Prevents multiple copies from running simultaneously.

## Screenshots

<details>
<summary>Screenshot(s)</summary>

| YOURLS Tray App Menu |
| :---: |
| ![YOURLS Tray App Menu](https://raw.githubusercontent.com/Bluscream/yourls-tray-app/master/assets/tray_menu.png) |

</details>

## Installation & Setup

### Windows

1. Download `yourls-tray-app_win64-release.exe` (or `win32`) from the [Releases](https://github.com/Bluscream/yourls-tray-app/releases/latest) page.
2. Run it once to generate a default `config.toml` in `%USERPROFILE%\.yourls-clipboard-shortener\`.
3. Open the config via the tray menu → **Edit Configuration**, or navigate to the file directly.
4. Fill in your server details and restart the app.

### Linux

Install required system dependencies first:

| Distribution | Command |
| :--- | :--- |
| **Ubuntu / Debian** | `sudo apt install wl-clipboard xdotool` |
| **Arch Linux** | `sudo pacman -S wl-clipboard xdotool` |
| **Fedora / RHEL** | `sudo dnf install wl-clipboard xdotool` |
| **Fedora Silverblue / Bazzite** | `sudo rpm-ostree install --apply-live wl-clipboard xdotool` |

1. Download `yourls-tray-app_lin64-release` (ELF) or `yourls-tray-app_lin64-release.AppImage` from the [Releases](https://github.com/Bluscream/yourls-tray-app/releases/latest) page.
2. Make it executable: `chmod +x yourls-tray-app_lin64-release`
3. Run once to generate `~/.yourls-clipboard-shortener/config.toml`.
4. Edit config via tray → **Edit Configuration** and restart.

> **Tip**: The `config.toml` is also auto-discovered next to the executable, so you can keep app + config in the same folder for a portable setup.

## Configuration Reference

The app auto-generates a default `config.toml` on first run. All fields are optional and revert to their defaults if omitted. Comments below describe every available option.

```toml
# ── Clipboard behaviour ────────────────────────────────────────────────────────

# Master switch. Set to false to start the app with monitoring paused.
enabled = true

# Regex pattern — URLs matching this are never shortened.
# Leave empty to disable. Example: "^https://discord\\.com/"
blacklist_regex = ""

# ── Copy bypasses ──────────────────────────────────────────────────────────────

# Skip shortening when the exact same URL is copied twice consecutively.
bypass_double_copy = true

# Skip shortening when the Shift key is held during the copy event.
bypass_shift_key = true

# Pause all shortening while keyboard Scroll Lock is active.
bypass_scroll_lock = true

# ── Undo ───────────────────────────────────────────────────────────────────────

# Ctrl+Backspace restores the original (pre-shortened) URL to your clipboard.
enable_undo = true

# ── Server selection ───────────────────────────────────────────────────────────

# Name of the server to use. Must match a [[servers]] name below.
# Use "Random" to rotate through all servers.
selected_server = "Random"

# Send every shortening request to ALL configured servers simultaneously.
shorten_on_all = false

# ── Appearance & locale ────────────────────────────────────────────────────────

# UI language. "auto" reads from the OS.
# Supported values: "auto", "en", "de", "ru", "ja", "zh", "fr", "es"
locale = "auto"

# ── Logging ────────────────────────────────────────────────────────────────────

# Log file written to the same directory as the executable.
# Supports chrono date tokens: e.g. "yourls-%Y-%m-%d.log" for daily rotation.
log_file_name = "yourls-tray-app.log"

# ── Updates ────────────────────────────────────────────────────────────────────

# Silently check GitHub for a new release on every startup.
# A dialog will prompt you to open the release page if an update is found.
check_update_on_startup = false

# ── Servers ────────────────────────────────────────────────────────────────────
# Add one [[servers]] block per YOURLS instance.
# "name" is optional — auto-derived from the URL domain if omitted.
# Only one of "base_url" or "api_url" is required; the other is inferred.

[[servers]]
name      = "sho.rt"
base_url  = "https://sho.rt/"
# api_url is inferred as https://sho.rt/yourls-api.php
signature = "your_signature_token_here"

[[servers]]
name      = "my.link"
api_url   = "https://my.link/yourls-api.php"
# base_url is inferred as https://my.link/
signature = "another_signature_token_here"
```

## Building from Source

Requires [Rust](https://rustup.rs) stable.

```bash
# Windows (x64)
cargo build --release

# Windows (x86)
cargo build --release --target i686-pc-windows-msvc

# Linux (via WSL Alpine, statically linked)
cargo build --release
```

For a full automated build + release (Windows + Linux + AppImage + GitHub release):
```powershell
.\tools\update.ps1 -Version "1.0.4" -CommitMessage "Your release notes here"
```

## Authors

- **Bluscream**
- **Antigravity.AI**

## Other YOURLS Plugins

- [Manage Protocols](https://github.com/Bluscream/yourls-manage-protocols-plugin): Add, view, toggle, and delete allowed URL protocols.
- [Prune Inactive Links](https://github.com/Bluscream/yourls-prune-inactive-links-plugin): Automatically deletes old links that receive no clicks.
- [Public Shortener Front Page](https://github.com/Bluscream/yourls-public-shortener-plugin): A premium, Turnstile-secured public URL shortener.
- [Modern Clicks Log Viewer](https://github.com/Bluscream/yourls-modern-log-viewer-plugin): Responsive table of click logs with GeoLite2 geolocation.

## AI Disclaimer

This application was created and is maintained with the assistance of Antigravity, an agentic AI coding assistant by Google DeepMind.
