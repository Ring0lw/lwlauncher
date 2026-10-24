#!/bin/sh
set -e
cd "$(dirname "$0")"

command -v cargo-zigbuild >/dev/null 2>&1 || { echo "dist: cargo-zigbuild not found (cargo install cargo-zigbuild)"; exit 1; }

T=x86_64-unknown-linux-gnu
cargo zigbuild --release --target $T.2.17

ver=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
log="${1:-New launcher}"
out=dist/launcher
rm -rf "$out"
mkdir -p "$out/bin"
cp target/$T/release/lwlauncher "$out/bin/lwlauncher"
md5sum "$out/bin/lwlauncher" | cut -d' ' -f1 > "$out/bin/MD5SUM"
printf '{\n    "version": "%s",\n    "updated": true,\n    "log": "%s"\n}\n' "$ver" "$log" > "$out/status.json"
cp scripts/launcher.sh scripts/update.sh scripts/install.sh "$out/"

echo "dist: ok $out ($(du -h "$out/bin/lwlauncher" | cut -f1), glibc $(objdump -T "$out/bin/lwlauncher" | grep -oE 'GLIBC_[0-9.]+' | sort -uV | tail -1))"
echo "dist: copy $out/{status.json,bin/} to launcher/ in the repo, and launcher.sh update.sh install.sh to the repo root"
