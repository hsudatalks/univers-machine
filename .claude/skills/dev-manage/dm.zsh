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

# dev-manager command (dm = dev-manage)
# New command structure: dm <command> [session-name]
# If session-name is omitted, defaults to "ark-dev"

dm() {
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
Usage: dm <command> [session-name]

Commands:
  start [session]    Start a dev session (default: ark-dev)
  stop [session]     Stop a dev session
  status [session]   Show session status
  attach [session]   Attach to a session
  restart [session]  Restart a session
  list               List all available sessions
  --help             Show this help

Examples:
  dm start           # Start default session (ark-dev)
  dm start infra-dev # Start infra-dev session
  dm attach          # Attach to default session
  dm status ark-dev  # Check ark-dev status
  dm list            # List all sessions

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

    # Validate command
    case "$command" in
        start|stop|status|attach|restart)
            # Valid command, execute it
            "$DEV_MANAGE_SCRIPT" "$session_name" "$command"
            ;;
        *)
            echo "❌ Unknown command: '$command'"
            echo ""
            echo "Available commands:"
            echo "  start, stop, status, attach, restart, list, --help"
            echo ""
            echo "Usage: dm <command> [session-name]"
            return 1
            ;;
    esac
}

# Optional: Add completion for dm
_dm() {
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
    compdef _dm dm
else
    # Fallback: define completion function without compdef
    # This allows the function to be available but without tab completion
    :
fi
