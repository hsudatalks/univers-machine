#!/bin/zsh
# Dev Manage Skill - Zsh Configuration
# 开发环境管理 Skill - Zsh 配置
#
# 使用方法：
# 1. 将此文件添加到 ~/.zshrc：
#    source /Users/davidxu/repos/univers-machine/.claude/skills/dev-manage/dm.zsh
# 2. 重新加载配置：
#    source ~/.zshrc
# 3. 使用命令：
#    dm ark-dev start
#

# Get the script directory dynamically
DEV_MANAGE_SKILL_DIR="${${(%):-%x}:A:h}"
DEV_MANAGE_SCRIPT="$DEV_MANAGE_SKILL_DIR/scripts/dev-session-manager.sh"

# Primary function: dev-manager
# Usage: dev-manager <command> [session-name]
# If session-name is omitted, defaults to "ark-dev"

dev-manager() {
    if [[ ! -x "$DEV_MANAGE_SCRIPT" ]]; then
        echo "❌ Error: dev-session-manager.sh not found or not executable"
        echo "   Path: $DEV_MANAGE_SCRIPT"
        return 1
    fi

    # Handle no arguments
    if [[ $# -eq 0 ]]; then
        # Default to status of default session
        "$DEV_MANAGE_SCRIPT" "ark-dev" "status"
        return 0
    fi

    # Check for help flag anywhere in arguments
    for arg in "$@"; do
        if [[ "$arg" == "--help" || "$arg" == "-h" ]]; then
            cat <<'EOF'
Dev Manage - Development Session Manager
Usage: dev-manager <command> [session-name]
Alias: dm

Commands:
  start [session]     Start a dev session (default: ark-dev)
  stop [session]      Stop a dev session
  status [session]    Show session status
  attach [session]    Attach to a session
  restart [session]   Restart a session
  list                List all available sessions
  update [session]    Update remote server repositories
  mm-start [session] [view] Start machine view on servers (mobile|desktop)
  mm-manage [session] [action] Manage machine-manage service (start|stop|restart|status)
  cm-setup [session]  Setup container-manage on all servers
  --help              Show this help

Examples:
  dev-manager start           # Start default session (ark-dev)
  dev-manager start infra-dev # Start infra-dev session
  dev-manager attach          # Attach to default session
  dev-manager status ark-dev  # Check ark-dev status
  dev-manager list            # List all sessions
  dev-manager mm-start ark-dev mobile  # Start machine-mobile-view on all servers
  dev-manager mm-manage ark-dev status # Check machine-manage service status

Alias examples:
  dm start           # Same as dev-manager start
  dm attach          # Same as dev-manager attach

Available sessions:
  ark-dev   - Multi-server development environment
  infra-dev - Infrastructure development
  domain-dev - Domain services development

In a dm session:
  Ctrl+N    - Next window
  Ctrl+P    - Previous window
  Ctrl+B D  - Detach
EOF
            return 0
        fi
    done

    # Parse command
    local command="$1"
    shift

    # Handle list command (no session needed)
    if [[ "$command" == "list" ]]; then
        # Pass ark-dev as session name (required by script) but will be ignored for list
        "$DEV_MANAGE_SCRIPT" "ark-dev" "list"
        return 0
    fi

    # Get session name (default to ark-dev if not provided)
    local session_name="${1:-ark-dev}"

    # Handle remote commands that don't need tmux
    case "$command" in
        update|mm-stop|mm-restart|mm-status|cm-setup)
            "$DEV_MANAGE_SCRIPT" "$session_name" "$command"
            return $?
            ;;
        mm-manage)
            # Parse action for mm-manage
            if [[ -n "${2:-}" && ("$2" == "start" || "$2" == "stop" || "$2" == "restart" || "$2" == "status") ]]; then
                action="$2"
                "$DEV_MANAGE_SCRIPT" "$session_name" "mm-manage" "$action"
            else
                # Default to status if not specified
                "$DEV_MANAGE_SCRIPT" "$session_name" "mm-manage" "status"
            fi
            return $?
            ;;
        mm-start)
            # Parse view type for mm-start (mobile or desktop)
            if [[ -n "${2:-}" && ("$2" == "mobile" || "$2" == "desktop") ]]; then
                # Command is: dev-manager mm-start ark-dev mobile
                view_type="$2"
                "$DEV_MANAGE_SCRIPT" "$session_name" "mm-start" "" "$view_type"
            else
                # Default to mobile if not specified
                "$DEV_MANAGE_SCRIPT" "$session_name" "mm-start" "" "mobile"
            fi
            return $?
            ;;
    esac

    # Validate command for tmux operations
    case "$command" in
        start|stop|status|attach|restart)
            # Valid command, execute it
            "$DEV_MANAGE_SCRIPT" "$session_name" "$command"
            ;;
        *)
            echo "❌ Unknown command: '$command'"
            echo ""
            echo "Available commands:"
            echo "  start, stop, status, attach, restart, list"
            echo "  update, mm-start, mm-stop, mm-restart, mm-status"
            echo "  --help"
            echo ""
            echo "Usage: dev-manager <command> [session-name]"
            return 1
            ;;
    esac
}

# Alias: dm (dev-manage)
# Short alias for dev-manager
alias dm=dev-manager

# Optional: Add completion for dm/dev-manager
_dev_manager() {
    local -a sessions commands
    sessions=(
        'ark-dev:Ark开发环境'
        'infra-dev:基础设施开发'
    )
    commands=(
        'start:启动会话'
        'stop:停止会话'
        'status:查看状态'
        'attach:连接到会话'
        'restart:重启会话'
    )

    if [[ ${#words} -eq 2 ]]; then
        # First argument: session name
        _describe 'session' sessions
    elif [[ ${#words} -eq 3 ]]; then
        # Second argument: command
        _describe 'command' commands
    fi
}

# Safely add completion if compdef is available
if command -v compdef >/dev/null 2>&1; then
    compdef _dev_manager dm
    compdef _dev_manager dev-manager
else
    # Fallback: define completion function without compdef
    # This allows the function to be available but without tab completion
    :
fi
