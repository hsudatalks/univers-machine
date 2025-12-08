#!/bin/bash
# Install ckimi function to remote servers and containers
# This script will deploy the ckimi function to all environments

set -e

# Color codes
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# ckimi function definition
CKIMI_FUNCTION='#!/bin/bash
# ckimi - Claude with Kimi API configuration
ckimi() {
    # Set Kimi API configuration
    export ANTHROPIC_BASE_URL=https://api.kimi.com/coding/
    export ANTHROPIC_API_KEY=sk-kimi-WXG4X2V1myVl1to5IZLVctjwFQHHU61lICbuX5rvPlfh1fg1Xku8F6TbOyahNo64

    # Check if -c flag is present
    local has_c_flag=false
    local args=()

    for arg in "$@"; do
        if [[ "$arg" == "-c" ]]; then
            has_c_flag=true
        fi
        args+=("$arg")
    done

    # If -c flag is not provided, add it
    if [[ "$has_c_flag" == false ]]; then
        args=("-c" "${args[@]}")
    fi

    # Execute claude with dangerously-skip-permissions and all arguments
    claude --dangerously-skip-permissions "${args[@]}"
}
'

# Install ckimi function to a remote server
install_to_remote_server() {
    local server="$1"
    print_info "Installing ckimi function to $server..."
    
    # Check if server is accessible
    if ! ssh -o ConnectTimeout=5 -o BatchMode=yes "$server" "echo 'Connection successful'" 2>/dev/null; then
        print_warning "Cannot connect to $server, skipping..."
        return 1
    fi
    
    # Check if claude is installed (check multiple locations)
    if ! ssh "$server" "which claude" 2>/dev/null && ! ssh "$server" "ls ~/.local/bin/claude" 2>/dev/null && ! ssh "$server" "ls ~/.npm-global/bin/claude" 2>/dev/null; then
        print_warning "claude not found on $server, skipping..."
        return 1
    fi
    
    # Backup existing .zshrc
    ssh "$server" "cp ~/.zshrc ~/.zshrc.bak.$(date +%Y%m%d_%H%M%S) 2>/dev/null || true"
    
    # Remove existing ckimi function if exists
    ssh "$server" "sed -i '/^# ckimi - Claude with Kimi API configuration/,/^}$/d' ~/.zshrc 2>/dev/null || true"
    
    # Add ckimi function to .zshrc
    ssh "$server" "echo '' >> ~/.zshrc"
    ssh "$server" "echo '# ckimi - Claude with Kimi API configuration' >> ~/.zshrc"
    ssh "$server" "echo 'ckimi() {' >> ~/.zshrc"
    ssh "$server" "echo '    # Set Kimi API configuration' >> ~/.zshrc"
    ssh "$server" "echo '    export ANTHROPIC_BASE_URL=https://api.kimi.com/coding/' >> ~/.zshrc"
    ssh "$server" "echo '    export ANTHROPIC_API_KEY=sk-kimi-WXG4X2V1myVl1to5IZLVctjwFQHHU61lICbuX5rvPlfh1fg1Xku8F6TbOyahNo64' >> ~/.zshrc"
    ssh "$server" "echo '' >> ~/.zshrc"
    ssh "$server" "echo '    # Execute claude with dangerously-skip-permissions and all arguments' >> ~/.zshrc"
    ssh "$server" "echo '    # Try different claude locations' >> ~/.zshrc"
    ssh "$server" "echo '    if command -v claude >/dev/null 2>&1; then' >> ~/.zshrc"
    ssh "$server" "echo '        claude --dangerously-skip-permissions \"\$@\"' >> ~/.zshrc"
    ssh "$server" "echo '    elif [ -f ~/.local/bin/claude ]; then' >> ~/.zshrc"
    ssh "$server" "echo '        ~/.local/bin/claude --dangerously-skip-permissions \"\$@\"' >> ~/.zshrc"
    ssh "$server" "echo '    elif [ -f ~/.npm-global/bin/claude ]; then' >> ~/.zshrc"
    ssh "$server" "echo '        ~/.npm-global/bin/claude --dangerously-skip-permissions \"\$@\"' >> ~/.zshrc"
    ssh "$server" "echo '    else' >> ~/.zshrc"
    ssh "$server" "echo '        echo \"Error: claude not found\" >&2' >> ~/.zshrc"
    ssh "$server" "echo '        return 1' >> ~/.zshrc"
    ssh "$server" "echo '    fi' >> ~/.zshrc"
    ssh "$server" "echo '}' >> ~/.zshrc"
    
    # Test the function
    if ssh "$server" "zsh -c 'source ~/.zshrc && which ckimi'" >/dev/null 2>&1; then
        print_success "ckimi function installed successfully on $server"
        return 0
    else
        print_error "Failed to install ckimi function on $server"
        return 1
    fi
}

# Install ckimi function to a container
install_to_container() {
    local container="$1"
    print_info "Installing ckimi function to container $container..."
    
    # Source the helper library
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    source "$SCRIPT_DIR/lib/container-helper.sh"
    
    # Check if container is running
    if ! container_is_running "$container"; then
        print_warning "Container $container is not running, skipping..."
        return 1
    fi
    
    # Check if claude is installed in container (check multiple locations)
    if ! container_exec "$container" "which claude" 2>/dev/null && ! container_exec "$container" "ls ~/.npm-global/bin/claude" 2>/dev/null; then
        print_warning "claude not found in container $container, skipping..."
        return 1
    fi
    
    # Backup existing .zshrc
    container_exec "$container" "cp ~/.zshrc ~/.zshrc.bak.$(date +%Y%m%d_%H%M%S) 2>/dev/null || true"
    
    # Remove existing ckimi function if exists
    container_exec "$container" "sed -i '/^# ckimi - Claude with Kimi API configuration/,/^}$/d' ~/.zshrc 2>/dev/null || true"
    
    # Add ckimi function to .zshrc
    container_exec "$container" "echo '' >> ~/.zshrc"
    container_exec "$container" "echo '# ckimi - Claude with Kimi API configuration' >> ~/.zshrc"
    container_exec "$container" "echo 'ckimi() {' >> ~/.zshrc"
    container_exec "$container" "echo '    # Set Kimi API configuration' >> ~/.zshrc"
    container_exec "$container" "echo '    export ANTHROPIC_BASE_URL=https://api.kimi.com/coding/' >> ~/.zshrc"
    container_exec "$container" "echo '    export ANTHROPIC_API_KEY=sk-kimi-WXG4X2V1myVl1to5IZLVctjwFQHHU61lICbuX5rvPlfh1fg1Xku8F6TbOyahNo64' >> ~/.zshrc"
    container_exec "$container" "echo '' >> ~/.zshrc"
    container_exec "$container" "echo '    # Execute claude with dangerously-skip-permissions and all arguments' >> ~/.zshrc"
    container_exec "$container" "echo '    # Try different claude locations' >> ~/.zshrc"
    container_exec "$container" "echo '    if command -v claude >/dev/null 2>&1; then' >> ~/.zshrc"
    container_exec "$container" "echo '        claude --dangerously-skip-permissions \"\$@\"' >> ~/.zshrc"
    container_exec "$container" "echo '    elif [ -f ~/.local/bin/claude ]; then' >> ~/.zshrc"
    container_exec "$container" "echo '        ~/.local/bin/claude --dangerously-skip-permissions \"\$@\"' >> ~/.zshrc"
    container_exec "$container" "echo '    elif [ -f ~/.npm-global/bin/claude ]; then' >> ~/.zshrc"
    container_exec "$container" "echo '        ~/.npm-global/bin/claude --dangerously-skip-permissions \"\$@\"' >> ~/.zshrc"
    container_exec "$container" "echo '    else' >> ~/.zshrc"
    container_exec "$container" "echo '        echo \"Error: claude not found\" >&2' >> ~/.zshrc"
    container_exec "$container" "echo '        return 1' >> ~/.zshrc"
    container_exec "$container" "echo '    fi' >> ~/.zshrc"
    container_exec "$container" "echo '}' >> ~/.zshrc"
    
    # Test the function
    if container_exec "$container" "zsh -c 'source ~/.zshrc && which ckimi'" >/dev/null 2>&1; then
        print_success "ckimi function installed successfully in container $container"
        return 0
    else
        print_error "Failed to install ckimi function in container $container"
        return 1
    fi
}

# Main installation function
main() {
    echo "=== Installing ckimi function to all environments ==="
    echo
    
    # List of remote servers (infra-dev and other dev servers)
    # Note: eu2-ops and sg1-ops may have different user/access requirements
    REMOTE_SERVERS=("infra-dev" "domain-dev" "mechanism-dev" "scenario-dev")
    
    # List of local containers (from orb list output, excluding ubuntu)
    LOCAL_CONTAINERS=("server-dev" "ui-dev" "web-dev")
    
    # Install to remote servers
    print_info "Installing to remote SSH servers..."
    echo
    for server in "${REMOTE_SERVERS[@]}"; do
        install_to_remote_server "$server"
        echo
    done
    
    # Install to local containers
    print_info "Installing to local containers..."
    echo
    for container in "${LOCAL_CONTAINERS[@]}"; do
        install_to_container "$container"
        echo
    done
    
    print_success "Installation completed!"
    echo
    print_info "Usage examples:"
    echo "  # On remote servers:"
    echo "  ssh infra-dev 'zsh -c \"source ~/.zshrc && ckimi \\\"Hello\\\"\"'"
    echo
    echo "  # In local containers:"
    echo "  mm shell server-dev 'zsh -c \"source ~/.zshrc && ckimi \\\"Hello\\\"\"'"
    echo
    echo "  # Or source .zshrc first:"
    echo "  ssh infra-dev 'source ~/.zshrc && ckimi \"Hello\"'"
    echo "  mm shell server-dev 'source ~/.zshrc && ckimi \"Hello\"'"
}

# Run main function
main "$@"