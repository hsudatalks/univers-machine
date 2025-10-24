#!/bin/bash
# Clone a VM with optional cleanup
# Usage: ./clone-vm.sh <source-vm> <new-vm-name> [--cleanup]

set -e

SOURCE_VM="$1"
NEW_VM="$2"
CLEANUP="${3:-}"

if [ -z "$SOURCE_VM" ] || [ -z "$NEW_VM" ]; then
    echo "Usage: $0 <source-vm> <new-vm-name> [--cleanup]"
    echo
    echo "Options:"
    echo "  --cleanup    Clean up source VM before cloning"
    echo
    echo "Example:"
    echo "  $0 ubuntu web-dev --cleanup"
    exit 1
fi

echo "=== Cloning VM: $SOURCE_VM → $NEW_VM ==="
echo

# Check if source VM exists
if ! orb list | grep -q "^$SOURCE_VM "; then
    echo "Error: Source VM '$SOURCE_VM' not found"
    echo
    echo "Available VMs:"
    orb list
    exit 1
fi

# Check if target VM already exists
if orb list | grep -q "^$NEW_VM "; then
    echo "Error: VM '$NEW_VM' already exists"
    exit 1
fi

# Optional cleanup
if [ "$CLEANUP" = "--cleanup" ]; then
    echo "Step 1: Cleaning up source VM..."
    ./cleanup-dev.sh "$SOURCE_VM"
    echo
fi

# Clone the VM
echo "Step 2: Cloning $SOURCE_VM to $NEW_VM..."
orb clone "$SOURCE_VM" "$NEW_VM"

echo
echo "✅ Clone complete!"
echo
echo "New VM created: $NEW_VM"
orb list | grep "^$NEW_VM "

echo
echo "To access the new VM:"
echo "  orb shell $NEW_VM"
