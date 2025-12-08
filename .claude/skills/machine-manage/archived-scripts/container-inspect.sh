#!/bin/bash

# Container Inspection Script
# 检查容器内部的服务和会话状态

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check if container name is provided
if [ $# -lt 1 ]; then
    echo "Usage: $0 <container-name>"
    exit 1
fi

CONTAINER="$1"

# Helper functions
print_header() {
    echo -e "${BLUE}━━━ $1 ━━━${NC}"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

# Detect container system (LXD or OrbStack)
if command -v lxc &> /dev/null && lxc list --format csv | grep -q "^$CONTAINER,"; then
    CONTAINER_CMD="lxc exec $CONTAINER -- su - ubuntu -c"
elif command -v orbctl &> /dev/null && orbctl list | grep -q "$CONTAINER"; then
    CONTAINER_CMD="orbctl run --machine $CONTAINER"
else
    print_error "Container $CONTAINER not found"
    exit 1
fi

# Check if Claude Code is working
check_claude_working() {
    # Find claude process
    local claude_pid=$($CONTAINER_CMD 'pgrep -x claude' 2>/dev/null | head -1 || echo "")

    if [ -z "$claude_pid" ]; then
        echo "${RED}[Not Running]${NC}"
        return
    fi

    # Get CPU usage - use ps for more reliable output
    local cpu_usage=$($CONTAINER_CMD "ps -p $claude_pid -o %cpu= 2>/dev/null | tr -d ' '" || echo "0")

    # Get connection count
    local connections=$($CONTAINER_CMD "lsof -p $claude_pid 2>/dev/null | grep -c ESTABLISHED || true")

    # Remove decimal point for comparison (convert 18.5 to 18)
    local cpu_int=${cpu_usage%.*}

    # Handle empty or non-numeric values
    if [[ ! "$cpu_int" =~ ^[0-9]+$ ]]; then
        cpu_int=0
    fi

    # Determine status based on CPU and connections
    if [ "$cpu_int" -gt 5 ] || [ "$connections" -gt 2 ]; then
        echo "${GREEN}[Working - CPU: ${cpu_usage}%, Conns: ${connections}]${NC}"
    else
        echo "${YELLOW}[Idle - CPU: ${cpu_usage}%, Conns: ${connections}]${NC}"
    fi
}

# Check tmux sessions
check_tmux_sessions() {
    print_header "Tmux Sessions in $CONTAINER"
    
    local sessions=$($CONTAINER_CMD 'tmux list-sessions 2>/dev/null' || echo "")
    
    if [ -z "$sessions" ]; then
        print_warning "No tmux sessions found"
        return
    fi
    
    # Check specific sessions
    local important_sessions=(
        "univers-developer"
        "univers-server"
        "univers-web"
        "univers-ui"
        "container-desktop-view"
        "container-mobile-view"
    )
    
    for session in "${important_sessions[@]}"; do
        if echo "$sessions" | grep -q "^$session:"; then
            local info=$(echo "$sessions" | grep "^$session:" | sed 's/^[^:]*: //')

            # Special check for univers-developer (Claude Code session)
            if [ "$session" = "univers-developer" ]; then
                local claude_status=$(check_claude_working)
                print_success "$session - $info $claude_status"
            else
                print_success "$session - $info"
            fi
        fi
    done
    
    echo ""
}

# Check workbench service
check_workbench() {
    print_header "Workbench Service Status"
    
    # Check if workbench dev server is running
    local workbench_proc=$($CONTAINER_CMD 'ps aux | grep -E "pnpm.*workbench.*dev|vite.*--host" | grep -v grep' || echo "")
    
    if [ -n "$workbench_proc" ]; then
        print_success "Workbench dev server is running"
        
        # Check if port 5173 (vite default) is listening
        local port_check=$($CONTAINER_CMD 'ss -tlnp 2>/dev/null | grep ":5173"' || echo "")
        if [ -n "$port_check" ]; then
            print_success "Vite server listening on port 5173"
        fi
    else
        print_warning "Workbench dev server not running"
    fi
    
    echo ""
}

# Check running processes
check_processes() {
    print_header "Key Processes"
    
    # Check for Rust compilation
    local rust_proc=$($CONTAINER_CMD 'ps aux | grep -E "rustc|cargo" | grep -v grep | wc -l' || echo "0")
    if [ "$rust_proc" -gt 0 ]; then
        print_success "Rust compilation in progress ($rust_proc processes)"
    fi
    
    # Check for Node processes
    local node_proc=$($CONTAINER_CMD 'ps aux | grep -E "node.*vite|pnpm" | grep -v grep | wc -l' || echo "0")
    if [ "$node_proc" -gt 0 ]; then
        print_success "Node.js processes running ($node_proc processes)"
    fi
    
    echo ""
}

# Check system resources in container
check_resources() {
    print_header "Resource Usage"
    
    # Get CPU and memory usage
    local cpu_usage=$($CONTAINER_CMD 'top -bn1 | grep "Cpu(s)" | sed "s/.*, *\([0-9.]*\)%* id.*/\1/" | awk "{print 100 - \$1}"' 2>/dev/null || echo "N/A")
    local mem_usage=$($CONTAINER_CMD 'free | grep Mem | awk "{print \$3/\$2 * 100.0}"' 2>/dev/null || echo "N/A")
    
    echo -e "  CPU: ${YELLOW}${cpu_usage}%${NC}"
    echo -e "  Memory: ${YELLOW}${mem_usage}%${NC}"
    
    echo ""
}

# Main execution
echo ""
print_header "Container Inspection: $CONTAINER"
echo ""

check_tmux_sessions
check_workbench
check_processes
check_resources

echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
