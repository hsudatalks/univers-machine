#!/bin/bash
# Helper script to persistently display a container's tmux session in machine-view
# This script continuously attempts to attach to a container's session
# If the session doesn't exist, it waits and retries
# Usage: container-view-attach-loop.sh <container> <session-name> [fallback-session]

CONTAINER="${1:-hvac-dev}"
SESSION="${2:-univers-mobile-view}"
FALLBACK_SESSION="${3:-univers-manager}"
RETRY_INTERVAL=3

# Detect container system
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$SCRIPT_DIR/lib/container-helper.sh"

CONTAINER_SYSTEM="$(detect_container_system)"

# Function to attempt connection
attempt_connection() {
    case "$CONTAINER_SYSTEM" in
        lxd)
            # Try to attach to the requested session
            lxc exec "$CONTAINER" -- su ubuntu -c "
                if tmux has-session -t '$SESSION' 2>/dev/null; then
                    unset TMUX
                    exec tmux attach -t '$SESSION'
                elif tmux has-session -t '$FALLBACK_SESSION' 2>/dev/null; then
                    unset TMUX
                    exec tmux attach -t '$FALLBACK_SESSION'
                else
                    # Session doesn't exist yet
                    echo '⏳ Waiting for session: $SESSION'
                    exit 1
                fi
            " 2>/dev/null
            return $?
            ;;
        orbstack)
            orb run --machine "$CONTAINER" bash -c "
                if tmux has-session -t '$SESSION' 2>/dev/null; then
                    unset TMUX
                    exec tmux attach -t '$SESSION'
                elif tmux has-session -t '$FALLBACK_SESSION' 2>/dev/null; then
                    unset TMUX
                    exec tmux attach -t '$FALLBACK_SESSION'
                else
                    echo '⏳ Waiting for session: $SESSION'
                    exit 1
                fi
            " 2>/dev/null
            return $?
            ;;
    esac
}

# Main loop - continuously attempt to connect
while true; do
    attempt_connection
    EXIT_CODE=$?

    # If connection succeeded (exit code 0), we're done
    # If user detached (exit code != 0), we retry
    if [ $EXIT_CODE -eq 0 ]; then
        # User detached gracefully, try to reattach
        sleep 1
        continue
    else
        # Session doesn't exist or error occurred
        echo ""
        echo "⏳ Waiting for container session '${SESSION}' to be created..."
        echo "   Retrying in ${RETRY_INTERVAL} seconds..."
        echo ""
        sleep $RETRY_INTERVAL
    fi
done
