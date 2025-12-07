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
# Excludes: ubuntu (not a dev machine, used for other purposes)
load_vm_list() {
    local vms=()
    local filtered_vms=()
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

    # Filter: only include *-dev VMs, exclude ubuntu and non-dev VMs
    local excluded_vms=("ubuntu")
    for vm in "${vms[@]}"; do
        local is_excluded=0
        # Check if in excluded list
        for excluded in "${excluded_vms[@]}"; do
            if [ "$vm" = "$excluded" ]; then
                is_excluded=1
                break
            fi
        done
        # Only include VMs ending with -dev
        if [ $is_excluded -eq 0 ] && [[ "$vm" == *-dev ]]; then
            filtered_vms+=("$vm")
        fi
    done
    vms=("${filtered_vms[@]}")

    # If no running containers found, return empty (caller will handle)
    if [ ${#vms[@]} -eq 0 ]; then
        # Try to get from config as fallback
        if [ -f "$VMS_CONFIG" ]; then
            local order_vms=$(grep -A 20 "^display-order:" "$VMS_CONFIG" 2>/dev/null | grep "^  - " | sed 's/^  - //' | grep -v "^#")
            if [ -n "$order_vms" ]; then
                while IFS= read -r vm; do
                    # Skip excluded containers and non-dev VMs
                    local is_excluded=0
                    for excluded in "${excluded_vms[@]}"; do
                        if [ "$vm" = "$excluded" ]; then
                            is_excluded=1
                            break
                        fi
                    done
                    # Only include VMs ending with -dev
                    if [ $is_excluded -eq 0 ] && [[ "$vm" == *-dev ]]; then
                        local enabled=$(grep -A 5 "^  $vm:" "$VMS_CONFIG" 2>/dev/null | grep "enabled:" | awk '{print $2}')
                        if [ "$enabled" = "true" ]; then
                            vms+=("$vm")
                        fi
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
    local session_name="${2:-container-desktop-view}"
    local fallback_session="${3:-container-manager}"

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
    # Use larger default size (179x50) to avoid constraining nested sessions
    local first_vm="${DEV_VMS[0]}"
    local window_name="${first_vm%-dev}"  # Remove -dev suffix for display
    tmux new-session -d -s machine-desktop-view -n "$window_name" -x 179 -y 50

    # Enable aggressive-resize so each window can resize independently for different clients
    tmux set-option -t machine-desktop-view -g aggressive-resize on

    # Apply desktop style configuration
    if [ -f "$DESKTOP_STYLE_CONFIG" ]; then
        tmux source-file -t machine-desktop-view "$DESKTOP_STYLE_CONFIG" 2>/dev/null || true
        print_info "已应用桌面视图样式配置"
    else
        print_warning "样式配置文件未找到: $DESKTOP_STYLE_CONFIG"
    fi

    # Attach to first VM's container-desktop-view
    # Use -d to detach other clients so window size follows this connection
    tmux send-keys -t "machine-desktop-view:$window_name" "mm shell $first_vm" C-m
    sleep 1
    tmux send-keys -t "machine-desktop-view:$window_name" "tmux attach -d -t container-desktop-view" C-m

    # Add windows for other VMs
    for vm in "${DEV_VMS[@]:1}"; do
        local window_name="${vm%-dev}"  # Remove -dev suffix for display
        tmux new-window -t machine-desktop-view -n "$window_name"
        tmux send-keys -t "machine-desktop-view:$window_name" "mm shell $vm" C-m
        sleep 1
        tmux send-keys -t "machine-desktop-view:$window_name" "tmux attach -d -t container-desktop-view" C-m
    done

    # Add machine-manage window at the end
    tmux new-window -t machine-desktop-view -n "machine"
    tmux send-keys -t "machine-desktop-view:machine" "unset TMUX && tmux attach -t univers-machine-manage" C-m

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
    # Use larger default size (179x50) to avoid constraining nested sessions
    local first_vm="${DEV_VMS[0]}"
    local window_name="${first_vm%-dev}"  # Remove -dev suffix for display
    tmux new-session -d -s machine-mobile-view -n "$window_name" -x 179 -y 50

    # Enable aggressive-resize so each window can resize independently for different clients
    tmux set-option -t machine-mobile-view -g aggressive-resize on

    # Apply mobile style configuration
    if [ -f "$MOBILE_STYLE_CONFIG" ]; then
        tmux source-file -t machine-mobile-view "$MOBILE_STYLE_CONFIG" 2>/dev/null || true
        print_info "已应用移动视图样式配置（简化版）"
    else
        print_warning "样式配置文件未找到: $MOBILE_STYLE_CONFIG"
    fi

    # Attach to first VM's container-mobile-view
    # Use -d to detach other clients so window size follows this connection
    tmux send-keys -t "machine-mobile-view:$window_name" "mm shell $first_vm" C-m
    sleep 1
    tmux send-keys -t "machine-mobile-view:$window_name" "tmux attach -d -t container-mobile-view" C-m

    # Add windows for other VMs
    for vm in "${DEV_VMS[@]:1}"; do
        local window_name="${vm%-dev}"  # Remove -dev suffix for display
        tmux new-window -t machine-mobile-view -n "$window_name"
        tmux send-keys -t "machine-mobile-view:$window_name" "mm shell $vm" C-m
        sleep 1
        tmux send-keys -t "machine-mobile-view:$window_name" "tmux attach -d -t container-mobile-view" C-m
    done

    # Add machine-manage window at the end
    tmux new-window -t machine-mobile-view -n "machine"
    tmux send-keys -t "machine-mobile-view:machine" "unset TMUX && tmux attach -t univers-machine-manage" C-m

    # Select first window
    tmux select-window -t "machine-mobile-view:0"

    print_success "Machine Mobile View 会话已创建 (包含 machine-manage 窗口)"
}

# Initialize container tmux sessions
initialize_container_tmux() {
    local vm="$1"

    print_info "初始化容器 $vm 的 tmux 会话..."

    # Step 1: Update univers-container repository
    print_info "  → 更新 univers-container 仓库..."
    if ! container_exec "$vm" "cd ~/repos/univers-container && git pull" 2>/dev/null; then
        print_warning "  → git pull 失败，可能仓库不存在或网络问题，继续执行..."
    fi

    # Step 2: Start container tmux sessions using cm
    print_info "  → 启动容器内的 tmux 会话 (cm tmux start)..."
    if ! container_exec "$vm" "cm tmux start" 2>/dev/null; then
        print_warning "  → cm tmux start 失败，可能 cm 命令不存在或会话已存在，继续执行..."
    fi

    print_success "容器 $vm 初始化完成"
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
    # Initialize container tmux sessions
    print_info "初始化容器内的 tmux 会话..."
    for vm in "${DEV_VMS[@]}"; do
        initialize_container_tmux "$vm"
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

        # Step 1: Remove old windows first (except machine)
        local window_list=$(tmux list-windows -t machine-desktop-view -F "#{window_name}:#{window_index}")
        while IFS=: read -r window_name window_index; do
            if [[ "$window_name" != "machine" && "$window_name" != machine-manage* ]]; then
                # Check if window name matches any VM (after removing -dev suffix)
                local found=false
                for vm in "${DEV_VMS[@]}"; do
                    if [[ "$window_name" == "${vm%-dev}" ]]; then
                        found=true
                        break
                    fi
                done
                if ! $found; then
                    print_info "  移除窗口: $window_name (窗口 $window_index)"
                    tmux kill-window -t "machine-desktop-view:$window_index"
                fi
            fi
        done <<< "$window_list"

        # Step 2: Add missing windows
        local current_windows=$(tmux list-windows -t machine-desktop-view -F "#{window_name}")
        for vm in "${DEV_VMS[@]}"; do
            local window_name="${vm%-dev}"  # Remove -dev suffix for display
            if ! echo "$current_windows" | grep -q "^$window_name$"; then
                print_info "  添加窗口: $window_name"
                # Find the position before machine window
                local machine_manage_index=$(tmux list-windows -t machine-desktop-view -F "#{window_index}:#{window_name}" | grep -E "^[0-9]+:(machine|machine-manage)$" | cut -d: -f1 | head -1)
                if [ -n "$machine_manage_index" ]; then
                    tmux new-window -t "machine-desktop-view:$machine_manage_index" -n "$window_name" -b
                else
                    tmux new-window -t machine-desktop-view -n "$window_name"
                fi
                tmux send-keys -t "machine-desktop-view:$window_name" "mm shell $vm" C-m
                sleep 1
                tmux send-keys -t "machine-desktop-view:$window_name" "tmux attach -d -t container-desktop-view" C-m
            fi
        done

        print_success "machine-desktop-view 已刷新"
    else
        print_warning "machine-desktop-view 未运行，跳过"
    fi

    echo

    # Refresh mobile view (same logic as desktop)
    if session_exists "machine-mobile-view"; then
        print_info "刷新 machine-mobile-view..."

        # Step 1: Remove old windows first (except machine)
        local window_list=$(tmux list-windows -t machine-mobile-view -F "#{window_name}:#{window_index}")
        while IFS=: read -r window_name window_index; do
            if [[ "$window_name" != "machine" && "$window_name" != machine-manage* ]]; then
                # Check if window name matches any VM (after removing -dev suffix)
                local found=false
                for vm in "${DEV_VMS[@]}"; do
                    if [[ "$window_name" == "${vm%-dev}" ]]; then
                        found=true
                        break
                    fi
                done
                if ! $found; then
                    print_info "  移除窗口: $window_name (窗口 $window_index)"
                    tmux kill-window -t "machine-mobile-view:$window_index"
                fi
            fi
        done <<< "$window_list"

        # Step 2: Add missing windows
        local current_windows=$(tmux list-windows -t machine-mobile-view -F "#{window_name}")
        for vm in "${DEV_VMS[@]}"; do
            local window_name="${vm%-dev}"  # Remove -dev suffix for display
            if ! echo "$current_windows" | grep -q "^$window_name$"; then
                print_info "  添加窗口: $window_name"
                # Find the position before machine window
                local machine_manage_index=$(tmux list-windows -t machine-mobile-view -F "#{window_index}:#{window_name}" | grep -E "^[0-9]+:(machine|machine-manage)$" | cut -d: -f1 | head -1)
                if [ -n "$machine_manage_index" ]; then
                    tmux new-window -t "machine-mobile-view:$machine_manage_index" -n "$window_name" -b
                else
                    tmux new-window -t machine-mobile-view -n "$window_name"
                fi
                tmux send-keys -t "machine-mobile-view:$window_name" "mm shell $vm" C-m
                sleep 1
                tmux send-keys -t "machine-mobile-view:$window_name" "tmux attach -d -t container-mobile-view" C-m
            fi
        done

        # Step 3: Ensure machine window exists
        if ! echo "$current_windows" | grep -qE "^(machine|machine-manage)$"; then
            print_info "  添加窗口: machine"
            tmux new-window -t machine-mobile-view -n "machine"
            tmux send-keys -t "machine-mobile-view:machine" "unset TMUX && tmux attach -t univers-machine-manage" C-m
        fi

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
    shell)
        # mm shell <container> [command...]
        # 使用方式：
        #   mm shell hvac-dev                  # 进入交互式 shell
        #   mm shell hvac-dev "tmux list-sessions"  # 执行命令
        #   mm shell hvac-dev ls -la /home/ubuntu   # 执行带参数的命令

        container="$2"
        if [ -z "$container" ]; then
            echo "❌ Error: container name required"
            echo "Usage: mm shell <container> [command...]"
            exit 1
        fi

        shift 2  # 移除 shell 和 container 参数

        if [ $# -gt 0 ]; then
            # 执行命令 - 将所有参数作为完整命令传递
            container_exec "$container" "$@"
        else
            # 进入交互式 shell
            container_shell "$container"
        fi
        ;;
    manage)
        # Check if this is build/rebuild/refresh or start/restart
        subcmd="$2"
        case "$subcmd" in
            build|rebuild|refresh)
                # mm manage [build|rebuild|refresh] - 构建/刷新 machine manage 会话
                "$SCRIPT_DIR/scripts/build-machine-manage.sh" "$subcmd"
                ;;
            start|restart)
                # mm manage <start|restart> [vm_name] - 管理容器内的 univers-manage 会话
                action="$subcmd"
                target_vm="$3"
                
                if [ -z "$action" ]; then
                    echo "❌ Error: action required"
                    echo "Usage: mm manage <start|restart> [vm_name]"
                    exit 1
                fi
                
                if [ "$action" != "start" ] && [ "$action" != "restart" ]; then
                    echo "❌ Error: invalid action '$action'"
                    echo "Supported actions: start, restart"
                    exit 1
                fi
                
                # 确定要操作的VM列表
                vms_to_manage=()
                if [ -n "$target_vm" ]; then
                    # 检查指定VM是否存在且运行
                    if ! container_is_running "$target_vm"; then
                        echo "❌ Error: VM '$target_vm' is not running"
                        exit 1
                    fi
                    vms_to_manage=("$target_vm")
                else
                    # 操作所有dev VMs
                    vms_to_manage=("${DEV_VMS[@]}")
                    if [ ${#vms_to_manage[@]} -eq 0 ]; then
                        echo "❌ Error: no running dev VMs found"
                        exit 1
                    fi
                fi
                
                echo "=== 管理 univers-manage 会话 ==="
                echo "操作: $action"
                echo "目标: ${vms_to_manage[@]}"
                echo
                
                for vm in "${vms_to_manage[@]}"; do
                    print_info "处理 VM: $vm"
                    
                    if [ "$action" = "restart" ]; then
                        # 先停止会话
                        print_info "  → 停止 univers-manage 会话..."
                        container_exec "$vm" "tmux has-session -t univers-manage 2>/dev/null && tmux kill-session -t univers-manage" 2>/dev/null || true
                    fi
                    
                    # 启动会话
                    print_info "  → 启动 univers-manage 会话..."
                    if container_exec "$vm" "cm tmux start" 2>/dev/null; then
                        print_success "  → $vm univers-manage 会话已启动"
                    else
                        print_warning "  → $vm univers-manage 启动失败，可能已存在或cm命令不可用"
                    fi
                done
                
                echo
                print_success "管理操作完成！"
                echo
                print_info "使用以下命令连接到管理会话："
                for vm in "${vms_to_manage[@]}"; do
                    echo "  mm shell $vm \"tmux attach -t univers-manage\""
                done
                ;;
            "")
                # Default to build if no subcommand provided
                "$SCRIPT_DIR/scripts/build-machine-manage.sh" build
                ;;
            *)
                echo "❌ Error: unknown manage subcommand '$subcmd'"
                echo "Supported subcommands: build, rebuild, refresh, start, restart"
                exit 1
                ;;
        esac
        ;;
    update)
        # mm update [container] - 更新容器内的代码库
        "$SCRIPT_DIR/scripts/update-container-repos.sh" "$2"
        ;;
    share)
        # mm share <container> <session> [options]
        # 通过 Web 浏览器分享 tmux 会话
        shift  # 移除 share 参数
        "$SCRIPT_DIR/scripts/tmux-web-share.sh" "$@"
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
        echo "  manage [cmd]  - 构建/刷新 machine manage 会话并内嵌容器管理会话"
        echo "                 - build: 构建会话（默认）"
        echo "                 - rebuild: 重新构建会话"
        echo "                 - refresh: 刷新会话"
        echo "                 - start|restart [vm]: 管理容器内univers-manage会话"
        echo "  shell <name> [cmd...]  - 进入容器或执行命令 (使用正确的用户账号)"
        echo "  -h, --help    - 显示此帮助信息"
        echo
        echo "Examples:"
        echo "  $0 start              # 启动所有会话"
        echo "  $0 attach desktop     # 连接桌面视图"
        echo "  $0 attach mobile      # 连接移动视图"
        echo "  $0 status             # 查看状态"
        echo "  $0 restart            # 重启所有会话"
        echo "  $0 refresh            # 刷新窗口（在 tmux 中运行）"
        echo "  $0 manage             # 构建 machine manage 会话（默认）"
        echo "  $0 manage rebuild     # 重新构建会话（删除现有）"
        echo "  $0 manage refresh     # 刷新会话（不关闭窗口）"
        echo "  $0 manage start       # 启动所有VM的univers-manage会话"
        echo "  $0 manage start web-dev  # 启动指定VM的univers-manage会话"
        echo "  $0 manage restart      # 重启所有VM的univers-manage会话"
        echo "  $0 update              # 更新所有VM的代码库"
        echo "  $0 update web-dev      # 更新指定VM的代码库"
        echo "  $0 shell hvac-dev     # 进入 hvac-dev 容器的交互式 shell"
        echo "  $0 shell hvac-dev tmux list-sessions  # 在容器中执行命令"
        echo "  $0 shell ubuntu 'ls -la ~'  # 在 ubuntu 容器执行命令"
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
