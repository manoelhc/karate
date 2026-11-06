#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
version=v26.13.0
arch=$(uname -m)
case "$arch" in x86_64) arch=x64 ;; aarch64) arch=arm64 ;; esac
local_bin="$root/.local/node-download/node-$version-$(uname -s | tr '[:upper:]' '[:lower:]')-$arch/bin"
if [ -x "$local_bin/node" ]; then export PATH="$local_bin:$PATH"; fi
if [ "$(node --version)" != "$version" ]; then
    printf '%s\n' "Node $version is required (see web/README.md)." >&2; exit 1
fi
exec "$@"
