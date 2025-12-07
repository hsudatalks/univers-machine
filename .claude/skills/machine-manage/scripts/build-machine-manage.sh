#!/bin/bash
# Machine Manage Session Builder
# 自动创建 univers-machine-manage 会话并内嵌各VM的容器管理会话

set -e

# Source the helper library
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$SCRIPT_DIR/lib/container-helper.sh"

# Get running VMs
load_vm_list() {
    local vms=()
    local filtered_vms=()
    local container_system="$(detect_container_system)"

    # Get running containers from actual system
    case "$container_system" in
        lxd)
            vms=($(lxc list --format=json 2>/dev/null | jq -r '.[] | select(.status == "Running") | .name' 2>/dev/null | sort))
            ;;
        orbstack)
            vms=($(orb list 2>/dev/null | grep " running " | awk '{print $1}' | sort))
            ;;
    esac

    # Filter: only include *-dev VMs, exclude ubuntu
    local excluded_vms=("ubuntu")
    for vm in "${vms[@]}"; do
        local is_excluded=0
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
    
    echo "${filtered_vms[@]}"
}

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

# Build machine manage session
build_machine_manage_session() {
    local session_name="univers-machine-manage"
    local vms=($(load_vm_list))
    
    if [ ${#vms[@]} -eq 0 ]; then
        echo "❌ Error: no running dev VMs found"
        exit 1
    fi

    echo "=== 构建 Machine Manage Session ==="
    echo "发现VM: ${vms[@]}"
    echo

    # Kill existing session if it exists
    if session_exists "$session_name"; then
        print_warning "发现现有 $session_name 会话，正在删除..."
        tmux kill-session -t "$session_name"
    fi

    print_header "创建 $session_name 会话"

    # Create main session
    local machine_dir="$(cd "$SCRIPT_DIR/../.." && pwd)"
    tmux new-session -d -s "$session_name" -c "$machine_dir" -n "opencode"

    # Enable aggressive-resize for different window sizes
    tmux set-option -t "$session_name" -g aggressive-resize on

    # Add windows for each VM in order: dev first, then ops
    local dev_vms=()
    local ops_vms=()
    
    for vm in "${vms[@]}"; do
        if [[ "$vm" == *-dev ]]; then
            dev_vms+=("$vm")
        elif [[ "$vm" == *-ops ]]; then
            ops_vms+=("$vm")
        fi
    done
    
    # Sort VMs within each group
    IFS=$'\n' dev_vms=($(sort <<<"${dev_vms[*]}"))
    IFS=$'\n' ops_vms=($(sort <<<"${ops_vms[*]}"))
    unset IFS
    
    local all_vms=("${dev_vms[@]}" "${ops_vms[@]}")
    
    # Create windows for each VM
    for vm in "${all_vms[@]}"; do
        print_info "创建窗口: $vm"
        tmux new-window -t "$session_name" -n "$vm"
        
        # SSH into VM and check/create univers-manage session, then attach
        tmux send-keys -t "$session_name:$vm" "ssh $vm@orb" Enter
        sleep 1
        tmux send-keys -t "$session_name:$vm" "if ! tmux has-session -t univers-manage 2>/dev/null; then echo 'Creating univers-manage session...'; cm tmux start 2>/dev/null || echo 'cm command not found, creating manually...'; tmux new-session -d -s univers-manage; fi" Enter
        sleep 2
        tmux send-keys -t "$session_name:$vm" "tmux attach -t univers-manage" Enter
        sleep 0.5
    done

    # Select first window
    tmux select-window -t "$session_name:0"

    print_success "Machine Manage 会话构建完成！"
    echo
    print_info "窗口列表："
    local i=0
    echo "  $i: opencode"
    i=$((i + 1))
    for vm in "${all_vms[@]}"; do
        echo "  $i: $vm (内嵌 univers-manage)"
        i=$((i + 1))
    done
    echo
    print_info "使用以下命令连接："
    echo "  tmux attach -t $session_name"
    echo
    print_info "在 tmux 中切换窗口："
    echo "  Ctrl+B 然后按数字键 (0-$((i - 1))) 切换窗口"
    echo "  Ctrl+B 然后按 w - 显示所有窗口列表"
}

# Refresh machine manage session without closing
refresh_machine_manage_session() {
    local session_name="univers-machine-manage"
    local vms=($(load_vm_list))
    
    if [ ${#vms[@]} -eq 0 ]; then
        echo "❌ Error: no running dev VMs found"
        exit 1
    fi

    if ! session_exists "$session_name"; then
        print_warning "$session_name 会话不存在，正在创建..."
        build_machine_manage_session
        return
    fi

    echo "=== 刷新 Machine Manage Session ==="
    echo "当前VM: ${vms[@]}"
    echo

    # Sort VMs: dev first, then ops
    local dev_vms=()
    local ops_vms=()
    
    for vm in "${vms[@]}"; do
        if [[ "$vm" == *-dev ]]; then
            dev_vms+=("$vm")
        elif [[ "$vm" == *-ops ]]; then
            ops_vms+=("$vm")
        fi
    done
    
    IFS=$'\n' dev_vms=($(sort <<<"${dev_vms[*]}"))
    IFS=$'\n' ops_vms=($(sort <<<"${ops_vms[*]}"))
    unset IFS
    
    local all_vms=("${dev_vms[@]}" "${ops_vms[@]}")

    # Get current windows
    local current_windows=$(tmux list-windows -t "$session_name" -F "#{window_name}")
    
    # Remove windows for VMs that are no longer running (except opencode)
    for window_name in $current_windows; do
        if [[ "$window_name" != "opencode" ]]; then
            if ! printf '%s\n' "${all_vms[@]}" | grep -q "^$window_name$"; then
                print_info "移除窗口: $window_name"
                local window_index=$(tmux list-windows -t "$session_name" -F "#{window_index}:#{window_name}" | grep ":$window_name$" | cut -d: -f1)
                tmux kill-window -t "$session_name:$window_index"
            fi
        fi
    done

    # Add missing VM windows
    current_windows=$(tmux list-windows -t "$session_name" -F "#{window_name}")
    for vm in "${all_vms[@]}"; do
        if ! echo "$current_windows" | grep -q "^$vm$"; then
            print_info "添加窗口: $vm"
            # Find position after opencode (window 0)
            tmux new-window -t "$session_name:1" -n "$vm"
            
            # SSH into VM and check/create univers-manage session, then attach
            tmux send-keys -t "$session_name:$vm" "ssh $vm@orb" Enter
            sleep 1
            tmux send-keys -t "$session_name:$vm" "if ! tmux has-session -t univers-manage 2>/dev/null; then echo 'Creating univers-manage session...'; cm tmux start 2>/dev/null || echo 'cm command not found, creating manually...'; tmux new-session -d -s univers-manage; fi" Enter
            sleep 2
            tmux send-keys -t "$session_name:$vm" "tmux attach -t univers-manage" Enter
            sleep 0.5
        fi
    done

    print_success "Machine Manage 会话已刷新！"
    echo
    print_info "当前窗口列表："
    tmux list-windows -t "$session_name" -F "#{window_index}: #{window_name}"
}

# Main command handling
case "${1:-build}" in
    build)
        build_machine_manage_session
        ;;
    rebuild)
        build_machine_manage_session
        ;;
    refresh)
        refresh_machine_manage_session
        ;;
    -h|--help|help)
        echo "Machine Manage Session Builder"
        echo
        echo "Usage: $0 [build|rebuild|refresh]"
        echo
        echo "Commands:"
        echo "  build    - 构建 machine manage 会话（默认）"
        echo "  rebuild  - 重新构建会话（删除现有会话）"
        echo "  refresh  - 刷新会话（不关闭现有窗口）"
        echo "  help     - 显示此帮助信息"
        echo
        echo "功能："
        echo "  - 自动检测运行中的 dev VMs"
        echo "  - 创建 univers-machine-manage 会话"
        echo "  - 为每个 VM 创建窗口并内嵌 univers-manage 会话"
        echo "  - 自动检测并创建容器内的 univers-manage 会话"
        echo "  - 按 dev VMs 优先，ops VMs 在后的顺序排列"
        echo "  - refresh 支持动态添加/删除 VM 窗口"
        ;;
    *)
        echo "❌ Error: unknown command '$1'"
        echo "Use '$0 --help' for help"
        exit 1
        ;;
esac