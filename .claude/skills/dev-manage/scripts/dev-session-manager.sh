#!/bin/bash
# Dev Session Manager - 4-Layer Architecture
# 层级: Dev层 → Machine层 → Container层 → Session层
# tmux: -L ark → -L machine → -L container → sessions
# Usage: ./dev-session-manager.sh <ark-dev> <start|stop|status|attach>

set -e

# Get the script's actual directory (works even when symlinked)
if [[ -L "${BASH_SOURCE[0]}" ]]; then
    # If script is symlinked, resolve the real path
    SCRIPT_DIR="$(cd "$(dirname "$(readlink "${BASH_SOURCE[0]}")")" && pwd)"
else
    # If not symlinked, use normal directory
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fi

# Calculate REPO_ROOT from script location
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
source "$REPO_ROOT/.claude/skills/machine-manage/lib/container-helper.sh"

SESSION_NAME="${1:-ark-dev}"
COMMAND="${2:-status}"
# Tmux server name: remove -dev suffix from session name
# e.g., ark-dev -> ark, infra-dev -> infra
TMUX_SERVER="${DEV_TMUX_SERVER:-${SESSION_NAME%-dev}}"

# Try config/servers.yaml first, fallback to configs/servers.yaml
if [[ -f "$REPO_ROOT/config/servers.yaml" ]]; then
    CONFIG_FILE="$REPO_ROOT/config/servers.yaml"
elif [[ -f "$REPO_ROOT/configs/servers.yaml" ]]; then
    CONFIG_FILE="$REPO_ROOT/configs/servers.yaml"
else
    print_error "Config file not found at $REPO_ROOT/config/servers.yaml or $REPO_ROOT/configs/servers.yaml"
    exit 1
fi

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

tmux() { command tmux -L "$TMUX_SERVER" "$@"; }
print_info() { echo -e "${BLUE}ℹ️  $1${NC}"; }
print_success() { echo -e "${GREEN}✅ $1${NC}"; }
print_warning() { echo -e "${YELLOW}⚠️  $1${NC}"; }
print_error() { echo -e "${RED}❌ $1${NC}"; }
print_header() { echo -e "${CYAN}\n📦 $1${NC}\n"; }

check_yq() {
    if ! command -v yq &> /dev/null; then
        print_error "yq command not found"
        echo "Install with: brew install yq (macOS)"
        exit 1
    fi
}

session_exists() { tmux has-session -t "$1" 2>/dev/null; }

start_session() {
    print_header "Starting Dev Session: $SESSION_NAME"

    if session_exists "$SESSION_NAME"; then
        print_warning "Session '$SESSION_NAME' already exists"
        echo "Use: mm dev $SESSION_NAME attach"
        return 0
    fi

    # Load config
    local ssh_options="$(yq eval ".sessions.$SESSION_NAME.ssh_options" "$CONFIG_FILE" 2>/dev/null | tr -d '"')"
    local disconnect_msg="$(yq eval ".sessions.$SESSION_NAME.disconnect_message" "$CONFIG_FILE" 2>/dev/null | tr -d '"')"
    local local_enabled="$(yq eval ".sessions.$SESSION_NAME.local_window.enabled" "$CONFIG_FILE" 2>/dev/null)"

    print_info "Config: SSH options=$ssh_options"

    # Get server keys
    local server_keys="$(yq eval ".sessions.$SESSION_NAME.servers | keys | .[]" "$CONFIG_FILE" 2>/dev/null)"

    # Create session with first server window
    local first_key="$(echo "$server_keys" | head -1)"
    if [ -n "$first_key" ]; then
        local host="$(yq eval ".sessions.$SESSION_NAME.servers.$first_key.host" "$CONFIG_FILE" 2>/dev/null | tr -d '"')"

        print_info "Creating window '$first_key' (SSH to $host)..."

        # Dev层: 创建会话和第一个窗口（直接启动SSH连接）
        tmux new-session -d -s "$SESSION_NAME" -n "$first_key" \
            "while true; do ssh $ssh_options $host -t 'bash -c \"tmux -L machine attach -t machine-mobile-view 2>/dev/null || echo Machine layer not found; exec bash\"' 2>&1; echo \"$disconnect_msg\"; read; done"
    fi

    # Configure dev session
    print_info "Configuring dev session..."
    tmux set-option -t "$SESSION_NAME" -g aggressive-resize on
    tmux set-option -t "$SESSION_NAME" status-style bg=colour234,fg=colour33
    tmux set-option -t "$SESSION_NAME" status-right '#[fg=colour39]%H:%M %m/%d'
    tmux set-option -t "$SESSION_NAME" window-status-format '#[fg=colour61] #W '
    tmux set-option -t "$SESSION_NAME" window-status-current-format '#[fg=colour39,bold][#W]#[fg=colour33] ✓'

    # Add remaining server windows
    local remaining_keys="$(echo "$server_keys" | tail -n +2)"
    while IFS= read -r key; do
        if [ -n "$key" ]; then
            local host="$(yq eval ".sessions.$SESSION_NAME.servers.$key.host" "$CONFIG_FILE" 2>/dev/null | tr -d '"')"

            print_info "Creating window '$key' (SSH to $host)..."

            tmux new-window -t "$SESSION_NAME" -n "$key" "while true; do ssh $ssh_options $host -t 'tmux -L machine attach -t machine-mobile-view 2>/dev/null || echo \"No machine-mobile-view found\"; bash' 2>&1; echo \"$disconnect_msg\"; read; done"
        fi
    done <<< "$remaining_keys"

    # Add local window (connects to local machine-mobile-view)
    if [ "$local_enabled" = "true" ]; then
        local local_name="$(yq eval ".sessions.$SESSION_NAME.local_window.display_name" "$CONFIG_FILE" 2>/dev/null | tr -d '"')"
        local local_path="$(yq eval ".sessions.$SESSION_NAME.local_window.tmux_path" "$CONFIG_FILE" 2>/dev/null | tr -d '"')"

        print_info "Creating local window '$local_name'..."

        tmux new-window -t "$SESSION_NAME" -n "$local_name" "unset TMUX && $local_path -L machine attach -t machine-mobile-view 2>/dev/null || echo \"No local machine-mobile-view found\""
    fi

    # Key bindings for window navigation
    tmux bind-key -T root C-n next-window
    tmux bind-key -T root C-p previous-window

    print_success "Session '$SESSION_NAME' created successfully!"
    echo
    echo "📋 Next steps:"
    echo "   1. Use: mm dev $SESSION_NAME attach"
    echo "   2. In tmux, switch windows with Ctrl+N / Ctrl+P"
    echo "   3. Each window connects to a server's machine-mobile-view"
    echo "   4. Use Ctrl+B D to detach from dev session"
}

stop_session() {
    print_header "Stopping Dev Session: $SESSION_NAME"

    if session_exists "$SESSION_NAME"; then
        tmux kill-session -t "$SESSION_NAME"
        print_success "Session '$SESSION_NAME' stopped"

        # Cleanup server if no sessions
        local session_count=$(tmux list-sessions 2>/dev/null | wc -l)
        if [ "$session_count" -eq 0 ]; then
            tmux kill-server 2>/dev/null || true
        fi
    else
        print_warning "Session is not running"
    fi
}

show_status() {
    print_header "Dev Session Status: $SESSION_NAME"

    if session_exists "$SESSION_NAME"; then
        print_success "Session is RUNNING"
        echo
        tmux list-windows -t "$SESSION_NAME" | sed 's/^/  /'
    else
        print_warning "Session is NOT RUNNING"
    fi
}

attach_session() {
    if session_exists "$SESSION_NAME"; then
        print_info "Attaching to $SESSION_NAME..."
        tmux attach-session -t "$SESSION_NAME"
    else
        print_error "Session not found. Use: mm dev $SESSION_NAME start"
        exit 1
    fi
}

restart_session() {
    stop_session
    sleep 1
    check_yq
    start_session
}

case "$COMMAND" in
    start)
        check_yq
        start_session
        ;;
    stop)
        stop_session
        ;;
    status)
        show_status
        ;;
    attach)
        attach_session
        ;;
    restart)
        restart_session
        ;;
    list)
        check_yq
        echo "Available sessions:"
        yq eval '.sessions | keys | .[]' "$CONFIG_FILE" 2>/dev/null | while read -r session; do
            # Get the first server's description as the session description
            desc=$(yq eval ".sessions.$session.servers[] | select(.description != null) | .description" "$CONFIG_FILE" 2>/dev/null | head -1 | tr -d '"')
            echo "  - $session: $desc"
        done || {
            # Fallback if yq fails
            echo "  - ark-dev: Multi-server development environment"
            echo "  - infra-dev: Infrastructure development"
            echo "  - domain-dev: Domain services development"
        }
        ;;
    -h|--help|help)
        echo "Dev Session Manager - 4-Layer Architecture"
        echo "Layers: Dev → Machine → Container → Session"
        echo
        echo "Usage: $0 <session-name> <command>"
        echo
        echo "Commands:"
        echo "  start    - Create and start session"
        echo "  stop     - Stop session"
        echo "  status   - Show session status"
        echo "  attach   - Attach to session"
        echo "  restart  - Restart session"
        echo "  list     - List available sessions"
        echo "  --help   - Show this help message"
        echo
        echo "Examples:"
        echo "  $0 ark-dev start   # Start ark-dev session (tmux -L ark)"
        echo "  $0 ark-dev attach  # Attach to ark-dev session"
        echo "  $0 ark-dev status  # Check ark-dev status"
        echo "  $0 --help          # Show this help"
        echo
        echo "Available sessions (see configs/servers.yaml):"
        echo "  - ark-dev: Multi-server development environment"
        echo "  - infra-dev: Infrastructure development"
        echo "  - domain-dev: Domain services development"
        echo
        echo "Note: Each dev session uses its own tmux server instance,"
        echo "      allowing independent key bindings for each session."
        ;;
    *)
        print_error "Unknown command: '$COMMAND'"
        echo "Use: $0 --help for usage information"
        exit 1
        ;;
esac
