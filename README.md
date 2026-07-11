# yourls-tray-app

A lightweight Windows system tray application written in Rust that monitors your clipboard for URLs and automatically shortens them using your personal YOURLS server.

## Features

- **Automated Shortening**: Automatically detects absolute URLs copied to the clipboard and replaces them with shortened links.
- **Double-Copy Bypass**: Quickly copy the same URL twice consecutively to bypass the shortener (useful when you want to copy the original long URL).
- **System Tray Context Menu**:
  - Toggle the shortener on/off.
  - Access recent shortening history to copy previous short links.
  - Quick access to edit configurations.
- **Regex URL Blacklist**: Block specific URLs or domains from being shortened using custom regular expressions.
- **Local Logging**: Debug logs are written to `%TEMP%/yourls-tray-app.log` to track URL detection and API status.
- **Desktop Notifications**: Standard Windows Toast Notifications alert you when links are successfully shortened.

## Installation & Setup

1. Download the latest release executable from the [Releases](https://github.com/Bluscream/yourls-tray-app/releases) tab.
2. Run the application once to generate the default configuration.
3. Open the configuration file (via tray menu **Edit Configuration** or at `%USERPROFILE%/.yourls-clipboard-shortener/config.toml`) and set your:
   - `api_url`: Your YOURLS API endpoint (e.g. `https://sho.rt/yourls-api.php`).
   - `signature`: Your YOURLS passwordless API token.
   - `blacklist_regex`: Optional regex pattern to filter out specific URLs (e.g., `'^https://discord\.com/users/\d{17,20}$'`).

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
