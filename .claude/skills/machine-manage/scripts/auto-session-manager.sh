#!/bin/bash
# Auto Session Manager - 自动为所有容器创建和管理会话
# 根据容器类型自动决定创建哪些会话，支持大规模容器管理

set -e

# Source helper library
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$SCRIPT_DIR/lib/container-helper.sh"

# Color codes
GREEN='\033[0;32m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
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

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Get running VMs dynamically
get_running_vms() {
    local container_system="$(detect_container_system)"
    local vms=()
    
    case "$container_system" in
        lxd)
            vms=($(lxc list --format=json 2>/dev/null | jq -r '.[] | select(.status == "Running") | .name' 2>/dev/null | sort))
            ;;
        orbstack)
            vms=($(orb list 2>/dev/null | grep " running " | awk '{print $1}' | sort))
            ;;
    esac
    
    # Filter to only *-dev VMs, exclude ubuntu
    local filtered_vms=()
    for vm in "${vms[@]}"; do
        if [[ "$vm" == *-dev ]] && [ "$vm" != "ubuntu" ]; then
            filtered_vms+=("$vm")
        fi
    done
    
    echo "${filtered_vms[@]}"
}

# Determine session types for a VM based on its name/pattern
# Currently all VMs get full sessions for maximum compatibility
get_session_types_for_vm() {
    local vm="$1"
    local session_types=("developer" "server" "ui" "web" "operator" "manager" "agents" "check" "e2e" "bench")
    
    echo "${session_types[@]}"
}

# Check if a session exists
session_exists() {
    local session_name="$1"
    tmux has-session -t "$session_name" 2>/dev/null
}

# Start a specific session for a VM
start_session_for_vm() {
    local vm="$1"
    local session_type="$2"
    
    case "$session_type" in
        developer)
            # univers-developer - from hvac-workbench
            tmux new-session -d -s "univers-developer" -c "/home/ubuntu/repos/hvac-workbench" 2>/dev/null || true
            ;;
        server)
            # univers-server - from hvac-workbench
            tmux new-session -d -s "univers-server" -c "/home/ubuntu/repos/hvac-workbench" 2>/dev/null || true
            ;;
        ui)
            # univers-ui - from hvac-workbench
            tmux new-session -d -s "univers-ui" -c "/home/ubuntu/repos/hvac-workbench" 2>/dev/null || true
            ;;
        web)
            # univers-web - from hvac-workbench
            tmux new-session -d -s "univers-web" -c "/home/ubuntu/repos/hvac-workbench" 2>/dev/null || true
            ;;
        operator)
            # univers-operator - from hvac-operation
            tmux new-session -d -s "univers-operator" -c "/home/ubuntu/repos/hvac-operation" 2>/dev/null || true
            ;;
        manager)
            # univers-manager - from univers-container
            tmux new-session -d -s "univers-manager" -c "/home/ubuntu/repos/univers-container" 2>/dev/null || true
            ;;
        agents)
            # univers-agents - from hvac-operation/univers-ark-agents
            tmux new-session -d -s "univers-agents" -c "/home/ubuntu/repos/hvac-operation/univers-ark-agents" 2>/dev/null || true
            ;;
        check)
            # univers-check - from univers-container
            tmux new-session -d -s "univers-check" -c "/home/ubuntu/repos/univers-container" 2>/dev/null || true
            ;;
        e2e)
            # univers-e2e - from univers-container
            tmux new-session -d -s "univers-e2e" -c "/home/ubuntu/repos/univers-container" 2>/dev/null || true
            ;;
        bench)
            # univers-bench - from univers-container
            tmux new-session -d -s "univers-bench" -c "/home/ubuntu/repos/univers-container" 2>/dev/null || true
            ;;
    esac
}

# Stop a specific session
stop_session() {
    local session_name="$1"
    if session_exists "$session_name"; then
        tmux kill-session -t "$session_name" 2>/dev/null || true
        print_info "已停止会话: $session_name"
    fi
}

# Start all sessions for all VMs
start_all_sessions() {
    print_header "启动所有容器会话"
    
    local vms=($(get_running_vms))
    if [ ${#vms[@]} -eq 0 ]; then
        print_warning "没有找到运行中的 dev 容器"
        return 1
    fi
    
    print_info "发现容器: ${vms[*]}"
    echo
    
    local total_sessions=0
    
    for vm in "${vms[@]}"; do
        print_info "处理容器: $vm"
        local session_types=($(get_session_types_for_vm "$vm"))
        
        for session_type in "${session_types[@]}"; do
            if ! session_exists "univers-$session_type" 2>/dev/null; then
                print_info "  启动: univers-$session_type"
                start_session_for_vm "$vm" "$session_type"
                ((total_sessions++))
            else
                print_info "  已存在: univers-$session_type"
            fi
        done
        echo
    done
    
    print_success "共启动 $total_sessions 个会话"
}

# Stop all sessions
stop_all_sessions() {
    print_header "停止所有容器会话"
    
    local sessions=("developer" "server" "ui" "web" "operator" "manager" "agents" "check" "e2e" "bench")
    local stopped_count=0
    
    for session_type in "${sessions[@]}"; do
        stop_session "univers-$session_type"
        ((stopped_count++))
    done
    
    print_success "共停止 $stopped_count 个会话类型"
}

# Show status of all sessions
show_status() {
    print_header "容器会话状态"
    
    local vms=($(get_running_vms))
    if [ ${#vms[@]} -eq 0 ]; then
        print_warning "没有找到运行中的 dev 容器"
        return 1
    fi
    
    echo "运行中的容器: ${vms[*]}"
    echo
    
    local sessions=("developer" "server" "ui" "web" "operator" "manager" "agents" "check" "e2e" "bench")
    local running_count=0
    local total_count=0
    
    echo "会话状态:"
    for session_type in "${sessions[@]}"; do
        ((total_count++))
        if session_exists "univers-$session_type"; then
            echo -e "  ${GREEN}✅ univers-$session_type${NC}"
            ((running_count++))
        else
            echo -e "  ${RED}❌ univers-$session_type${NC}"
        fi
    done
    
    echo
    print_info "运行中: $running_count/$total_count 个会话"
}

# Main command handling
case "${1:-status}" in
    start)
        start_all_sessions
        ;;
    stop)
        stop_all_sessions
        ;;
    restart)
        stop_all_sessions
        echo
        start_all_sessions
        ;;
    status)
        show_status
        ;;
    *)
        echo "用法: $0 <start|stop|restart|status>"
        echo
        echo "命令:"
        echo "  start   - 启动所有容器会话"
        echo "  stop    - 停止所有容器会话"
        echo "  restart - 重启所有容器会话"
        echo "  status  - 显示会话状态"
        echo
        echo "功能:"
        echo "  - 自动检测运行中的 *-dev 容器"
        echo "  - 为每个容器创建完整的会话集"
        echo "  - 支持大规模容器会话管理"
        exit 1
        ;;
esac
