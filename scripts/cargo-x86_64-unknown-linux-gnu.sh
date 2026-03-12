#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

"$ROOT_DIR/scripts/setup-x86_64-cross-toolchain.sh" >/dev/null

export CC_x86_64_unknown_linux_gnu="$ROOT_DIR/scripts/x86_64-linux-gnu-gcc"
export AR_x86_64_unknown_linux_gnu="$ROOT_DIR/scripts/x86_64-linux-gnu-ar"
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$ROOT_DIR/scripts/x86_64-linux-gnu-gcc"

exec cargo "$@"
