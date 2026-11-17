#!/bin/sh
set -e
cd "$(dirname "$0")"

command -v cargo-zigbuild >/dev/null 2>&1 || { echo "dist: cargo-zigbuild not found (cargo install cargo-zigbuild)"; exit 1; }
command -v rsign >/dev/null 2>&1 || { echo "dist: rsign not found (cargo install rsign2)"; exit 1; }
key="${LW_SIGN_KEY:-$HOME/.minisign/linusware.key}"
[ -f "$key" ] || { echo "dist: no signing key at $key"; exit 1; }

T=x86_64-unknown-linux-gnu
cargo zigbuild --release --target $T.2.17

ver=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
rm -rf dist
mkdir -p dist/dl/launcher/bin
cp target/$T/release/lwlauncher dist/lwlauncher
cp install.sh dist/install.sh
cp dist/lwlauncher dist/dl/launcher/bin/lwlauncher
sha256sum dist/lwlauncher | cut -d' ' -f1 > dist/dl/launcher/bin/SHA256SUM
md5sum dist/lwlauncher | cut -d' ' -f1 > dist/dl/launcher/bin/MD5SUM # 1.0.5 and older self update with this
rsign sign -W -s "$key" -t "launcher $ver" -x dist/dl/launcher/bin/lwlauncher.minisig dist/lwlauncher >/dev/null
rsign verify -p assets/linusware.pub -x dist/dl/launcher/bin/lwlauncher.minisig dist/lwlauncher >/dev/null 2>&1 || { echo "dist: $key isnt the key in assets/linusware.pub"; exit 1; }
printf '{\n    "version": "%s",\n    "updated": true,\n    "log": ""\n}\n' "$ver" > dist/dl/launcher/status.json
cp install.sh scripts/launcher.sh scripts/update.sh dist/dl/

echo "dist: ok v$ver ($(du -h dist/lwlauncher | cut -f1), glibc $(objdump -T dist/lwlauncher | grep -oE 'GLIBC_[0-9.]+' | sort -uV | tail -1))"
