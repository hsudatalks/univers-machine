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
elif [[ -f "$REPO_ROOT/config/servers.yaml.example" ]]; then
    CONFIG_FILE="$REPO_ROOT/config/servers.yaml.example"
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

load_tmux_config() {
    local session_name="$1"
    local config_file="$REPO_ROOT/.claude/skills/dev-manage/configs/dev-session-tmux.conf"

    if [[ ! -f "$config_file" ]]; then
        print_error "Tmux config file not found: $config_file"
        return 1
    fi

    print_info "Loading tmux configuration from $config_file..."

    # First, clear any conflicting global settings
    tmux set-option -t "$session_name" status-left ''
    tmux set-option -t "$session_name" status-right ''

    # Use a more robust approach: source the config file through tmux
    # The config file uses set -g which sets global options, we need session-specific
    while IFS= read -r line; do
        # Skip empty lines and comments
        [[ -z "$line" || "$line" =~ ^# ]] && continue

        # Remove inline comments (everything after # that's not in quotes)
        local clean_line=$(echo "$line" | sed 's/\s*#.*$//')
        [[ -z "$clean_line" ]] && continue

        # For set commands, replace -g (global) with -t (target session) if not already -t
        if [[ "$clean_line" =~ ^set-window-option ]]; then
            # Remove set-window-option -g or setw -g and replace with -t <session>
            if [[ "$clean_line" =~ -t\  ]]; then
                # Already has -t, don't modify
                eval "tmux $clean_line" 2>/dev/null || true
            else
                # Replace -g with -t <session>
                local cmd=$(echo "$clean_line" | sed "s/setw\? -g /set-window-option -t $session_name /")
                eval "tmux $cmd" 2>/dev/null || true
            fi
        elif [[ "$clean_line" =~ ^set\  ]]; then
            # Check if already has -t
            if [[ "$clean_line" =~ -t\  ]]; then
                # Already has -t, don't modify
                eval "tmux $clean_line" 2>/dev/null || true
            else
                # Replace set -g with set-option -t <session>
                local cmd=$(echo "$clean_line" | sed "s/set -g /set-option -t $session_name /")
                eval "tmux $cmd" 2>/dev/null || true
            fi
        fi
    done < "$config_file"

    print_success "Tmux configuration loaded"
}

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

        # Apply window format to first window immediately after creation
        tmux set-window-option -t "$SESSION_NAME:$first_key" window-status-format ' #W '
        tmux set-window-option -t "$SESSION_NAME:$first_key" window-status-current-format ' [#W] '
        tmux set-window-option -t "$SESSION_NAME:$first_key" automatic-rename off
        # Preserve color settings from config file
        tmux set-window-option -t "$SESSION_NAME:$first_key" window-status-style fg=colour244
        tmux set-window-option -t "$SESSION_NAME:$first_key" window-status-current-style fg=colour39,bold
    fi

    # Configure dev session from config file
    load_tmux_config "$SESSION_NAME"

    # Add remaining server windows
    local remaining_keys="$(echo "$server_keys" | tail -n +2)"
    while IFS= read -r key; do
        if [ -n "$key" ]; then
            local host="$(yq eval ".sessions.$SESSION_NAME.servers.$key.host" "$CONFIG_FILE" 2>/dev/null | tr -d '"')"

            print_info "Creating window '$key' (SSH to $host)..."

            tmux new-window -t "$SESSION_NAME" -n "$key" "while true; do ssh $ssh_options $host -t 'tmux -L machine attach -t machine-mobile-view 2>/dev/null || echo \"No machine-mobile-view found\"; bash' 2>&1; echo \"$disconnect_msg\"; read; done"

            # Apply window format immediately after creating window
            tmux set-window-option -t "$SESSION_NAME:$key" window-status-format ' #W '
            tmux set-window-option -t "$SESSION_NAME:$key" window-status-current-format ' [#W] '
            tmux set-window-option -t "$SESSION_NAME:$key" automatic-rename off
            # Preserve color settings from config file
            tmux set-window-option -t "$SESSION_NAME:$key" window-status-style fg=colour244
            tmux set-window-option -t "$SESSION_NAME:$key" window-status-current-style fg=colour39,bold
        fi
    done <<< "$remaining_keys"

    # Add local window (connects to local machine-mobile-view)
    if [ "$local_enabled" = "true" ]; then
        local local_name="$(yq eval ".sessions.$SESSION_NAME.local_window.display_name" "$CONFIG_FILE" 2>/dev/null | tr -d '"')"
        local local_path="$(yq eval ".sessions.$SESSION_NAME.local_window.tmux_path" "$CONFIG_FILE" 2>/dev/null | tr -d '"')"

        print_info "Creating local window '$local_name'..."

        # Keep window open even if machine-mobile-view doesn't exist
        tmux new-window -t "$SESSION_NAME" -n "$local_name" "
            while true; do
                if $local_path -L machine has-session -t machine-mobile-view 2>/dev/null; then
                    $local_path -L machine attach -t machine-mobile-view 2>/dev/null || echo \"Attach failed\";
                else
                    echo \"⚠️  Local machine-mobile-view not found\";
                    echo \"Run: mm machine start\" to create it;
                fi;
                echo \"Waiting... Press Ctrl+C to exit\";
                sleep 5;
            done
        "
    fi

    # Apply format to local window if it exists
    if [ "$local_enabled" = "true" ]; then
        print_info "Applying window status format to local window..."
        tmux set-window-option -t "$SESSION_NAME:$local_name" window-status-format ' #W '
        tmux set-window-option -t "$SESSION_NAME:$local_name" window-status-current-format ' [#W] '
        tmux set-window-option -t "$SESSION_NAME:$local_name" automatic-rename off
        # Preserve color settings from config file
        tmux set-window-option -t "$SESSION_NAME:$local_name" window-status-style fg=colour244
        tmux set-window-option -t "$SESSION_NAME:$local_name" window-status-current-style fg=colour39,bold
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

# Remote server management functions
update_repositories() {
    print_header "Updating repositories on all servers for $SESSION_NAME"

    # Get server keys
    local -a server_keys=()
    while IFS= read -r key; do
        server_keys+=("$key")
    done < <(yq eval ".sessions.$SESSION_NAME.servers | keys | .[]" "$CONFIG_FILE" 2>/dev/null)

    local ssh_options="$(yq eval ".sessions.$SESSION_NAME.ssh_options" "$CONFIG_FILE" 2>/dev/null | tr -d '"')"

    if [[ ${#server_keys[@]} -eq 0 ]]; then
        print_error "No servers found in configuration"
        exit 1
    fi

    # Statistics tracking - use simple arrays for bash 3.2 compatibility
    local total_servers=${#server_keys[@]}
    local successful_servers=0
    local -a failed_servers
    local -a failed_repos  # parallel array storing failed repos as "host:repo1 repo2"
    local -a all_repos_info  # parallel array storing all repos as "host:repo1 repo2"

    echo ""

    # Process each server
    for key in "${server_keys[@]}"; do
        if [[ -n "$key" ]]; then
            local host="$(yq eval ".sessions.$SESSION_NAME.servers.$key.host" "$CONFIG_FILE" 2>/dev/null | tr -d '"')"
            local username="$(yq eval ".sessions.$SESSION_NAME.servers.$key.username" "$CONFIG_FILE" 2>/dev/null | tr -d '"')"
            local repo_path="$(yq eval ".sessions.$SESSION_NAME.servers.$key.repo_path" "$CONFIG_FILE" 2>/dev/null | tr -d '"')"

            # Default username if not specified or null
            if [[ -z "$username" || "$username" == "null" ]]; then
                username="david"
            fi

            # Default repo path if not specified or null
            if [[ -z "$repo_path" || "$repo_path" == "null" ]]; then
                repo_path="/home/$username/repos"
            fi

            print_info "Pulling latest code on $host ($repo_path)..."

            # Execute SSH command and capture output
            local output
            local server_success=true
            local -a server_failed_repos
            local -a server_all_repos

            # Execute remote command
            output=$(ssh $ssh_options "$host" "/usr/bin/bash -lc 'cd $repo_path && for repo in univers-*/; do repo_name=\${repo%%/}; echo \"[\$repo_name]\"; cd $repo_path/\$repo_name && git pull --rebase 2>&1 || echo \"FAILED:\$repo_name\"; done'" 2>&1)

            # Display output
            echo "$output"

            # Parse output for failures
            while IFS= read -r line; do
                if [[ "$line" =~ ^FAILED:(.+)$ ]]; then
                    server_success=false
                    server_failed_repos+=("${BASH_REMATCH[1]}")
                elif [[ "$line" =~ ^\[([^\]]+)\] ]]; then
                    server_all_repos+=("${BASH_REMATCH[1]}")
                fi
            done <<< "$output"

            # Update statistics
            if [[ "$server_success" == "true" ]]; then
                ((successful_servers++))
            else
                failed_servers+=("$host")
                if [[ ${#server_failed_repos[@]} -gt 0 ]]; then
                    failed_repos+=("$host:${server_failed_repos[*]}")
                fi
            fi

            # Store all repos for this server
            if [[ ${#server_all_repos[@]} -gt 0 ]]; then
                all_repos_info+=("$host:${server_all_repos[*]}")
            fi
        fi
    done

    # Print summary
    echo ""
    print_header "Update Summary"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Total servers: $total_servers"
    echo "Successful:    $successful_servers ✓"
    echo "Failed:        $((total_servers - successful_servers)) ✗"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    if [[ ${#failed_servers[@]} -gt 0 ]]; then
        echo ""
        print_error "Failed repositories:"
        for info in "${failed_repos[@]}"; do
            local host="${info%%:*}"
            local repos="${info##*:}"
            echo "  ❌ $host"
            for repo in $repos; do
                echo "     • $repo"
            done
        done
        return 1
    else
        echo ""
        print_success "✅ All repositories updated successfully!"
        echo ""
        echo "Updated repositories:"
        for info in "${all_repos_info[@]}"; do
            local host="${info%%:*}"
            local repos="${info##*:}"
            echo "  ✓ $host: $repos"
        done
        return 0
    fi
}

mm_manage() {
    local action="$1"
    print_header "Managing machine-manage sessions ($action) on all servers for $SESSION_NAME"

    # Get server keys - use while loop without pipe to avoid subshell issues
    local ssh_options="$(yq eval ".sessions.$SESSION_NAME.ssh_options" "$CONFIG_FILE" 2>/dev/null | tr -d '"')"
    local -a server_keys=()
    while IFS= read -r key; do
        server_keys+=("$key")
    done < <(yq eval ".sessions.$SESSION_NAME.servers | keys | .[]" "$CONFIG_FILE" 2>/dev/null)

    if [[ ${#server_keys[@]} -eq 0 ]]; then
        print_error "No servers found in configuration"
        exit 1
    fi

    local all_success=true
    for key in "${server_keys[@]}"; do
        if [[ -n "$key" ]]; then
            local host="$(yq eval ".sessions.$SESSION_NAME.servers.$key.host" "$CONFIG_FILE" 2>/dev/null | tr -d '"')"

            print_info "Executing 'mm $action' on $host..."
            # Directly use the machine-view-manager.sh script instead of mm alias
            if ! ssh $ssh_options "$host" "/usr/bin/bash /home/david/repos/univers-machine/.claude/skills/machine-manage/scripts/machine-view-manager.sh $action" 2>&1; then
                print_warning "Command failed on $host, continuing..."
                all_success=false
            fi
        fi
    done

    if [[ "$all_success" == "true" ]]; then
        return 0
    else
        return 1
    fi
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
    update)
        check_yq
        update_repositories
        ;;
    mm-start)
        check_yq
        mm_manage "start"
        ;;
    mm-stop)
        check_yq
        mm_manage "stop"
        ;;
    mm-restart)
        check_yq
        mm_manage "restart"
        ;;
    mm-status)
        check_yq
        mm_manage "status"
        ;;
    -h|--help|help)
        echo "Dev Session Manager - 4-Layer Architecture"
        echo "Layers: Dev → Machine → Container → Session"
        echo
        echo "Usage: $0 <session-name> <command>"
        echo
        echo "Commands:"
        echo "  start       - Create and start session"
        echo "  stop        - Stop session"
        echo "  status      - Show session status"
        echo "  attach      - Attach to session"
        echo "  restart     - Restart session"
        echo "  list        - List available sessions"
        echo "  update      - Update repositories on all servers"
        echo "  mm-start    - Start machine-manage on all servers"
        echo "  mm-stop     - Stop machine-manage on all servers"
        echo "  mm-restart  - Restart machine-manage on all servers"
        echo "  mm-status   - Show machine-manage status on all servers"
        echo "  --help      - Show this help message"
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
