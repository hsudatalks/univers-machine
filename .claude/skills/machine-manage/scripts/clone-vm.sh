#!/bin/bash
# Clone a container/VM with optional cleanup
# Supports both LXD (Linux) and OrbStack (macOS)
# Usage: ./clone-vm.sh <source-container> <new-container-name> [--cleanup]

set -e

# Source the helper library
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$SCRIPT_DIR/lib/container-helper.sh"

SOURCE_CONTAINER="$1"
NEW_CONTAINER="$2"
CLEANUP="${3:-}"

if [ -z "$SOURCE_CONTAINER" ] || [ -z "$NEW_CONTAINER" ]; then
    echo "Usage: $0 <source-container> <new-container-name> [--cleanup]"
    echo
    echo "Options:"
    echo "  --cleanup    Clean up source container before cloning"
    echo
    echo "Example:"
    echo "  $0 ubuntu web-dev --cleanup"
    exit 1
fi

echo "=== Cloning Container/VM: $SOURCE_CONTAINER → $NEW_CONTAINER ==="
echo "System: $(print_system_info)"
echo

# Check if source container exists
if ! container_exists "$SOURCE_CONTAINER"; then
    echo "Error: Source container/VM '$SOURCE_CONTAINER' not found"
    echo
    echo "Available containers/VMs:"
    container_list
    exit 1
fi

# Check if target container already exists
if container_exists "$NEW_CONTAINER"; then
    echo "Error: Container/VM '$NEW_CONTAINER' already exists"
    exit 1
fi

# Optional cleanup
if [ "$CLEANUP" = "--cleanup" ]; then
    echo "Step 1: Cleaning up source container..."
    "$SCRIPT_DIR/scripts/cleanup-dev.sh" "$SOURCE_CONTAINER"
    echo
fi

# Clone the container
echo "Step 2: Cloning $SOURCE_CONTAINER to $NEW_CONTAINER..."
container_clone "$SOURCE_CONTAINER" "$NEW_CONTAINER"

echo
print_success "Clone complete!"
echo

# Show the new container
echo "New container/VM created: $NEW_CONTAINER"
CONTAINER_SYSTEM="$(detect_container_system)"
case "$CONTAINER_SYSTEM" in
    lxd)
        lxc list --format=json | jq -r ".[] | select(.name == \"$NEW_CONTAINER\") | \"\(.name) \(.status)\""
        echo
        echo "To access the new container:"
        echo "  lxc exec $NEW_CONTAINER -- /bin/bash"
        ;;
    orbstack)
        orb list | grep "^$NEW_CONTAINER "
        echo
        echo "To access the new VM:"
        echo "  orb shell $NEW_CONTAINER"
        ;;
esac
