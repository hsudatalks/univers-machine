#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLCHAIN_ROOT="${UNIVERS_X86_64_TOOLCHAIN_ROOT:-$ROOT_DIR/.toolchains/x86_64-linux-gnu}"
STAMP_PATH="$TOOLCHAIN_ROOT/.setup-complete"

PACKAGES=(
  binutils-common
  binutils-x86-64-linux-gnu
  cpp-13-x86-64-linux-gnu
  cpp-x86-64-linux-gnu
  gcc-13-cross-base
  gcc-13-x86-64-linux-gnu
  gcc-13-x86-64-linux-gnu-base
  gcc-x86-64-linux-gnu
  libasan8-amd64-cross
  libatomic1-amd64-cross
  libc6-amd64-cross
  libc6-dev-amd64-cross
  libgcc-13-dev-amd64-cross
  libgcc-s1-amd64-cross
  libgomp1-amd64-cross
  libhwasan0-amd64-cross
  libitm1-amd64-cross
  liblsan0-amd64-cross
  libquadmath0-amd64-cross
  libtsan2-amd64-cross
  libubsan1-amd64-cross
  linux-libc-dev-amd64-cross
)

if [[ -f "$STAMP_PATH" ]] && [[ -x "$TOOLCHAIN_ROOT/usr/bin/x86_64-linux-gnu-gcc" ]]; then
  printf '%s\n' "$TOOLCHAIN_ROOT"
  exit 0
fi

tmpdir="$(mktemp -d)"
staging_dir="$tmpdir/root"
trap 'rm -rf "$tmpdir"' EXIT

mkdir -p "$staging_dir"

(
  cd "$tmpdir"
  apt-get download "${PACKAGES[@]}" >/dev/null
  for deb in ./*.deb; do
    dpkg-deb -x "$deb" "$staging_dir"
  done
)

rm -rf "$TOOLCHAIN_ROOT"
mkdir -p "$(dirname "$TOOLCHAIN_ROOT")"
mv "$staging_dir" "$TOOLCHAIN_ROOT"
touch "$STAMP_PATH"

printf '%s\n' "$TOOLCHAIN_ROOT"
