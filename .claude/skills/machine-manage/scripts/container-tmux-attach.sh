#!/bin/bash
# Helper script to connect to a tmux session in a container
# This script creates an interactive connection to a container with tmux access
# Usage: container-tmux-attach.sh <container> <session-name>

CONTAINER="${1:-hvac-dev}"
SESSION="${2:-univers-mobile-view}"

# Detect container system
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$SCRIPT_DIR/lib/container-helper.sh"

CONTAINER_SYSTEM="$(detect_container_system)"

# Create an interactive shell in the container with tmux available
# The user can then run 'tmux attach -t <session>' manually, or the script can auto-attach
case "$CONTAINER_SYSTEM" in
    lxd)
        # For LXD: Run interactive bash in the container as ubuntu user
        # This gives the user a shell where they can run tmux commands
        lxc exec "$CONTAINER" -- su ubuntu -c "
            # Try to attach to the requested session, or provide fallback shell
            if tmux has-session -t '$SESSION' 2>/dev/null; then
                # Auto-attach to the session
                unset TMUX
                exec tmux attach -t '$SESSION'
            else
                # Session doesn't exist, show available sessions and provide shell
                echo '📌 Requested session not found: $SESSION'
                echo ''
                echo '📋 Available tmux sessions:'
                tmux list-sessions 2>/dev/null || echo '  (no sessions found)'
                echo ''
                echo '💡 You can attach to a session by running: tmux attach -t <session-name>'
                echo ''
                exec bash -i
            fi
        "
        ;;
    orbstack)
        # For OrbStack: Similar approach using orb
        orb run --machine "$CONTAINER" bash -c "
            if tmux has-session -t '$SESSION' 2>/dev/null; then
                unset TMUX
                exec tmux attach -t '$SESSION'
            else
                echo '📌 Requested session not found: $SESSION'
                echo ''
                echo '📋 Available tmux sessions:'
                tmux list-sessions 2>/dev/null || echo '  (no sessions found)'
                echo ''
                echo '💡 You can attach to a session by running: tmux attach -t <session-name>'
                echo ''
                exec bash -i
            fi
        "
        ;;
esac
