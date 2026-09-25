# yourls [![Latest Release](https://img.shields.io/github/v/release/Bluscream/yourls-tray-app?label=version&style=flat-square)](https://github.com/Bluscream/yourls-tray-app/releases/latest) [![Total Downloads](https://img.shields.io/github/downloads/Bluscream/yourls-tray-app/total?style=flat-square&label=total%20downloads)](https://github.com/Bluscream/yourls-tray-app/releases)

A URL shortener for your own [YOURLS](https://yourls.org) server, written in
Rust for **Windows** and **Linux** (Wayland/X11).

Give it a URL, it gives you a short one:

```sh
yourls https://example.com/a/very/long/address
https://sho.rt/abc123
```

It is also a tray app, behind `--tray`, which watches the clipboard and
replaces URLs as you copy them. That was the original program and it still
works exactly as it did — it is just no longer the only way to use it.

## Features

### Command line

- **Shorten a URL**: `yourls <url>`, or from a pipe — only the short URL is
  printed, so it composes with anything.
- **Custom ids**: `--id my-link` asks for `https://sho.rt/my-link` instead of
  a generated id. Also spelled `--keyword` (the API's own name) or `--slug`.
- **Server selection**: `--server <name>` pins one request to one configured
  instance; `auto`, or leaving it out, follows the config.
- **Honest exit codes**: 0 on success, 1 on failure, 2 on a bad argument.

### Tray (`--tray`)

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

## Installation

### Linux

Install the system dependencies first — only the tray needs them:

| Distribution | Command |
| :--- | :--- |
| **Ubuntu / Debian** | `sudo apt install wl-clipboard xdotool` |
| **Arch Linux** | `sudo pacman -S wl-clipboard xdotool` |
| **Fedora / RHEL** | `sudo dnf install wl-clipboard xdotool` |
| **Fedora Silverblue / Bazzite** | `sudo rpm-ostree install --apply-live wl-clipboard xdotool` |

Then either run the installer from a clone:

```sh
./scripts/install.sh              # binary + menu entry
./scripts/install.sh --autostart  # and start the tray at login
./scripts/install.sh --uninstall  # remove all of it
```

or download from [Releases](https://github.com/Bluscream/yourls-tray-app/releases/latest):

| Asset | What it is |
| :--- | :--- |
| `yourls_lin64-release.AppImage` | tray + CLI, dependencies bundled |
| `yourls_lin64-release` | tray + CLI, plain binary |
| `yourls-cli_lin64-release` | CLI only — no GTK, no X11, runs headless |

The installer puts everything under `$HOME`: the binary in `~/.local/bin`, a
menu entry in `~/.local/share/applications`, and with `--autostart` a
`~/.config/autostart` entry. Both entries run `yourls --tray`.

### Windows

```powershell
.\scripts\install.ps1              # binary + Start Menu entry
.\scripts\install.ps1 -Autostart   # and start the tray at login
.\scripts\install.ps1 -Uninstall   # remove all of it
```

Or download `yourls_win64-release.exe` (tray + CLI) or
`yourls-cli_win64-release.exe` (CLI only) from the releases page.

Installs to `%LOCALAPPDATA%\Programs\yourls`, adds it to your `PATH`, and
creates shortcuts that pass `--tray`. No admin rights needed.

### First run

Run it once to generate a config:

- **Linux**: `~/.yourls-clipboard-shortener/config.toml`
- **Windows**: `%USERPROFILE%\.yourls-clipboard-shortener\config.toml`

Fill in your server details. `yourls` tells you the exact path if no server is
configured yet, and the tray has an **Edit Configuration** entry.

> **Tip**: `config.toml` is also picked up from next to the executable, so app
> and config can live in one folder for a portable setup.

## Command line

The binary is `yourls`. Shortening is what it does by default; the tray is
opt-in.

```sh
yourls https://example.com/something        # prints the short URL
echo https://example.com | yourls           # or read it from stdin
yourls --server sho.rt <url>                # pin it to one configured server
yourls --id my-link <url>                   # ask for a specific short id
yourls --server auto <url>                  # or follow the config (the default)
yourls --tray                               # run the clipboard tray
```

Only the short URL goes to standard output, so it composes:

```sh
yourls "$url" | wl-copy
```

`--id` (also `--keyword`, the API's own name, or `--slug`) asks for a specific
short id instead of a generated one. It fails rather than quietly returning
something else if that id is taken, or if the URL already has a different one.

Errors go to standard error and exit 1; a bad argument exits 2. A URL matching
`blacklist_regex` is printed back unchanged, so a pipe never loses it.

Two builds are released. The default has the tray and works as a CLI too. The
`-cli` one is built with `--no-default-features` and links no GTK,
appindicator or libxdo at all — six libraries instead of seventy-two — so it
runs on a machine with no desktop.


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

Requires [Rust](https://rustup.rs) stable. The tray also needs GTK 3,
libayatana-appindicator and libxdo headers; a CLI-only build needs none of
them.

```bash
./scripts/build.sh          # the gate: fmt, clippy, tests, both builds
./scripts/appimage.sh       # portable AppImage into dist/
./scripts/windows.sh        # cross-compile the Windows binaries (mingw-w64)

cargo build --release                        # tray + CLI
cargo build --release --no-default-features  # CLI only, no GUI libraries
```

`scripts/build.sh` is the gate. Check its exit status directly rather than
piping it into `grep` — `build.sh | grep ok && git commit` reports grep's
status, so a failing build gets committed anyway.

`scripts/appimage.sh` compiles inside Ubuntu 20.04 (glibc 2.31) so the result
runs on anything newer, bundles the GTK/tray stack with linuxdeploy, then
starts the finished AppImage and fails if any library is unresolved. Do not
build Linux artifacts on Alpine: a dynamically linked musl binary cannot start
on a glibc desktop, which is what made every release up to v1.0.5 unrunnable.

For a full build + release (Windows + Linux + AppImage + GitHub release):

```powershell
.\tools\update.ps1 -Version "1.2.0" -CommitMessage "Your release notes here"
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
