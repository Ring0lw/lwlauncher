#!/usr/bin/env bash

INSTALL_DIR="${LINUSWARE_DIR:-${XDG_DATA_HOME:-${HOME}/.local/share}/linusware}"
SOURCES="${BASE_URL:-https://linusware.org/dl https://linusware.pages.dev/dl}"
BIN="${INSTALL_DIR}/lwlauncher"

if [ ! -x "$BIN" ]; then
  mkdir -p "$INSTALL_DIR"
  tmp="$(mktemp)"
  for base in $SOURCES; do
    want=$(curl -fsSL "${base}/launcher/bin/MD5SUM" | cut -c1-32)
    if curl -fsSL -o "$tmp" "${base}/launcher/bin/lwlauncher" && [ "$(md5sum "$tmp" | cut -d' ' -f1)" = "$want" ]; then
      chmod +x "$tmp" && mv "$tmp" "$BIN"
      mkdir -p "${HOME}/.local/bin" && ln -sfn "$BIN" "${HOME}/.local/bin/lw"
      break
    fi
  done
  if [ ! -x "$BIN" ]; then
    rm -f "$tmp"
    echo "Could not download the LinusWare launcher" >&2
    exit 1
  fi
fi

exec "$BIN" "$@"
