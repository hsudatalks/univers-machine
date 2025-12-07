#!/bin/bash
#
# Tmux Agents Manager
# 管理 univers-ark-agents 服务的 tmux 会话
#

set -e

# ============================================
# 加载核心库
# ============================================

# 动态检测 univers-core 路径
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
UNIVERS_CORE="$PROJECT_ROOT/.claude/skills/univers-core/lib"

# 如果 univers-core 不在当前项目，尝试其他常见路径
if [ ! -d "$UNIVERS_CORE" ]; then
    # 尝试在 repos 目录中查找
    if [ -d "/home/ubuntu/repos/univers-container/.claude/skills/univers-core/lib" ]; then
        UNIVERS_CORE="/home/ubuntu/repos/univers-container/.claude/skills/univers-core/lib"
    elif [ -d "/home/davidxu/repos/univers-container/.claude/skills/univers-core/lib" ]; then
        UNIVERS_CORE="/home/davidxu/repos/univers-container/.claude/skills/univers-core/lib"
    else
        echo "Error: Cannot find univers-core library"
        exit 1
    fi
fi

source "$UNIVERS_CORE/common.sh"
source "$UNIVERS_CORE/tmux-utils.sh"

# 确保非 root 运行
ensure_non_root "$@"

# ============================================
# 配置
# ============================================
REPOS_ROOT="$(get_repos_root)"
SESSION_NAME="univers-agents"
WINDOW_NAME="agents"
AGENTS_DIR="$REPOS_ROOT/hvac-operation"

# ============================================
# 帮助信息
# ============================================
show_help() {
    cat << EOF
Univers Agents Tmux Manager

Usage: $0 [command] [options]

Commands:
    start       Start agents session
    stop        Stop agents session
    restart     Restart agents session
    status      Show session status
    attach      Attach to session
    logs        Show session logs
    help        Show this help

Examples:
    $0 start
    $0 attach
    $0 logs

EOF
}

# ============================================
# 核心功能函数
# ============================================
start_agents() {
    log_info "Starting univers-agents session..."

    # 检查会话是否已存在
    if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
        log_warning "Session $SESSION_NAME already exists"
        return 0
    fi

    # 检查 agents 目录是否存在
    if [ ! -d "$AGENTS_DIR" ]; then
        log_error "Agents directory not found: $AGENTS_DIR"
        log_info "Available directories in $REPOS_ROOT:"
        ls -la "$REPOS_ROOT/" | grep -E "^[d]"
        return 1
    fi

    # 创建会话
    tmux new-session -d -s "$SESSION_NAME" -c "$AGENTS_DIR"

    # 在会话中设置环境变量
    tmux send-keys -t "$SESSION_NAME:0" "export UNIVERS_CORE=$UNIVERS_CORE" Enter
    tmux send-keys -t "$SESSION_NAME:0" "export AGENTS_DIR=$AGENTS_DIR" Enter
    tmux send-keys -t "$SESSION_NAME:0" "clear" Enter

    log_success "Session $SESSION_NAME started successfully"
    log_info "Working directory: $AGENTS_DIR"
}

stop_agents() {
    log_info "Stopping univers-agents session..."

    if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
        tmux kill-session -t "$SESSION_NAME"
        log_success "Session $SESSION_NAME stopped"
    else
        log_warning "Session $SESSION_NAME not found"
    fi
}

restart_agents() {
    log_info "Restarting univers-agents session..."
    stop_agents
    sleep 1
    start_agents
}

show_status() {
    if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
        log_success "Session $SESSION_NAME is running"
        echo "Session details:"
        tmux list-sessions | grep "$SESSION_NAME"
        echo ""
        echo "Windows:"
        tmux list-windows -t "$SESSION_NAME"
    else
        log_warning "Session $SESSION_NAME is not running"
    fi
}

attach_session() {
    if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
        tmux attach-session -t "$SESSION_NAME"
    else
        log_error "Session $SESSION_NAME not found"
        log_info "Use '$0 start' to create the session"
        return 1
    fi
}

show_logs() {
    if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
        tmux capture-pane -t "$SESSION_NAME:0" -p | tail -50
    else
        log_error "Session $SESSION_NAME not found"
        return 1
    fi
}

# ============================================
# 主程序
# ============================================
main() {
    case "${1:-help}" in
        start)
            start_agents
            ;;
        stop)
            stop_agents
            ;;
        restart)
            restart_agents
            ;;
        status)
            show_status
            ;;
        attach)
            attach_session
            ;;
        logs)
            show_logs
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            log_error "Unknown command: $1"
            show_help
            exit 1
            ;;
    esac
}

# 执行主程序
main "$@"