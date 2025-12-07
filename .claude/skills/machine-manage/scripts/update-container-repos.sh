#!/bin/bash
# Container Repository Updater
# 更新容器内的代码库到最新版本

set -e

# Source helper library
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
RED='\033[0;31m'
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

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Update repositories in a specific container
update_container_repos() {
    local container="$1"
    local repos=("$@")
    repos=("${repos[@]:1}") # Remove first element (container name)
    
    print_header "更新容器 $container 的代码库"
    
    for repo in "${repos[@]}"; do
        print_info "更新 $repo..."
        
        # Check if repo directory exists
        if ! container_exec "$container" "test -d ~/repos/$repo" 2>/dev/null; then
            print_warning "  → $repo 目录不存在，跳过"
            continue
        fi
        
        # Change to repo directory and pull
        if container_exec "$container" "cd ~/repos/$repo && git remote update && git status" 2>/dev/null; then
            # Check if we need to pull
            local status_output=$(container_exec "$container" "cd ~/repos/$repo && git status --porcelain" 2>/dev/null || echo "")
            
            if [ -n "$status_output" ]; then
                print_warning "  → $repo 有未提交的更改，跳过拉取"
                continue
            fi
            
            # Get current branch and pull
            local current_branch=$(container_exec "$container" "cd ~/repos/$repo && git rev-parse --abbrev-ref HEAD" 2>/dev/null || echo "unknown")
            if [ "$current_branch" = "master" ] || [ "$current_branch" = "main" ]; then
                if container_exec "$container" "cd ~/repos/$repo && git pull origin $current_branch" 2>/dev/null; then
                    print_success "  → $repo ($current_branch) 已更新"
                else
                    print_warning "  → $repo ($current_branch) 更新失败"
                fi
            else
                print_warning "  → $repo 当前分支: $current_branch (非main/master)，跳过"
            fi
        else
            print_warning "  → $repo 不是git仓库或无法访问"
        fi
    done
}

# Update all containers
update_all_containers() {
    local vms=($(load_vm_list))
    
    if [ ${#vms[@]} -eq 0 ]; then
        print_error "没有找到运行中的 dev VMs"
        exit 1
    fi

    echo "=== 更新所有容器的代码库 ==="
    echo "目标VM: ${vms[@]}"
    echo

    # Repositories to update
    local repos=("univers-container" "hvac-operation" "hvac-workbench")
    
    for vm in "${vms[@]}"; do
        update_container_repos "$vm" "${repos[@]}"
        echo
    done
    
    print_success "所有容器代码库更新完成！"
}

# Update specific container
update_specific_container() {
    local container="$1"
    
    if ! container_is_running "$container"; then
        print_error "容器 '$container' 未运行"
        exit 1
    fi

    echo "=== 更新容器 $container 的代码库 ==="
    
    # Repositories to update
    local repos=("univers-container" "hvac-operation" "hvac-workbench")
    
    update_container_repos "$container" "${repos[@]}"
    
    print_success "容器 $container 代码库更新完成！"
}

# Main command handling
case "${1:-all}" in
    all)
        update_all_containers
        ;;
    *)
        if [ -z "$1" ]; then
            echo "❌ Error: container name required"
            echo "Usage: $0 <container_name|all>"
            exit 1
        fi
        update_specific_container "$1"
        ;;
esac