#!/bin/bash
# Connect to a container's tmux session - tmux handles window resizing automatically
# Usage: container-view-attach-loop.sh <container> <session-name> [fallback]

CONTAINER="${1:-hvac-dev}"
SESSION="${2:-univers-mobile-view}"
FALLBACK="${3:-univers-manager}"

# Detect container system
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$SCRIPT_DIR/lib/container-helper.sh"

case "$(detect_container_system)" in
    lxd)
        lxc exec "$CONTAINER" -- su ubuntu -c "unset TMUX && exec tmux attach -t '$SESSION' 2>/dev/null || exec tmux attach -t '$FALLBACK' 2>/dev/null || exec bash"
        ;;
    orbstack)
        orb run --machine "$CONTAINER" bash -c "unset TMUX && exec tmux attach -t '$SESSION' 2>/dev/null || exec tmux attach -t '$FALLBACK' 2>/dev/null || exec bash"
        ;;
esac
