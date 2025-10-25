#!/bin/bash
# Container Management Helper Library
# Abstracts differences between LXD (Linux) and OrbStack (macOS)
# Source this file in scripts that need container operations

# Detect the operating system
detect_os() {
    local os_type
    case "$(uname -s)" in
        Linux*)  os_type="linux" ;;
        Darwin*) os_type="macos" ;;
        *)       os_type="unknown" ;;
    esac
    echo "$os_type"
}

# Detect the container system
detect_container_system() {
    local os="$(detect_os)"

    case "$os" in
        linux)
            # On Linux, check for LXD
            if command -v lxc &> /dev/null; then
                echo "lxd"
            else
                echo "none"
            fi
            ;;
        macos)
            # On macOS, check for OrbStack
            if command -v orb &> /dev/null || command -v orbctl &> /dev/null; then
                echo "orbstack"
            else
                echo "none"
            fi
            ;;
        *)
            echo "none"
            ;;
    esac
}

# Print system info
print_system_info() {
    local os="$(detect_os)"
    local container_system="$(detect_container_system)"

    case "$os" in
        linux)
            case "$container_system" in
                lxd)
                    echo "Linux + LXD"
                    ;;
                *)
                    echo "Linux (LXD not found)"
                    ;;
            esac
            ;;
        macos)
            case "$container_system" in
                orbstack)
                    echo "macOS + OrbStack"
                    ;;
                *)
                    echo "macOS (OrbStack not found)"
                    ;;
            esac
            ;;
        *)
            echo "Unknown OS"
            ;;
    esac
}

# List containers/VMs
container_list() {
    local container_system="$(detect_container_system)"

    case "$container_system" in
        lxd)
            lxc list --format=json | jq -r '.[] | "\(.name) \(.status)"'
            ;;
        orbstack)
            orb list
            ;;
        *)
            echo "Error: No container system detected" >&2
            return 1
            ;;
    esac
}

# Check if container/VM exists
container_exists() {
    local name="$1"
    local container_system="$(detect_container_system)"

    case "$container_system" in
        lxd)
            lxc list --format=json | jq -e ".[] | select(.name == \"$name\")" > /dev/null 2>&1
            ;;
        orbstack)
            orb list | grep -q "^$name "
            ;;
        *)
            return 1
            ;;
    esac
}

# Check if container/VM is running
container_is_running() {
    local name="$1"
    local container_system="$(detect_container_system)"

    case "$container_system" in
        lxd)
            local status=$(lxc list --format=json | jq -r ".[] | select(.name == \"$name\") | .status" 2>/dev/null)
            [ "$status" = "Running" ]
            ;;
        orbstack)
            orb list | grep "^$name " | grep -q " running "
            ;;
        *)
            return 1
            ;;
    esac
}

# Execute command in container/VM
container_exec() {
    local name="$1"
    shift
    local container_system="$(detect_container_system)"

    case "$container_system" in
        lxd)
            lxc exec "$name" -- "$@"
            ;;
        orbstack)
            orb run "$name" "$@"
            ;;
        *)
            echo "Error: No container system detected" >&2
            return 1
            ;;
    esac
}

# Start a container/VM
container_start() {
    local name="$1"
    local container_system="$(detect_container_system)"

    case "$container_system" in
        lxd)
            lxc start "$name"
            ;;
        orbstack)
            orb start "$name"
            ;;
        *)
            echo "Error: No container system detected" >&2
            return 1
            ;;
    esac
}

# Stop a container/VM
container_stop() {
    local name="$1"
    local container_system="$(detect_container_system)"

    case "$container_system" in
        lxd)
            lxc stop "$name"
            ;;
        orbstack)
            orb stop "$name"
            ;;
        *)
            echo "Error: No container system detected" >&2
            return 1
            ;;
    esac
}

# Clone a container/VM
container_clone() {
    local source="$1"
    local target="$2"
    local container_system="$(detect_container_system)"

    case "$container_system" in
        lxd)
            lxc copy "$source" "$target"
            ;;
        orbstack)
            orb clone "$source" "$target"
            ;;
        *)
            echo "Error: No container system detected" >&2
            return 1
            ;;
    esac
}

# Delete a container/VM
container_delete() {
    local name="$1"
    local container_system="$(detect_container_system)"

    case "$container_system" in
        lxd)
            lxc delete "$name" --force
            ;;
        orbstack)
            orb delete "$name"
            ;;
        *)
            echo "Error: No container system detected" >&2
            return 1
            ;;
    esac
}

# Open shell in container/VM
container_shell() {
    local name="$1"
    local container_system="$(detect_container_system)"

    case "$container_system" in
        lxd)
            lxc exec "$name" -- /bin/bash
            ;;
        orbstack)
            orb shell "$name"
            ;;
        *)
            echo "Error: No container system detected" >&2
            return 1
            ;;
    esac
}

# Color codes
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[0;33m'
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
