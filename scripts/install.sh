#!/usr/bin/env bash
# Installs yourls for the current user: the binary, a menu entry, and an
# autostart entry for the tray.
#
# Everything goes under $HOME. No root, no system directories — a URL
# shortener has no business needing either.
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

BIN_DIR="${YOURLS_BIN_DIR:-$HOME/.local/bin}"
APP_DIR="${YOURLS_APP_DIR:-$HOME/.local/share/applications}"
ICON_DIR="${YOURLS_ICON_DIR:-$HOME/.local/share/icons/hicolor/256x256/apps}"
AUTOSTART_DIR="${YOURLS_AUTOSTART_DIR:-$HOME/.config/autostart}"

DESKTOP_NAME=yourls.desktop
AUTOSTART=0
SOURCE=""

usage() {
    cat <<'USAGE'
Usage: install.sh [--autostart] [--binary <path>]
       install.sh --uninstall

  --autostart      also start the tray when you log in
  --binary <path>  install this binary instead of building one
  --uninstall      remove everything this script installed

Environment: YOURLS_BIN_DIR, YOURLS_APP_DIR, YOURLS_ICON_DIR,
YOURLS_AUTOSTART_DIR override where things go.
USAGE
}

uninstall() {
    local removed=0
    for path in "$BIN_DIR/yourls" "$BIN_DIR/yourls-cli" \
                "$APP_DIR/$DESKTOP_NAME" "$AUTOSTART_DIR/$DESKTOP_NAME" \
                "$ICON_DIR/yourls.png"; do
        if [[ -e "$path" ]]; then
            rm -f "$path"
            echo "removed $path"
            removed=1
        fi
    done
    [[ $removed -eq 0 ]] && echo "nothing was installed"
    command -v update-desktop-database >/dev/null && \
        update-desktop-database "$APP_DIR" 2>/dev/null || true
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --autostart) AUTOSTART=1 ;;
        --binary) shift; SOURCE="${1:-}" ;;
        --uninstall) uninstall ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
    shift
done

cd "$PROJECT_DIR"

if [[ -z "$SOURCE" ]]; then
    SOURCE="$PROJECT_DIR/target/release/yourls"
    if [[ ! -x "$SOURCE" ]]; then
        echo "==> building (no binary at $SOURCE)"
        cargo build --release --bin yourls
    fi
fi
if [[ ! -x "$SOURCE" ]]; then
    echo "no binary at $SOURCE" >&2
    exit 1
fi

echo "==> installing"
install -Dm755 "$SOURCE" "$BIN_DIR/yourls"
install -Dm644 "$PROJECT_DIR/src/icon.png" "$ICON_DIR/yourls.png"
install -Dm644 "$PROJECT_DIR/packaging/$DESKTOP_NAME" "$APP_DIR/$DESKTOP_NAME"

# An absolute Exec, because a desktop session's PATH often lacks
# ~/.local/bin — the entry would then do nothing at all, silently.
sed -i "s|^Exec=yourls --tray$|Exec=$BIN_DIR/yourls --tray|" "$APP_DIR/$DESKTOP_NAME"

echo "    $BIN_DIR/yourls"
echo "    $APP_DIR/$DESKTOP_NAME"

if [[ $AUTOSTART -eq 1 ]]; then
    install -Dm644 "$APP_DIR/$DESKTOP_NAME" "$AUTOSTART_DIR/$DESKTOP_NAME"
    echo "    $AUTOSTART_DIR/$DESKTOP_NAME (starts the tray at login)"
else
    echo "    (no autostart; pass --autostart for that)"
fi

command -v update-desktop-database >/dev/null && \
    update-desktop-database "$APP_DIR" 2>/dev/null || true

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo
       echo "note: $BIN_DIR is not on your PATH, so \`yourls\` will not be found."
       echo "      Add it in your shell's rc file:  export PATH=\"\$PATH:$BIN_DIR\"" ;;
esac

echo "==> done. Try: yourls https://example.com"
