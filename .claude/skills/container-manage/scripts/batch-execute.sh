#!/bin/bash
# Execute a command across multiple VMs
# Usage: ./batch-execute.sh <command> [vm1 vm2 vm3...] or [--all]

set -e

COMMAND="$1"
shift

if [ -z "$COMMAND" ]; then
    echo "Usage: $0 <command> [vm1 vm2 vm3...] or [--all]"
    echo
    echo "Examples:"
    echo "  $0 'uptime' --all"
    echo "  $0 'df -h /' ubuntu debian"
    echo "  $0 'apt-get update' web-dev integration-dev"
    exit 1
fi

# Determine target VMs
if [ "$1" = "--all" ]; then
    TARGET_VMS=$(orb list | grep " running " | awk '{print $1}')
else
    TARGET_VMS="$@"
fi

if [ -z "$TARGET_VMS" ]; then
    echo "Error: No VMs specified and no running VMs found"
    exit 1
fi

echo "=== Batch Execute: $COMMAND ==="
echo "Target VMs: $TARGET_VMS"
echo

for VM in $TARGET_VMS; do
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "VM: $VM"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    if orb list | grep "^$VM " | grep -q " running "; then
        orb run "$VM" "$COMMAND" 2>&1 || echo "⚠️  Command failed on $VM"
    else
        echo "⚠️  VM $VM is not running, skipping..."
    fi

    echo
done

echo "✅ Batch execution complete!"
