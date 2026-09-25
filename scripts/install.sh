#!/usr/bin/env bash
# Installs yourls for the current user.
#
# Works two ways: from a clone, where it installs what is in target/release or
# builds it, and piped straight from the web, where it downloads the release
# asset that suits this machine:
#
#   curl -fsSL https://raw.githubusercontent.com/Bluscream/yourls-tray-app/main/scripts/install.sh | bash
#   curl -fsSL .../install.sh | bash -s -- --autostart-tray --start-menu-shortcut
#
# Everything goes under $HOME. No root, no system directories — a URL
# shortener has no business needing either.
set -euo pipefail

REPO="Bluscream/yourls-tray-app"
ISSUES="https://github.com/$REPO/issues/new"

BIN_DIR="${YOURLS_BIN_DIR:-$HOME/.local/bin}"
APP_DIR="${YOURLS_APP_DIR:-$HOME/.local/share/applications}"
ICON_DIR="${YOURLS_ICON_DIR:-$HOME/.local/share/icons/hicolor/256x256/apps}"
AUTOSTART_DIR="${YOURLS_AUTOSTART_DIR:-$HOME/.config/autostart}"
DESKTOP_DIR="${YOURLS_DESKTOP_DIR:-$(xdg-user-dir DESKTOP 2>/dev/null || echo "$HOME/Desktop")}"

CONFIG_DIR="${YOURLS_CONFIG_DIR:-$HOME/.yourls-clipboard-shortener}"
DESKTOP_NAME=yourls.desktop
AUTOSTART_TRAY=0
DESKTOP_SHORTCUT=0
START_MENU_SHORTCUT=0
SOURCE=""

usage() {
    cat <<'USAGE'
Usage: install.sh [options]
       install.sh --uninstall

  --autostart-tray        start the tray when you log in
  --desktop-shortcut      put a shortcut on the desktop
  --start-menu-shortcut   put an entry in the application menu
  --binary <path>         install this binary instead of downloading one
  --uninstall             remove everything this script installed
  --purge                 that, and the configuration as well

With no options the binary is installed on its own. All three shortcut
options run `yourls --tray`: without that argument the shortcut would run a
shortener with no URL, which exits at once.

Environment: YOURLS_BIN_DIR, YOURLS_APP_DIR, YOURLS_ICON_DIR,
YOURLS_AUTOSTART_DIR, YOURLS_DESKTOP_DIR override where things go.
USAGE
}

say() { printf '%s\n' "$*"; }
die() { printf '%s\n' "$*" >&2; exit 1; }

# Reports a machine this cannot serve, with somewhere to say so.
unsupported() {
    cat >&2 <<EOF

yourls has no build that runs here.

  system:       $(uname -s) $(uname -m)
  reason:       $1

If this machine should be supported, please say so — include the two lines
above:

  $ISSUES

You can still build it yourself: https://github.com/$REPO#building-from-source
EOF
    exit 1
}

# `purge` is passed as the first argument to also remove the configuration.
uninstall() {
    local purge="${1:-no}"
    local removed=0
    for path in "$BIN_DIR/yourls" "$BIN_DIR/yourls-cli" \
                "$APP_DIR/$DESKTOP_NAME" "$AUTOSTART_DIR/$DESKTOP_NAME" \
                "$DESKTOP_DIR/$DESKTOP_NAME" "$ICON_DIR/yourls.png"; do
        if [[ -e "$path" ]]; then
            rm -f "$path"
            say "removed $path"
            removed=1
        fi
    done
    if [[ $removed -eq 0 ]]; then
        say "nothing was installed"
    else
        command -v update-desktop-database >/dev/null 2>&1 && \
            update-desktop-database "$APP_DIR" 2>/dev/null || true
    fi

    say
    if [[ "$purge" == "purge" ]]; then
        if [[ -d "$CONFIG_DIR" ]]; then
            rm -rf "$CONFIG_DIR"
            say "removed $CONFIG_DIR, including your server settings"
        else
            say "no configuration at $CONFIG_DIR"
        fi
    else
        say "Your configuration was left alone: $CONFIG_DIR"
        say "Use --purge to remove that too."
    fi
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --autostart-tray) AUTOSTART_TRAY=1 ;;
        --desktop-shortcut) DESKTOP_SHORTCUT=1 ;;
        --start-menu-shortcut) START_MENU_SHORTCUT=1 ;;
        --binary) shift; SOURCE="${1:-}" ;;
        --uninstall) uninstall ;;
        --purge) uninstall purge ;;
        -h|--help) usage; exit 0 ;;
        *) printf 'unknown option: %s\n\n' "$1" >&2; usage >&2; exit 2 ;;
    esac
    shift
done

case "$(uname -s)" in
    Linux) ;;
    *) unsupported "this installer is for Linux; see the releases page for Windows" ;;
esac
case "$(uname -m)" in
    x86_64|amd64) ;;
    *) unsupported "only x86_64 is released today" ;;
esac

# Which build suits this machine.
#
# The tray needs GTK and libxdo at load time, so a binary that has them linked
# cannot even start without them. The AppImage carries its own copies, which
# is what makes it the answer for a desktop that lacks them.
choose_asset() {
    local has_desktop=0 has_gtk=0
    [[ -n "${WAYLAND_DISPLAY:-}${DISPLAY:-}${XDG_CURRENT_DESKTOP:-}" ]] && has_desktop=1

    # The library list is captured once and matched in the shell rather than
    # piped into grep. Under `set -o pipefail`, `ldconfig -p | grep -q` fails
    # even when the library is there: grep exits at the first match, ldconfig
    # dies of SIGPIPE, and pipefail reports the pipeline as failed — so every
    # machine looked like it had no GTK.
    local libs
    libs="$(ldconfig -p 2>/dev/null || true)"
    if [[ "$libs" == *libgtk-3.so.0* && "$libs" == *libxdo.so* ]]; then
        has_gtk=1
    fi

    if [[ $has_desktop -eq 0 ]]; then
        # No session to put a tray in; the CLI is the whole of what is useful.
        printf 'yourls-cli_lin64-release'
        return
    fi
    if [[ $has_gtk -eq 1 ]]; then
        printf 'yourls_lin64-release'
        return
    fi
    printf 'yourls_lin64-release.AppImage'
}

download() {
    local asset="$1" target="$2"
    local url="https://github.com/$REPO/releases/latest/download/$asset"
    say "==> downloading $asset"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL -o "$target" "$url" || die "could not download $url"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$target" "$url" || die "could not download $url"
    else
        unsupported "neither curl nor wget is installed"
    fi
    chmod +x "$target"
}

# The icon, for a shortcut to point at. Taken from the repo when this is a
# clone, downloaded otherwise.
install_icon() {
    local here="${BASH_SOURCE[0]:-}"
    local repo_icon=""
    if [[ -n "$here" ]]; then
        repo_icon="$(cd "$(dirname "$here")/.." 2>/dev/null && pwd || true)/src/icon.png"
    fi
    if [[ -n "$repo_icon" && -f "$repo_icon" ]]; then
        install -Dm644 "$repo_icon" "$ICON_DIR/yourls.png"
        return
    fi
    local url="https://raw.githubusercontent.com/$REPO/main/src/icon.png"
    mkdir -p "$ICON_DIR"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL -o "$ICON_DIR/yourls.png" "$url" 2>/dev/null || true
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$ICON_DIR/yourls.png" "$url" 2>/dev/null || true
    fi
}

write_desktop_entry() {
    local path="$1"
    mkdir -p "$(dirname "$path")"
    # An absolute Exec, because a desktop session's PATH often lacks
    # ~/.local/bin and the entry would then silently do nothing.
    cat > "$path" <<EOF
[Desktop Entry]
Type=Application
Name=YOURLS Shortener
Comment=Shorten links from the clipboard
Exec=$BIN_DIR/yourls --tray
Icon=$ICON_DIR/yourls.png
Terminal=false
Categories=Utility;
Keywords=url;shortener;yourls;clipboard;
StartupNotify=false
EOF
    chmod +x "$path"
}

TMP_DIR=""
cleanup() { [[ -n "$TMP_DIR" ]] && rm -rf "$TMP_DIR"; }
trap cleanup EXIT

if [[ -z "$SOURCE" ]]; then
    HERE="$(cd "$(dirname "${BASH_SOURCE[0]:-/nonexistent}")" 2>/dev/null && pwd || true)"
    LOCAL_BUILD="${HERE:+$HERE/../target/release/yourls}"

    if [[ -n "$LOCAL_BUILD" && -x "$LOCAL_BUILD" ]]; then
        SOURCE="$LOCAL_BUILD"
        say "==> installing the local build"
    else
        ASSET="$(choose_asset)"
        TMP_DIR="$(mktemp -d)"
        download "$ASSET" "$TMP_DIR/yourls"
        SOURCE="$TMP_DIR/yourls"
        case "$ASSET" in
            *-cli_*) say "    no desktop session detected; installed the CLI-only build" ;;
            *.AppImage) say "    GTK not found; installed the AppImage, which bundles it" ;;
        esac
    fi
fi

[[ -x "$SOURCE" ]] || die "no binary at $SOURCE"

say "==> installing"
install -Dm755 "$SOURCE" "$BIN_DIR/yourls"
say "    $BIN_DIR/yourls"

if [[ $START_MENU_SHORTCUT -eq 1 || $DESKTOP_SHORTCUT -eq 1 || $AUTOSTART_TRAY -eq 1 ]]; then
    install_icon
fi
if [[ $START_MENU_SHORTCUT -eq 1 ]]; then
    write_desktop_entry "$APP_DIR/$DESKTOP_NAME"
    say "    $APP_DIR/$DESKTOP_NAME"
    command -v update-desktop-database >/dev/null 2>&1 && \
        update-desktop-database "$APP_DIR" 2>/dev/null || true
fi
if [[ $DESKTOP_SHORTCUT -eq 1 ]]; then
    write_desktop_entry "$DESKTOP_DIR/$DESKTOP_NAME"
    # KDE and GNOME both refuse to launch a desktop file they do not trust.
    command -v gio >/dev/null 2>&1 && \
        gio set "$DESKTOP_DIR/$DESKTOP_NAME" metadata::trusted true 2>/dev/null || true
    say "    $DESKTOP_DIR/$DESKTOP_NAME"
fi
if [[ $AUTOSTART_TRAY -eq 1 ]]; then
    write_desktop_entry "$AUTOSTART_DIR/$DESKTOP_NAME"
    say "    $AUTOSTART_DIR/$DESKTOP_NAME (starts the tray at login)"
fi

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) say
       say "note: $BIN_DIR is not on your PATH, so \`yourls\` will not be found."
       say "      Add it in your shell's rc file:  export PATH=\"\$PATH:$BIN_DIR\"" ;;
esac

say "==> done. Try: yourls https://example.com"
