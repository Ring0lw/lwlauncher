#!/usr/bin/env bash
set -euo pipefail

REPO="https://github.com/Ring0lw/lwlauncher"
INSTALL_DIR="${LINUSWARE_DIR:-${XDG_DATA_HOME:-${HOME}/.local/share}/linusware}"
BIN_DIR="${BIN_DIR:-${HOME}/.local/bin}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() {
  echo -e "${GREEN}[*]${NC} $1"
}
warn() {
  echo -e "${YELLOW}[!]${NC} $1"
}
error() {
  echo -e "${RED}[x]${NC} $1"
  exit 1
}

command -v curl >/dev/null 2>&1 || error "curl is required"
command -v md5sum >/dev/null 2>&1 || error "md5sum is required"
[ "$(uname -m)" = "x86_64" ] || error "only x86_64 is supported"

case "$(cd "$INSTALL_DIR" 2>/dev/null && pwd -P || echo "$INSTALL_DIR")" in
  /|"$HOME"|"$(cd "$HOME" && pwd -P)") error "Refusing to install into $INSTALL_DIR, pick a folder of its own" ;;
esac
if [ -d "$INSTALL_DIR" ] && [ -n "$(ls -A "$INSTALL_DIR")" ] \
   && [ ! -e "$INSTALL_DIR/external" ] && [ ! -e "$INSTALL_DIR/lwlauncher" ]; then
  error "$INSTALL_DIR is not empty and is not a LinusWare install"
fi

mkdir -p "$INSTALL_DIR" "$BIN_DIR"
export LINUSWARE_DIR="$INSTALL_DIR"

info "Downloading the launcher..."
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
tag=$(curl -fsSL -H "Accept: application/vnd.github+json" "https://api.github.com/repos/Ring0lw/lwlauncher/releases/latest" 2>/dev/null | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1) || true
if [ -n "$tag" ]; then url="${REPO}/releases/download/${tag}"; else url="${REPO}/releases/latest/download"; fi
want=$(curl -fsSL "${url}/lwlauncher.md5" | cut -c1-32) || error "Could not fetch lwlauncher.md5"
curl -fSL --progress-bar -o "$tmp" "${url}/lwlauncher" || error "Could not download the launcher"
[ "$(md5sum "$tmp" | cut -d' ' -f1)" = "$want" ] || error "MD5 mismatch for the launcher"
chmod +x "$tmp"
mv "$tmp" "${INSTALL_DIR}/lwlauncher"
ln -sfn "${INSTALL_DIR}/lwlauncher" "${BIN_DIR}/lw"
"${INSTALL_DIR}/lwlauncher" --version >/dev/null

info "Installed $("${INSTALL_DIR}/lwlauncher" --version)"
echo ""
case ":${PATH}:" in
  *":${BIN_DIR}:"*)
    echo "  Run:  lw"
    ;;
  *)
    warn "${BIN_DIR} is not in your PATH"
    echo "  bash/zsh:  echo 'export PATH=\"${BIN_DIR}:\$PATH\"' >> ~/.profile"
    echo "  fish:      fish_add_path ${BIN_DIR}"
    echo "  or run it directly:  ${BIN_DIR}/lw"
    ;;
esac
echo "  LinusWare is also in your app menu."
