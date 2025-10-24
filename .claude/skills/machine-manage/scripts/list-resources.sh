#!/bin/bash
# List resource usage across all OrbStack VMs
# Usage: ./list-resources.sh

set -e

echo "=== OrbStack VMs Resource Usage ==="
echo

# Get list of VMs
orb list | while IFS= read -r line; do
    # Parse VM name (first column)
    VM_NAME=$(echo "$line" | awk '{print $1}')
    STATUS=$(echo "$line" | awk '{print $2}')

    if [ "$STATUS" = "running" ]; then
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        echo "VM: $VM_NAME"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

        # Get disk usage
        echo "📊 Disk Usage:"
        orb run "$VM_NAME" df -h / 2>/dev/null || echo "  (could not retrieve)"

        echo

        # Get memory info
        echo "💾 Memory:"
        orb run "$VM_NAME" free -h 2>/dev/null || echo "  (could not retrieve)"

        echo

        # Check for large directories
        echo "📁 Large directories (>100MB):"
        orb run "$VM_NAME" "du -h --max-depth=3 ~ 2>/dev/null | grep -E '^[0-9.]+G|^[0-9]{3,}M' | sort -hr | head -10" 2>/dev/null || echo "  (could not retrieve)"

        echo
    fi
done

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Summary:"
orb list
