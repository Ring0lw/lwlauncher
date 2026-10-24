#!/usr/bin/env bash

INSTALL_DIR="${LINUSWARE_DIR:-${XDG_DATA_HOME:-${HOME}/.local/share}/linusware}"
REPO="https://github.com/Ring0lw/lwlauncher"
BIN="${INSTALL_DIR}/lwlauncher"

if [ ! -x "$BIN" ]; then
  mkdir -p "$INSTALL_DIR"
  tmp="$(mktemp)"
  want=$(curl -fsSL "${REPO}/releases/latest/download/lwlauncher.md5" | cut -c1-32)
  if curl -fsSL -o "$tmp" "${REPO}/releases/latest/download/lwlauncher" && [ "$(md5sum "$tmp" | cut -d' ' -f1)" = "$want" ]; then
    chmod +x "$tmp" && mv "$tmp" "$BIN"
    mkdir -p "${HOME}/.local/bin" && ln -sfn "$BIN" "${HOME}/.local/bin/lw"
  else
    rm -f "$tmp"
    echo "Could not download the LinusWare launcher" >&2
    exit 1
  fi
fi

exec "$BIN" --update "$@"
