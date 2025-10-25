#!/bin/bash
# Initialize container-level tmux sessions for desktop and mobile views
# This script should be run INSIDE a container to set up aggregated tmux sessions
# These sessions group all service sessions together for easy multi-service monitoring
#
# Usage: ./container-session-init.sh [--cleanup]
#
# Note: This script can be used for any univers container:
#   - hvac-dev
#   - web-dev
#   - integration-dev
#   - control-dev
#   - validation-dev
#
# The script automatically creates:
#   - univers-desktop-view: aggregated desktop view with 3 windows
#   - univers-mobile-view: aggregated mobile view with 4 windows

set -e

# Constants
DESKTOP_VIEW="univers-desktop-view"
MOBILE_VIEW="univers-mobile-view"

# Desktop view windows: workbench, operation, manager
DESKTOP_WINDOWS=("workbench" "operation" "manager")

# Mobile view windows: dev, service, ops, manager
MOBILE_WINDOWS=("dev" "service" "ops" "manager")

# Color codes
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_header() {
    echo -e "${CYAN}📦 $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

# Check if a tmux session exists
session_exists() {
    tmux has-session -t "$1" 2>/dev/null
}

# Cleanup old sessions if requested
cleanup_sessions() {
    print_header "清理旧的 tmux 会话"

    if session_exists "$DESKTOP_VIEW"; then
        print_info "  删除 $DESKTOP_VIEW"
        tmux kill-session -t "$DESKTOP_VIEW"
    fi

    if session_exists "$MOBILE_VIEW"; then
        print_info "  删除 $MOBILE_VIEW"
        tmux kill-session -t "$MOBILE_VIEW"
    fi

    print_success "清理完成"
}

# Create desktop view session with all windows
create_desktop_view() {
    print_header "创建 $DESKTOP_VIEW 会话"

    if session_exists "$DESKTOP_VIEW"; then
        print_warning "$DESKTOP_VIEW 已存在，跳过初始化"
        return
    fi

    # Create main session with first window
    tmux new-session -d -s "$DESKTOP_VIEW" -c ~

    # Rename first window
    tmux rename-window -t "$DESKTOP_VIEW:0" "${DESKTOP_WINDOWS[0]}"
    tmux send-keys -t "$DESKTOP_VIEW:${DESKTOP_WINDOWS[0]}" "# Desktop View - $(date)" C-m

    # Add remaining windows
    for ((i=1; i<${#DESKTOP_WINDOWS[@]}; i++)); do
        local window="${DESKTOP_WINDOWS[$i]}"
        tmux new-window -t "$DESKTOP_VIEW" -n "$window"
        tmux send-keys -t "$DESKTOP_VIEW:$window" "# $window window" C-m
    done

    print_success "$DESKTOP_VIEW 会话已创建 (${#DESKTOP_WINDOWS[@]} 个窗口)"
}

# Create mobile view session with all windows
create_mobile_view() {
    print_header "创建 $MOBILE_VIEW 会话"

    if session_exists "$MOBILE_VIEW"; then
        print_warning "$MOBILE_VIEW 已存在，跳过初始化"
        return
    fi

    # Create main session with first window
    tmux new-session -d -s "$MOBILE_VIEW" -c ~

    # Rename first window
    tmux rename-window -t "$MOBILE_VIEW:0" "${MOBILE_WINDOWS[0]}"
    tmux send-keys -t "$MOBILE_VIEW:${MOBILE_WINDOWS[0]}" "# Mobile View - $(date)" C-m

    # Add remaining windows
    for ((i=1; i<${#MOBILE_WINDOWS[@]}; i++)); do
        local window="${MOBILE_WINDOWS[$i]}"
        tmux new-window -t "$MOBILE_VIEW" -n "$window"
        tmux send-keys -t "$MOBILE_VIEW:$window" "# $window window" C-m
    done

    print_success "$MOBILE_VIEW 会话已创建 (${#MOBILE_WINDOWS[@]} 个窗口)"
}

# Verify sessions are properly set up
verify_sessions() {
    print_header "验证会话设置"
    echo

    echo "Desktop View 窗口列表："
    if session_exists "$DESKTOP_VIEW"; then
        tmux list-windows -t "$DESKTOP_VIEW" | sed 's/^/  /'
    else
        print_warning "  $DESKTOP_VIEW 未创建"
    fi
    echo

    echo "Mobile View 窗口列表："
    if session_exists "$MOBILE_VIEW"; then
        tmux list-windows -t "$MOBILE_VIEW" | sed 's/^/  /'
    else
        print_warning "  $MOBILE_VIEW 未创建"
    fi
    echo

    print_success "验证完成"
}

# Main logic
main() {
    echo "=== 容器内 Tmux 会话初始化 ==="
    echo

    # Parse arguments
    CLEANUP=false
    if [ "$1" = "--cleanup" ]; then
        CLEANUP=true
    fi

    # Cleanup if requested
    if [ "$CLEANUP" = true ]; then
        cleanup_sessions
        echo
    fi

    # Create sessions
    create_desktop_view
    echo
    create_mobile_view
    echo

    # Verify
    verify_sessions
    echo

    print_success "容器初始化完成！"
    echo
    print_info "使用以下命令连接到聚合会话："
    echo "  tmux attach -t $DESKTOP_VIEW  # 桌面视图"
    echo "  tmux attach -t $MOBILE_VIEW   # 移动视图"
}

main "$@"
