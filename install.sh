#!/usr/bin/env bash
set -euo pipefail

SOURCES="${BASE_URL:-https://linusware.org/dl https://linusware.pages.dev/dl}"
INSTALL_DIR="${LINUSWARE_DIR:-${XDG_DATA_HOME:-${HOME}/.local/share}/linusware}"
BIN_DIR="${BIN_DIR:-${HOME}/.local/bin}"

LogLine() {
  printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" "$1"
}

Info() {
  LogLine "$1"
}
Warn() {
  LogLine "$1"
}
Error() {
  LogLine "$1"
  exit 1
}

command -v curl >/dev/null 2>&1 || Error "curl is required"
command -v sha256sum >/dev/null 2>&1 || Error "sha256sum is required"
[ "$(uname -m)" = "x86_64" ] || Error "only x86_64 is supported"

case "$(cd "$INSTALL_DIR" 2>/dev/null && pwd -P || echo "$INSTALL_DIR")" in
  /|"$HOME"|"$(cd "$HOME" && pwd -P)") Error "Refusing to install into $INSTALL_DIR, pick a folder of its own" ;;
esac
if [ -d "$INSTALL_DIR" ] && [ -n "$(ls -A "$INSTALL_DIR")" ] \
   && [ ! -e "$INSTALL_DIR/external" ] && [ ! -e "$INSTALL_DIR/lwlauncher" ]; then
  Error "$INSTALL_DIR is not empty and is not a LinusWare install"
fi

mkdir -p "$INSTALL_DIR" "$BIN_DIR"
export LINUSWARE_DIR="$INSTALL_DIR"

Info "Downloading the launcher..."
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
got=""
for base in $SOURCES; do
  want=$(curl -fsSL "${base}/launcher/bin/SHA256SUM" 2>/dev/null | cut -c1-64) || continue
  [ -n "$want" ] || continue
  curl -fSL --progress-bar -o "$tmp" "${base}/launcher/bin/lwlauncher" || continue
  [ "$(sha256sum "$tmp" | cut -d' ' -f1)" = "$want" ] && { got="$base"; break; }
  Warn "SHA256 mismatch for the launcher from ${base}"
done
[ -n "$got" ] || Error "Could not download the launcher"
chmod +x "$tmp"
mv "$tmp" "${INSTALL_DIR}/lwlauncher"
ln -sfn "${INSTALL_DIR}/lwlauncher" "${BIN_DIR}/lw"
"${INSTALL_DIR}/lwlauncher" --version >/dev/null

Info "Installed $("${INSTALL_DIR}/lwlauncher" --version)"
echo ""
case ":${PATH}:" in
  *":${BIN_DIR}:"*)
    echo "  Run:  lw"
    ;;
  *)
    Warn "${BIN_DIR} is not in your PATH"
    echo "  bash/zsh:  echo 'export PATH=\"${BIN_DIR}:\$PATH\"' >> ~/.profile"
    echo "  fish:      fish_add_path ${BIN_DIR}"
    echo "  or run it directly:  ${BIN_DIR}/lw"
    ;;
esac
echo "  LinusWare is also in your app menu."
