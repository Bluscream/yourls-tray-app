# YOURLS Tray App

A lightweight system tray application written in Rust for Windows and Linux (Wayland/X11) that monitors your clipboard for URLs and automatically shortens them using your personal YOURLS server.

## Features

- **Automated Shortening**: Automatically detects absolute URLs copied to the clipboard and replaces them with shortened links.
- **Copy Bypasses**:
  - **Double-Copy**: Copy the same URL twice consecutively to bypass shortening.
  - **Shift Key**: Hold down the `Shift` key while copying a URL to temporarily bypass shortening.
  - **Scroll Lock**: Activate `Scroll Lock` on your keyboard to pause all automatic shortening globally.
- **System Tray Context Menu**:
  - Toggle the shortener on/off.
  - Access recent shortening history to copy previous short links.
  - Quick access to edit configurations.
- **Regex URL Blacklist**: Block specific URLs or domains from being shortened using custom regular expressions.
- **Local Logging**: Debug logs are written to `%TEMP%/yourls-tray-app.log` (Windows) or `/tmp/yourls-tray-app.log` (Linux).
- **Desktop Notifications**: Standard Toast Notifications (Windows) or Desktop Notifications (Linux) alert you when links are successfully shortened.

## Screenshots

<details>
<summary>Screenshot(s)</summary>

| YOURLS Tray App Menu |
| :---: |
| ![YOURLS Tray App Menu](https://raw.githubusercontent.com/Bluscream/yourls-tray-app/master/screenshots/tray_menu.png) |

</details>

## Installation & Setup

### Windows
1. Download the latest release `yourls-tray-app.exe` from the [Releases](https://github.com/Bluscream/yourls-tray-app/releases) tab.
2. Run the application once to generate the default configuration.
3. Open the configuration file (via tray menu **Edit Configuration** or at `%USERPROFILE%/.yourls-clipboard-shortener/config.toml`).

### Linux
To run natively on Linux (Wayland or X11), ensure the required clipboard and keyboard library packages are installed:

* **Fedora Silverblue / Bazzite / Kinoite (via rpm-ostree)**:
  ```bash
  sudo rpm-ostree install --apply-live wl-clipboard xdotool
  ```
* **Fedora / RHEL**:
  ```bash
  sudo dnf install wl-clipboard xdotool
  ```
* **Ubuntu / Debian**:
  ```bash
  sudo apt install wl-clipboard xdotool
  ```
* **Arch Linux**:
  ```bash
  sudo pacman -S wl-clipboard xdotool
  ```

1. Download the latest release `yourls-tray-app` Linux binary from the [Releases](https://github.com/Bluscream/yourls-tray-app/releases) tab.
2. Make it executable (`chmod +x yourls-tray-app`) and run it once to generate the configuration file.
3. Edit the config via the tray menu or at `~/.yourls-clipboard-shortener/config.toml`.

## Configuration Options

Configure the application by modifying `config.toml`. The following options are available:

| Setting Option | Type | Default Value | Description |
| :--- | :--- | :--- | :--- |
| `base_url` | String | `""` | The base/home URL of your YOURLS instance (e.g. `https://sho.rt/`). Optional if `api_url` is provided. |
| `api_url` | String | `""` | The API endpoint of your YOURLS instance (`yourls-api.php`). Optional if `base_url` is provided. |
| `signature` | String | `""` | Your secure, passwordless YOURLS API signature token. |
| `enabled` | Boolean | `true` | Whether the clipboard monitoring and auto-shortening is active on startup. |
| `blacklist_regex` | String | `""` | Optional regex pattern to filter out specific URLs (e.g. `^https://discord\.com/users/\d{17,20}$`). |
| `bypass_double_copy` | Boolean | `true` | Skip shortening if the exact same URL is copied twice consecutively. |
| `bypass_shift_key` | Boolean | `true` | Bypass shortening if the `Shift` key is held down during the copy event. |
| `bypass_scroll_lock` | Boolean | `true` | Pause shortening globally when keyboard `Scroll Lock` is turned ON. |
| `enable_undo` | Boolean | `true` | Restore/undo the previous clipboard contents if the shortener request fails. |

## Authors

- **Bluscream**
- **Antigravity.AI**

## Other Plugins

Check out our other YOURLS plugins:
- [Manage Protocols](https://github.com/Bluscream/yourls-manage-protocols-plugin): Add, view, toggle, and delete allowed URL protocols.
- [Prune Inactive Links](https://github.com/Bluscream/yourls-prune-inactive-links-plugin): Automatically deletes old links that receive no clicks.
- [Public Shortener Front Page](https://github.com/Bluscream/yourls-public-shortener-plugin): A premium, Turnstile-secured public URL shortener.
- [Modern Clicks Log Viewer](https://github.com/Bluscream/yourls-modern-log-viewer-plugin): Responsive table of click logs with GeoLite2 geolocation.

## AI Disclaimer

This application was created and is maintained with the assistance of Antigravity, an agentic AI coding assistant by Google DeepMind.
