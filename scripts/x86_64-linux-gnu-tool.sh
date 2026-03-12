#!/usr/bin/env bash

set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <gcc|ar|strip> [args...]" >&2
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLCHAIN_ROOT="${UNIVERS_X86_64_TOOLCHAIN_ROOT:-$ROOT_DIR/.toolchains/x86_64-linux-gnu}"

if [[ ! -x "$TOOLCHAIN_ROOT/usr/bin/x86_64-linux-gnu-gcc" ]]; then
  TOOLCHAIN_ROOT="$("$ROOT_DIR/scripts/setup-x86_64-cross-toolchain.sh")"
fi

export LD_LIBRARY_PATH="$TOOLCHAIN_ROOT/usr/lib/aarch64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

tool="$1"
shift

case "$tool" in
  gcc)
    exec "$TOOLCHAIN_ROOT/usr/bin/x86_64-linux-gnu-gcc" --sysroot="$TOOLCHAIN_ROOT" "$@"
    ;;
  ar)
    exec "$TOOLCHAIN_ROOT/usr/bin/x86_64-linux-gnu-ar" "$@"
    ;;
  strip)
    exec "$TOOLCHAIN_ROOT/usr/bin/x86_64-linux-gnu-strip" "$@"
    ;;
  *)
    echo "unsupported x86_64 cross tool: $tool" >&2
    exit 1
    ;;
esac
