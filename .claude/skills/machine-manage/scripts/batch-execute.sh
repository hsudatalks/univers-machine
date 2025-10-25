#!/bin/bash
# Execute a command across multiple containers/VMs
# Supports both LXD (Linux) and OrbStack (macOS)
# Usage: ./batch-execute.sh <command> [container1 container2...] or [--all]

set -e

# Source the helper library
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$SCRIPT_DIR/lib/container-helper.sh"

COMMAND="$1"
shift

if [ -z "$COMMAND" ]; then
    echo "Usage: $0 <command> [container1 container2...] or [--all]"
    echo
    echo "Examples:"
    echo "  $0 'uptime' --all"
    echo "  $0 'df -h /' ubuntu debian"
    echo "  $0 'apt-get update' web-dev integration-dev"
    exit 1
fi

echo "=== Batch Execute: $COMMAND ==="
echo "System: $(print_system_info)"
echo

# Determine target containers
if [ "$1" = "--all" ]; then
    # Get all running containers
    CONTAINER_SYSTEM="$(detect_container_system)"
    case "$CONTAINER_SYSTEM" in
        lxd)
            TARGET_CONTAINERS=$(lxc list --format=json | jq -r '.[] | select(.status == "Running") | .name' 2>/dev/null)
            ;;
        orbstack)
            TARGET_CONTAINERS=$(orb list | grep " running " | awk '{print $1}')
            ;;
    esac
else
    TARGET_CONTAINERS="$@"
fi

if [ -z "$TARGET_CONTAINERS" ]; then
    echo "Error: No containers specified and no running containers found"
    exit 1
fi

echo "Target containers: $TARGET_CONTAINERS"
echo

for CONTAINER in $TARGET_CONTAINERS; do
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Container: $CONTAINER"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    if container_is_running "$CONTAINER"; then
        container_exec "$CONTAINER" bash -c "$COMMAND" 2>&1 || print_warning "Command failed on $CONTAINER"
    else
        print_warning "Container $CONTAINER is not running, skipping..."
    fi

    echo
done

print_success "Batch execution complete!"
