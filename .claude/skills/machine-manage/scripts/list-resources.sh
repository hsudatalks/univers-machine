#!/bin/bash
# List resource usage across all containers/VMs
# Supports both LXD (Linux) and OrbStack (macOS)
# Usage: ./list-resources.sh

set -e

# Source the helper library
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$SCRIPT_DIR/lib/container-helper.sh"

echo "=== Container/VM Resource Usage ==="
echo "System: $(print_system_info)"
echo

# Get list of containers/VMs
CONTAINER_SYSTEM="$(detect_container_system)"
case "$CONTAINER_SYSTEM" in
    lxd)
        # Get running LXD containers
        CONTAINERS=$(lxc list --format=json | jq -r '.[] | select(.status == "Running") | .name' 2>/dev/null)
        ;;
    orbstack)
        # Get running OrbStack VMs
        CONTAINERS=$(orb list | grep " running " | awk '{print $1}')
        ;;
    *)
        echo "Error: No container system detected"
        exit 1
        ;;
esac

# Process each container
while IFS= read -r CONTAINER; do
    [ -z "$CONTAINER" ] && continue

    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Container: $CONTAINER"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    # Get disk usage
    echo "📊 Disk Usage:"
    container_exec "$CONTAINER" df -h / 2>/dev/null || echo "  (could not retrieve)"

    echo

    # Get memory info
    echo "💾 Memory:"
    container_exec "$CONTAINER" free -h 2>/dev/null || echo "  (could not retrieve)"

    echo

    # Check for large directories
    echo "📁 Large directories (>100MB):"
    container_exec "$CONTAINER" bash -c "du -h --max-depth=3 ~ 2>/dev/null | grep -E '^[0-9.]+G|^[0-9]{3,}M' | sort -hr | head -10" 2>/dev/null || echo "  (could not retrieve)"

    echo
done <<< "$CONTAINERS"

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Summary:"
container_list
