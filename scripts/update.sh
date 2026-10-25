#!/usr/bin/env bash

INSTALL_DIR="${LINUSWARE_DIR:-${XDG_DATA_HOME:-${HOME}/.local/share}/linusware}"
REPO="https://github.com/Ring0lw/lwlauncher"
BIN="${INSTALL_DIR}/lwlauncher"

if [ ! -x "$BIN" ]; then
  mkdir -p "$INSTALL_DIR"
  tmp="$(mktemp)"
  tag=$(curl -fsSL -H "Accept: application/vnd.github+json" "https://api.github.com/repos/Ring0lw/lwlauncher/releases/latest" 2>/dev/null | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)
  if [ -n "$tag" ]; then url="${REPO}/releases/download/${tag}"; else url="${REPO}/releases/latest/download"; fi
  want=$(curl -fsSL "${url}/lwlauncher.md5" | cut -c1-32)
  if curl -fsSL -o "$tmp" "${url}/lwlauncher" && [ "$(md5sum "$tmp" | cut -d' ' -f1)" = "$want" ]; then
    chmod +x "$tmp" && mv "$tmp" "$BIN"
    mkdir -p "${HOME}/.local/bin" && ln -sfn "$BIN" "${HOME}/.local/bin/lw"
  else
    rm -f "$tmp"
    echo "Could not download the LinusWare launcher" >&2
    exit 1
  fi
fi

exec "$BIN" --update "$@"
