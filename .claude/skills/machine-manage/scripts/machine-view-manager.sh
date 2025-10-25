#!/bin/bash
# Machine-level tmux view manager
# Aggregates container/VM tmux sessions into machine-level sessions
# Supports both LXD (Linux) and OrbStack (macOS)
# Usage: ./machine-view-manager.sh <start|stop|status|attach>

set -e

# Source the helper library
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$SCRIPT_DIR/lib/container-helper.sh"

COMMAND="${1:-status}"

# Determine machine directory based on OS
OS_TYPE="$(detect_os)"
case "$OS_TYPE" in
    linux)
        # On Linux, assume this repo is in home directory or user's repos
        MACHINE_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
        ;;
    macos)
        # On macOS, use the path from original script
        MACHINE_DIR="${HOME}/repos/univers-machine"
        ;;
    *)
        # Fallback: try to find it relative to script location
        MACHINE_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
        ;;
esac

# Get config paths
DESKTOP_STYLE_CONFIG="$SCRIPT_DIR/configs/machine-desktop-tmux-style.conf"
MOBILE_STYLE_CONFIG="$SCRIPT_DIR/configs/machine-mobile-tmux-style.conf"

# Config file path depends on container system
CONTAINER_SYSTEM="$(detect_container_system)"
case "$CONTAINER_SYSTEM" in
    lxd)
        # For LXD on Linux, check for containers.yaml first, then fall back to vms.yaml
        VMS_CONFIG="$MACHINE_DIR/config/containers.yaml"
        if [ ! -f "$VMS_CONFIG" ]; then
            VMS_CONFIG="$MACHINE_DIR/config/vms.yaml"
        fi
        ;;
    orbstack)
        # For OrbStack on macOS, use vms.yaml
        VMS_CONFIG="$MACHINE_DIR/config/vms.yaml"
        ;;
    *)
        VMS_CONFIG="$MACHINE_DIR/config/vms.yaml"
        ;;
esac

# Load VM list dynamically from actual running containers/VMs
# This detects real containers/VMs instead of relying on static config
load_vm_list() {
    local vms=()
    local container_system="$(detect_container_system)"

    # Get running containers from actual system (not config)
    case "$container_system" in
        lxd)
            # Get running LXD containers, sorted by name
            vms=($(lxc list --format=json 2>/dev/null | jq -r '.[] | select(.status == "Running") | .name' 2>/dev/null | sort))
            ;;
        orbstack)
            # Get running OrbStack VMs, sorted by name
            vms=($(orb list 2>/dev/null | grep " running " | awk '{print $1}' | sort))
            ;;
    esac

    # If no running containers found, return empty (caller will handle)
    if [ ${#vms[@]} -eq 0 ]; then
        # Try to get from config as fallback
        if [ -f "$VMS_CONFIG" ]; then
            local order_vms=$(grep -A 20 "^display-order:" "$VMS_CONFIG" 2>/dev/null | grep "^  - " | sed 's/^  - //' | grep -v "^#")
            if [ -n "$order_vms" ]; then
                while IFS= read -r vm; do
                    local enabled=$(grep -A 5 "^  $vm:" "$VMS_CONFIG" 2>/dev/null | grep "enabled:" | awk '{print $2}')
                    if [ "$enabled" = "true" ]; then
                        vms+=("$vm")
                    fi
                done <<< "$order_vms"
            fi
        fi
    fi

    echo "${vms[@]}"
}

# VM list for dev machines (loaded dynamically)
DEV_VMS=($(load_vm_list))

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

# Helper function to attach to container session with fallback
attach_to_container_session() {
    local container="$1"
    local session_name="${2:-univers-desktop-view}"
    local fallback_session="${3:-univers-manager}"

    local container_system="$(detect_container_system)"

    case "$container_system" in
        lxd)
            # Try main session first, then fallback
            lxc exec "$container" -- tmux attach -t "$session_name" 2>/dev/null || \
            lxc exec "$container" -- tmux attach -t "$fallback_session" 2>/dev/null || \
            lxc exec "$container" -- bash
            ;;
        orbstack)
            # Try main session first, then fallback
            orbctl run --machine "$container" tmux attach -t "$session_name" 2>/dev/null || \
            orbctl run --machine "$container" tmux attach -t "$fallback_session" 2>/dev/null || \
            orbctl shell "$container"
            ;;
    esac
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

    # Apply desktop style configuration
    if [ -f "$DESKTOP_STYLE_CONFIG" ]; then
        tmux source-file "$DESKTOP_STYLE_CONFIG"
        print_info "已应用桌面视图样式配置"
    else
        print_warning "样式配置文件未找到: $DESKTOP_STYLE_CONFIG"
    fi

    # Create command that properly handles cross-platform container access
    local container_system="$(detect_container_system)"
    case "$container_system" in
        lxd)
            # For LXD, need to unset TMUX for nested tmux sessions
            tmux send-keys -t "machine-desktop-view:$first_vm" "lxc exec $first_vm -- su ubuntu -c 'unset TMUX && tmux attach -t univers-desktop-view' 2>/dev/null || lxc exec $first_vm -- su ubuntu -c 'unset TMUX && tmux attach -t univers-mobile-view' 2>/dev/null || lxc exec $first_vm -- su ubuntu -c 'bash'" C-m
            ;;
        orbstack)
            tmux send-keys -t "machine-desktop-view:$first_vm" "orbctl run --machine $first_vm tmux attach -t univers-desktop-view 2>/dev/null || orbctl run --machine $first_vm tmux attach -t univers-manager 2>/dev/null || orbctl shell $first_vm" C-m
            ;;
    esac

    # Add windows for other VMs
    for vm in "${DEV_VMS[@]:1}"; do
        tmux new-window -t machine-desktop-view -n "$vm"
        case "$container_system" in
            lxd)
                # For LXD, need to unset TMUX for nested tmux sessions
                tmux send-keys -t "machine-desktop-view:$vm" "lxc exec $vm -- su ubuntu -c 'unset TMUX && tmux attach -t univers-desktop-view' 2>/dev/null || lxc exec $vm -- su ubuntu -c 'unset TMUX && tmux attach -t univers-mobile-view' 2>/dev/null || lxc exec $vm -- su ubuntu -c 'bash'" C-m
                ;;
            orbstack)
                tmux send-keys -t "machine-desktop-view:$vm" "orbctl run --machine $vm tmux attach -t univers-desktop-view 2>/dev/null || orbctl run --machine $vm tmux attach -t univers-manager 2>/dev/null || orbctl shell $vm" C-m
                ;;
        esac
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

    # Apply mobile style configuration (simplified)
    if [ -f "$MOBILE_STYLE_CONFIG" ]; then
        tmux source-file "$MOBILE_STYLE_CONFIG"
        print_info "已应用移动视图样式配置（简化版）"
    else
        print_warning "样式配置文件未找到: $MOBILE_STYLE_CONFIG"
    fi

    # Create command that properly handles cross-platform container access
    local container_system="$(detect_container_system)"
    case "$container_system" in
        lxd)
            # For LXD, need to unset TMUX for nested tmux sessions
            tmux send-keys -t "machine-mobile-view:$first_vm" "lxc exec $first_vm -- su ubuntu -c 'unset TMUX && tmux attach -t univers-mobile-view' 2>/dev/null || lxc exec $first_vm -- su ubuntu -c 'unset TMUX && tmux attach -t univers-desktop-view' 2>/dev/null || lxc exec $first_vm -- su ubuntu -c 'bash'" C-m
            ;;
        orbstack)
            tmux send-keys -t "machine-mobile-view:$first_vm" "orbctl run --machine $first_vm tmux attach -t univers-mobile-view 2>/dev/null || orbctl run --machine $first_vm tmux attach -t univers-manager 2>/dev/null || orbctl shell $first_vm" C-m
            ;;
    esac

    # Add windows for other VMs
    for vm in "${DEV_VMS[@]:1}"; do
        tmux new-window -t machine-mobile-view -n "$vm"
        case "$container_system" in
            lxd)
                # For LXD, need to unset TMUX for nested tmux sessions
                tmux send-keys -t "machine-mobile-view:$vm" "lxc exec $vm -- su ubuntu -c 'unset TMUX && tmux attach -t univers-mobile-view' 2>/dev/null || lxc exec $vm -- su ubuntu -c 'unset TMUX && tmux attach -t univers-desktop-view' 2>/dev/null || lxc exec $vm -- su ubuntu -c 'bash'" C-m
                ;;
            orbstack)
                tmux send-keys -t "machine-mobile-view:$vm" "orbctl run --machine $vm tmux attach -t univers-mobile-view 2>/dev/null || orbctl run --machine $vm tmux attach -t univers-manager 2>/dev/null || orbctl shell $vm" C-m
                ;;
        esac
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

    # Check if containers/VMs are running
    print_info "检查容器/虚拟机状态..."
    CONTAINER_SYSTEM="$(detect_container_system)"
    for vm in "${DEV_VMS[@]}"; do
        if ! container_is_running "$vm"; then
            print_warning "容器/虚拟机 $vm 未运行，正在启动..."
            container_start "$vm"
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
    local total_windows=$((${#DEV_VMS[@]} + 1))
    echo "  Ctrl+B 然后按数字键 (0-$((total_windows - 1))) 切换窗口"
    local i=0
    for vm in "${DEV_VMS[@]}"; do
        echo "    $i: $vm"
        i=$((i + 1))
    done
    echo "    $i: machine-manage (物理机管理)"
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
    echo "System: $(print_system_info)"
    echo

    print_header "容器/虚拟机状态:"

    # Show container/VM status
    CONTAINER_SYSTEM="$(detect_container_system)"
    case "$CONTAINER_SYSTEM" in
        lxd)
            lxc list --format=json | jq -r '.[] | "\(.name) \(.status)"' 2>/dev/null || echo "无法获取 LXD 容器列表"
            ;;
        orbstack)
            orb list | grep -E "(integration-dev|web-dev|control-dev|validation-dev)" || echo "没有找到 dev 虚拟机"
            ;;
        *)
            echo "Error: No container system detected"
            ;;
    esac

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

# Refresh windows in existing sessions to match current running containers
refresh_windows() {
    echo "=== 刷新 Machine View Sessions 窗口 ==="
    echo

    # Reload current VM list
    DEV_VMS=($(load_vm_list))

    if [ ${#DEV_VMS[@]} -eq 0 ]; then
        print_warning "没有找到运行中的容器/虚拟机"
        return 1
    fi

    echo "发现容器/虚拟机: ${DEV_VMS[@]}"
    echo

    # Refresh desktop view
    if session_exists "machine-desktop-view"; then
        print_info "刷新 machine-desktop-view..."

        # Get current window list
        local current_windows=$(tmux list-windows -t machine-desktop-view -F "#{window_name}" | head -n -1)

        # Add missing windows
        local container_system="$(detect_container_system)"
        local last_window_index=0
        for vm in "${DEV_VMS[@]}"; do
            if ! echo "$current_windows" | grep -q "^$vm$"; then
                print_info "  添加窗口: $vm"
                tmux new-window -t machine-desktop-view -n "$vm"
                case "$container_system" in
                    lxd)
                        # For LXD, need to unset TMUX for nested tmux sessions
                        tmux send-keys -t "machine-desktop-view:$vm" "lxc exec $vm -- su ubuntu -c 'unset TMUX && tmux attach -t univers-desktop-view' 2>/dev/null || lxc exec $vm -- su ubuntu -c 'unset TMUX && tmux attach -t univers-mobile-view' 2>/dev/null || lxc exec $vm -- su ubuntu -c 'bash'" C-m
                        ;;
                    orbstack)
                        tmux send-keys -t "machine-desktop-view:$vm" "orbctl run --machine $vm tmux attach -t univers-desktop-view 2>/dev/null || orbctl run --machine $vm tmux attach -t univers-manager 2>/dev/null || orbctl shell $vm" C-m
                        ;;
                esac
            fi
            last_window_index=$((last_window_index + 1))
        done

        # Remove old windows (except machine-manage)
        local window_list=$(tmux list-windows -t machine-desktop-view -F "#{window_name}:#{window_index}")
        while IFS=: read -r window_name window_index; do
            if [ "$window_name" != "machine-manage" ]; then
                if ! printf '%s\n' "${DEV_VMS[@]}" | grep -q "^$window_name$"; then
                    print_info "  移除窗口: $window_name"
                    tmux kill-window -t "machine-desktop-view:$window_index"
                fi
            fi
        done <<< "$window_list"

        print_success "machine-desktop-view 已刷新"
    else
        print_warning "machine-desktop-view 未运行，跳过"
    fi

    echo

    # Refresh mobile view (same logic)
    if session_exists "machine-mobile-view"; then
        print_info "刷新 machine-mobile-view..."

        # Similar logic for mobile view
        local current_windows=$(tmux list-windows -t machine-mobile-view -F "#{window_name}" | head -n -1)

        for vm in "${DEV_VMS[@]}"; do
            if ! echo "$current_windows" | grep -q "^$vm$"; then
                print_info "  添加窗口: $vm"
                tmux new-window -t machine-mobile-view -n "$vm"
                case "$container_system" in
                    lxd)
                        # For LXD, need to unset TMUX for nested tmux sessions
                        tmux send-keys -t "machine-mobile-view:$vm" "lxc exec $vm -- su ubuntu -c 'unset TMUX && tmux attach -t univers-mobile-view' 2>/dev/null || lxc exec $vm -- su ubuntu -c 'unset TMUX && tmux attach -t univers-desktop-view' 2>/dev/null || lxc exec $vm -- su ubuntu -c 'bash'" C-m
                        ;;
                    orbstack)
                        tmux send-keys -t "machine-mobile-view:$vm" "orbctl run --machine $vm tmux attach -t univers-mobile-view 2>/dev/null || orbctl run --machine $vm tmux attach -t univers-manager 2>/dev/null || orbctl shell $vm" C-m
                        ;;
                esac
            fi
        done

        local window_list=$(tmux list-windows -t machine-mobile-view -F "#{window_name}:#{window_index}")
        while IFS=: read -r window_name window_index; do
            if [ "$window_name" != "machine-manage" ]; then
                if ! printf '%s\n' "${DEV_VMS[@]}" | grep -q "^$window_name$"; then
                    print_info "  移除窗口: $window_name"
                    tmux kill-window -t "machine-mobile-view:$window_index"
                fi
            fi
        done <<< "$window_list"

        print_success "machine-mobile-view 已刷新"
    else
        print_warning "machine-mobile-view 未运行，跳过"
    fi

    echo
    print_success "所有会话已刷新！可在 tmux 中按 Ctrl+B w 查看新的窗口列表"
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
    refresh)
        refresh_windows
        ;;
    -h|--help|help)
        echo "Machine View Manager - 机器层面 tmux 会话管理"
        echo "支持 LXD (Linux) 和 OrbStack (macOS)"
        echo
        echo "System: $(print_system_info)"
        echo
        echo "Usage: $0 <command> [options]"
        echo
        echo "Commands:"
        echo "  start         - 创建并启动 machine view 会话"
        echo "  stop          - 停止所有 machine view 会话"
        echo "  status        - 显示会话状态"
        echo "  attach <type> - 连接到指定会话 (desktop|mobile)"
        echo "  restart       - 重启所有会话"
        echo "  refresh       - 刷新窗口以匹配当前运行的容器/VM（无需退出 tmux）"
        echo "  -h, --help    - 显示此帮助信息"
        echo
        echo "Examples:"
        echo "  $0 start              # 启动所有会话"
        echo "  $0 attach desktop     # 连接桌面视图"
        echo "  $0 attach mobile      # 连接移动视图"
        echo "  $0 status             # 查看状态"
        echo "  $0 restart            # 重启所有会话"
        echo "  $0 refresh            # 刷新窗口（在 tmux 中运行）"
        echo
        echo "Machine Views:"
        echo "  machine-desktop-view  - 聚合所有容器/VM 的桌面视图（完整信息）"
        echo "  machine-mobile-view   - 聚合所有容器/VM 的移动视图（简化显示）"
        echo "  univers-machine-manage - 物理机管理会话"
        echo
        echo "Supported Container Systems:"
        echo "  LXD (Linux)      - 使用 'lxc' 命令"
        echo "  OrbStack (macOS) - 使用 'orb' 命令"
        echo
        echo "Configuration:"
        echo "  Container list: $VMS_CONFIG"
        echo "  Machine dir: $MACHINE_DIR"
        exit 0
        ;;
    *)
        echo "错误: 未知命令 '$COMMAND'"
        echo
        echo "使用 '$0 --help' 查看帮助"
        exit 1
        ;;
esac
