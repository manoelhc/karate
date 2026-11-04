#!/bin/sh
set -eu
# Homebrew's cargo/rustc can precede rustup shims; pin both subprocess tools.
compiler=$(rustup which --toolchain 1.99.1 rustc)
toolchain_bin=$(dirname "$compiler")
export PATH="$toolchain_bin:$PATH"
export RUSTC="$compiler"
export RUSTDOC="$toolchain_bin/rustdoc"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
exec "$toolchain_bin/cargo" "$@"
