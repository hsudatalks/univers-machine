#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="$ROOT_DIR/apps/univers-ark-developer/src-tauri/resources/daemons"
X86_CARGO_WRAPPER="$ROOT_DIR/scripts/cargo-x86_64-unknown-linux-gnu.sh"
X86_STRIP_WRAPPER="$ROOT_DIR/scripts/x86_64-linux-gnu-strip"

mkdir -p "$OUTPUT_DIR"

build_binary() {
  local crate_name="$1"
  local target_triple="$2"

  if [[ "$target_triple" == "x86_64-unknown-linux-gnu" ]]; then
    "$X86_CARGO_WRAPPER" build \
      --manifest-path "$ROOT_DIR/Cargo.toml" \
      -p "$crate_name" \
      --release \
      --target "$target_triple"
    return
  fi

  cargo build \
    --manifest-path "$ROOT_DIR/Cargo.toml" \
    -p "$crate_name" \
    --release \
    --target "$target_triple"
}

strip_binary() {
  local binary_path="$1"
  local target_triple="$2"

  if [[ "$target_triple" == "x86_64-unknown-linux-gnu" ]]; then
    "$X86_STRIP_WRAPPER" "$binary_path"
    return
  fi

  strip "$binary_path"
}

build_asset() {
  local crate_name="$1"
  local target_triple="$2"
  local binary_path="$ROOT_DIR/target/$target_triple/release/$crate_name"
  local archive_path="$OUTPUT_DIR/${crate_name}-${target_triple}.tar.gz"

  build_binary "$crate_name" "$target_triple"

  if [[ ! -f "$binary_path" ]]; then
    echo "Expected binary not found: $binary_path" >&2
    exit 1
  fi

  strip_binary "$binary_path" "$target_triple"
  tar -C "$(dirname "$binary_path")" -czf "$archive_path" "$(basename "$binary_path")"
  ls -lh "$binary_path" "$archive_path"
}

build_asset "univers-machine-daemon" "x86_64-unknown-linux-gnu"
build_asset "univers-machine-daemon" "aarch64-unknown-linux-gnu"
build_asset "univers-container-daemon" "x86_64-unknown-linux-gnu"
build_asset "univers-container-daemon" "aarch64-unknown-linux-gnu"
