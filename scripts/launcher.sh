#!/usr/bin/env bash

INSTALL_DIR="${LINUSWARE_DIR:-${XDG_DATA_HOME:-${HOME}/.local/share}/linusware}"
BASE_URL="${BASE_URL:-https://git.catapis.uk/linusware/main/raw/branch/main}"
BIN="${INSTALL_DIR}/lwlauncher"

if [ ! -x "$BIN" ]; then
  mkdir -p "$INSTALL_DIR"
  tmp="$(mktemp)"
  want=$(curl -fsSL "${BASE_URL}/launcher/bin/MD5SUM" | tr -cd '[:xdigit:]')
  if curl -fsSL -o "$tmp" "${BASE_URL}/launcher/bin/lwlauncher" && [ "$(md5sum "$tmp" | cut -d' ' -f1)" = "$want" ]; then
    chmod +x "$tmp" && mv "$tmp" "$BIN"
  else
    rm -f "$tmp"
    echo "Could not download the LinusWare launcher" >&2
    exit 1
  fi
fi

exec "$BIN" "$@"
