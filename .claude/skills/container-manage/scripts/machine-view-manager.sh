#!/bin/bash
# Machine-level tmux view manager
# Aggregates VM tmux sessions into machine-level sessions
# Usage: ./machine-view-manager.sh <start|stop|status|attach>

set -e

COMMAND="${1:-status}"

# VM list for dev machines
DEV_VMS=("integration-dev" "web-dev" "control-dev" "validation-dev")

# Machine management directory
MACHINE_DIR="/Users/davidxu/repos/univers-machine"

# Get script directory and config path
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STYLE_CONFIG="$SCRIPT_DIR/../configs/machine-tmux-style.conf"

# Color codes
GREEN='\033[0;32m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

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

# Create or ensure univers-machine-manage session exists
ensure_machine_manage_session() {
    if session_exists "univers-machine-manage"; then
        print_info "univers-machine-manage 会话已存在"
        return
    fi

    print_header "创建 univers-machine-manage 会话"

    # Create the session in the machine directory
    tmux new-session -d -s univers-machine-manage -c "$MACHINE_DIR"

    print_success "univers-machine-manage 会话已创建"
}

# Create machine-desktop-view session
create_desktop_view() {
    print_header "创建 Machine Desktop View 会话"

    if session_exists "machine-desktop-view"; then
        print_warning "machine-desktop-view 会话已存在"
        return
    fi

    # Create the main session with first VM
    local first_vm="${DEV_VMS[0]}"
    tmux new-session -d -s machine-desktop-view -n "$first_vm"

    # Apply style configuration
    if [ -f "$STYLE_CONFIG" ]; then
        tmux source-file "$STYLE_CONFIG"
        print_info "已应用样式配置"
    else
        print_warning "样式配置文件未找到: $STYLE_CONFIG"
    fi

    tmux send-keys -t "machine-desktop-view:$first_vm" "orbctl run --machine $first_vm tmux attach -t univers-desktop-view" C-m

    # Add windows for other VMs
    for vm in "${DEV_VMS[@]:1}"; do
        tmux new-window -t machine-desktop-view -n "$vm"
        tmux send-keys -t "machine-desktop-view:$vm" "orbctl run --machine $vm tmux attach -t univers-desktop-view" C-m
    done

    # Add machine-manage window at the end
    tmux new-window -t machine-desktop-view -n "machine-manage"
    tmux send-keys -t "machine-desktop-view:machine-manage" "unset TMUX && tmux attach -t univers-machine-manage" C-m

    # Select first window
    tmux select-window -t "machine-desktop-view:0"

    print_success "Machine Desktop View 会话已创建 (包含 machine-manage 窗口)"
}

# Create machine-mobile-view session
create_mobile_view() {
    print_header "创建 Machine Mobile View 会话"

    if session_exists "machine-mobile-view"; then
        print_warning "machine-mobile-view 会话已存在"
        return
    fi

    # Create the main session with first VM
    local first_vm="${DEV_VMS[0]}"
    tmux new-session -d -s machine-mobile-view -n "$first_vm"

    # Apply style configuration
    if [ -f "$STYLE_CONFIG" ]; then
        tmux source-file "$STYLE_CONFIG"
        print_info "已应用样式配置"
    else
        print_warning "样式配置文件未找到: $STYLE_CONFIG"
    fi

    tmux send-keys -t "machine-mobile-view:$first_vm" "orbctl run --machine $first_vm tmux attach -t univers-mobile-view" C-m

    # Add windows for other VMs
    for vm in "${DEV_VMS[@]:1}"; do
        tmux new-window -t machine-mobile-view -n "$vm"
        tmux send-keys -t "machine-mobile-view:$vm" "orbctl run --machine $vm tmux attach -t univers-mobile-view" C-m
    done

    # Add machine-manage window at the end
    tmux new-window -t machine-mobile-view -n "machine-manage"
    tmux send-keys -t "machine-mobile-view:machine-manage" "unset TMUX && tmux attach -t univers-machine-manage" C-m

    # Select first window
    tmux select-window -t "machine-mobile-view:0"

    print_success "Machine Mobile View 会话已创建 (包含 machine-manage 窗口)"
}

# Start both sessions
start_sessions() {
    echo "=== 启动 Machine View Sessions ==="
    echo

    # Create or ensure machine-manage session exists
    ensure_machine_manage_session
    echo

    # Check if VMs are running
    print_info "检查虚拟机状态..."
    for vm in "${DEV_VMS[@]}"; do
        if ! orb list | grep "^$vm " | grep -q "running"; then
            print_warning "虚拟机 $vm 未运行，正在启动..."
            orbctl start "$vm"
        fi
    done

    echo
    create_desktop_view
    echo
    create_mobile_view

    echo
    print_success "所有会话已创建！"
    echo
    print_info "使用以下命令连接："
    echo "  tmux attach -t machine-desktop-view  # 桌面视图"
    echo "  tmux attach -t machine-mobile-view   # 移动视图"
    echo
    print_info "在 tmux 中切换窗口："
    echo "  Ctrl+B 然后按数字键 (0-4) 切换窗口"
    echo "    0: integration-dev"
    echo "    1: web-dev"
    echo "    2: control-dev"
    echo "    3: validation-dev"
    echo "    4: machine-manage (物理机管理)"
    echo "  Ctrl+B 然后按 w - 显示所有窗口列表"
    echo "  Ctrl+B 然后按 n - 下一个窗口"
    echo "  Ctrl+B 然后按 p - 上一个窗口"
    echo "  Ctrl+B 然后按 D - 退出 (detach)"
}

# Stop sessions
stop_sessions() {
    echo "=== 停止 Machine View Sessions ==="
    echo

    if session_exists "machine-desktop-view"; then
        tmux kill-session -t machine-desktop-view
        print_success "machine-desktop-view 已停止"
    else
        print_info "machine-desktop-view 未运行"
    fi

    if session_exists "machine-mobile-view"; then
        tmux kill-session -t machine-mobile-view
        print_success "machine-mobile-view 已停止"
    else
        print_info "machine-mobile-view 未运行"
    fi

    if session_exists "univers-machine-manage"; then
        tmux kill-session -t univers-machine-manage
        print_success "univers-machine-manage 已停止"
    else
        print_info "univers-machine-manage 未运行"
    fi
}

# Show status
show_status() {
    echo "=== Machine View Sessions 状态 ==="
    echo

    print_header "虚拟机状态:"
    orb list | grep -E "(integration-dev|web-dev|control-dev|validation-dev)" || echo "没有找到 dev 虚拟机"

    echo
    print_header "Machine View 会话状态:"

    if session_exists "machine-desktop-view"; then
        echo -e "${GREEN}● machine-desktop-view${NC} - 运行中"
        tmux list-windows -t machine-desktop-view | sed 's/^/  /'
    else
        echo -e "${YELLOW}○ machine-desktop-view${NC} - 未运行"
    fi

    echo

    if session_exists "machine-mobile-view"; then
        echo -e "${GREEN}● machine-mobile-view${NC} - 运行中"
        tmux list-windows -t machine-mobile-view | sed 's/^/  /'
    else
        echo -e "${YELLOW}○ machine-mobile-view${NC} - 未运行"
    fi

    echo

    if session_exists "univers-machine-manage"; then
        echo -e "${GREEN}● univers-machine-manage${NC} - 运行中"
        echo "  工作目录: $MACHINE_DIR"
    else
        echo -e "${YELLOW}○ univers-machine-manage${NC} - 未运行"
    fi

    echo
    print_info "所有 tmux 会话："
    tmux list-sessions 2>/dev/null | grep -E "machine-|univers-" || echo "  没有相关会话"
}

# Attach to a session
attach_session() {
    local view_type="${1:-desktop}"

    if [ "$view_type" = "desktop" ]; then
        if session_exists "machine-desktop-view"; then
            tmux attach -t machine-desktop-view
        else
            print_warning "machine-desktop-view 未运行，请先执行: $0 start"
            exit 1
        fi
    elif [ "$view_type" = "mobile" ]; then
        if session_exists "machine-mobile-view"; then
            tmux attach -t machine-mobile-view
        else
            print_warning "machine-mobile-view 未运行，请先执行: $0 start"
            exit 1
        fi
    else
        echo "Usage: $0 attach <desktop|mobile>"
        exit 1
    fi
}

# Main command handling
case "$COMMAND" in
    start)
        start_sessions
        ;;
    stop)
        stop_sessions
        ;;
    status)
        show_status
        ;;
    attach)
        attach_session "$2"
        ;;
    restart)
        stop_sessions
        echo
        sleep 2
        start_sessions
        ;;
    *)
        echo "Usage: $0 <start|stop|status|attach|restart> [desktop|mobile]"
        echo
        echo "Commands:"
        echo "  start         - 创建并启动 machine view 会话"
        echo "  stop          - 停止所有 machine view 会话"
        echo "  status        - 显示会话状态"
        echo "  attach <type> - 连接到指定会话 (desktop|mobile)"
        echo "  restart       - 重启所有会话"
        echo
        echo "Examples:"
        echo "  $0 start"
        echo "  $0 attach desktop"
        echo "  $0 status"
        exit 1
        ;;
esac
