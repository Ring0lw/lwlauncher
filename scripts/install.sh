#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-https://git.catapis.uk/linusware/main/raw/branch/main}"

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

info() {
  echo -e "${GREEN}[*]${NC} $1"
}
error() {
  echo -e "${RED}[x]${NC} $1"
  exit 1
}

INSTALL_DIR="${LINUSWARE_DIR:-${XDG_DATA_HOME:-${HOME}/.local/share}/linusware}"

command -v curl >/dev/null 2>&1 || error "curl is required"
command -v md5sum >/dev/null 2>&1 || error "md5sum is required"

case "$(cd "$INSTALL_DIR" 2>/dev/null && pwd -P || echo "$INSTALL_DIR")" in
  /|"$HOME"|"$(cd "$HOME" && pwd -P)") error "Refusing to install into $INSTALL_DIR, pick a folder of its own" ;;
esac
if [ -d "$INSTALL_DIR" ] && [ -n "$(ls -A "$INSTALL_DIR")" ] \
   && [ ! -e "$INSTALL_DIR/external" ] && [ ! -e "$INSTALL_DIR/launcher.sh" ] && [ ! -e "$INSTALL_DIR/lwlauncher" ]; then
  error "$INSTALL_DIR is not empty and is not a LinusWare install"
fi

mkdir -p "$INSTALL_DIR"
export LINUSWARE_DIR="$INSTALL_DIR"

info "Downloading the launcher..."
tmp="$(mktemp)"
want=$(curl -fsSL "${BASE_URL}/launcher/bin/MD5SUM" | tr -cd '[:xdigit:]') || error "Could not fetch the launcher MD5SUM"
curl -fsSL -o "$tmp" "${BASE_URL}/launcher/bin/lwlauncher" || { rm -f "$tmp"; error "Could not download the launcher"; }
[ "$(md5sum "$tmp" | cut -d' ' -f1)" = "$want" ] || { rm -f "$tmp"; error "MD5 mismatch for the launcher"; }
chmod +x "$tmp"
mv "$tmp" "${INSTALL_DIR}/lwlauncher"

"${INSTALL_DIR}/lwlauncher" --install

echo ""
echo "  Open LinusWare from your app menu, or run:  ${INSTALL_DIR}/lwlauncher"
