#!/bin/bash
# Cleanup development artifacts and caches from a VM
# Usage: ./cleanup-dev.sh <vm-name> [--dry-run]

set -e

VM_NAME="$1"
DRY_RUN="${2:-}"

if [ -z "$VM_NAME" ]; then
    echo "Usage: $0 <vm-name> [--dry-run]"
    exit 1
fi

echo "=== Cleanup Development Environment: $VM_NAME ==="
echo

# Build cleanup command
CLEANUP_CMD='
set -x

# Show disk usage before
echo "=== Disk usage BEFORE cleanup ==="
df -h /

# Rust/Cargo build artifacts
echo "=== Cleaning Rust build artifacts ==="
find ~ -type d -name "target" -path "*/*/target" 2>/dev/null | while read dir; do
    echo "Found: $dir"
    du -sh "$dir" 2>/dev/null || true
done

# Clean target directories (only in project folders)
find ~ -type d -name "target" -path "*/*/target" -exec rm -rf {} + 2>/dev/null || true

# sccache cleanup
echo "=== Cleaning sccache ==="
if command -v sccache &> /dev/null; then
    sccache --stop-server 2>/dev/null || true
    rm -rf ~/.cache/sccache/* 2>/dev/null || true
fi

# Cargo cache (optional - keeps downloaded dependencies but cleans registry index)
echo "=== Cleaning Cargo cache ==="
rm -rf ~/.cargo/registry/index/* 2>/dev/null || true
rm -rf ~/.cargo/registry/cache/* 2>/dev/null || true
rm -rf ~/.cargo/git/db/* 2>/dev/null || true

# System package caches
echo "=== Cleaning system caches ==="
sudo apt-get clean 2>/dev/null || true
sudo rm -rf /var/cache/apt/archives/*.deb 2>/dev/null || true

# Temporary files
echo "=== Cleaning temporary files ==="
sudo rm -rf /tmp/* 2>/dev/null || true
rm -rf ~/.cache/pip/* 2>/dev/null || true

# Docker cleanup (if Docker is installed)
if command -v docker &> /dev/null; then
    echo "=== Docker cleanup ==="
    docker system prune -f --volumes 2>/dev/null || true
fi

# Show disk usage after
echo "=== Disk usage AFTER cleanup ==="
df -h /

echo "=== Cleanup complete! ==="
'

if [ "$DRY_RUN" = "--dry-run" ]; then
    echo "DRY RUN - Would execute the following on $VM_NAME:"
    echo "$CLEANUP_CMD"
else
    echo "Executing cleanup on $VM_NAME..."
    orb run "$VM_NAME" bash -c "$CLEANUP_CMD"
fi

echo
echo "Done!"
