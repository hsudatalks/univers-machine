#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$ROOT_DIR/apps/univers-ark-developer"
BUNDLE_DIR="$ROOT_DIR/target/release/bundle/deb"

cd "$ROOT_DIR"
"$ROOT_DIR/scripts/build-ark-daemon-assets.sh"

cd "$APP_DIR"
pnpm tauri build

find "$BUNDLE_DIR" -maxdepth 1 -type f -name '*.deb' -print | sort
