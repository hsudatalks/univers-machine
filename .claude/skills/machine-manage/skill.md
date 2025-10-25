# Machine Management Skill

This skill provides comprehensive machine-level management for physical servers, including:
- **Tmux Session Aggregation**: Manage machine-level desktop and mobile view sessions
- **Container/VM Lifecycle**: Start, stop, clone, and delete containers/VMs
- **Resource Monitoring**: Check disk, memory, and resource usage
- **Cross-Platform Support**: Works on both Linux (LXD) and macOS (OrbStack)

## Core Capabilities

### 1. Machine-Level Tmux Session Management (PRIMARY SKILL)
- Create and manage aggregated tmux sessions that combine all container/VM sessions
- `machine-desktop-view` - Full information display for development
- `machine-mobile-view` - Simplified display for focused work
- `univers-machine-manage` - Physical machine management session
- Commands: `mm start`, `mm stop`, `mm status`, `mm attach desktop|mobile`, `mm restart`

### 2. Container/VM Lifecycle Management
You are an expert at managing containers and virtual machines:
- **Linux**: Use LXD containers (`lxc` commands)
- **macOS**: Use OrbStack VMs (`orb` commands)
- Automatically detects your OS and uses appropriate commands

### 3. Cross-Platform Abstraction
The skill includes a container abstraction library (`lib/container-helper.sh`) that:
- Detects OS (Linux vs macOS)
- Detects container system (LXD vs OrbStack)
- Provides unified commands that work on both platforms

## Available Operations

### 1. Tmux Session Management
**Start all sessions:**
- `mm start` - Create desktop-view, mobile-view, and machine-manage sessions
- Automatically starts containers/VMs if needed

**Manage sessions:**
- `mm status` - Show status of all machine view sessions
- `mm attach desktop` - Connect to desktop view
- `mm attach mobile` - Connect to mobile view
- `mm stop` - Stop all sessions
- `mm restart` - Restart all sessions

**Navigation in tmux:**
- Ctrl+B then 0-4 - Switch between windows
- Ctrl+B then w - List all windows
- Ctrl+B then n/p - Next/previous window
- Ctrl+B then D - Detach

### 2. Container/VM Lifecycle
**List containers:**
- Linux: `lxc list`
- macOS: `orb list`
- Skill automatically uses correct command

**Lifecycle operations:**
- **Start**: `container_start <name>` (automatic OS detection)
- **Stop**: `container_stop <name>`
- **Restart**: Container operations
- **Delete**: `container_delete <name>`

### 3. Clone Containers/VMs
- Use `scripts/clone-vm.sh <source> <dest> [--cleanup]`
- Optional cleanup of source before cloning
- Works on both Linux (LXD) and macOS (OrbStack)

### 4. Cleanup Operations
Run `scripts/cleanup-dev.sh <container>` to:
- Remove build artifacts (target/ directories)
- Clear sccache and Cargo caches
- Clean apt package caches
- Remove temporary files

### 5. Resource Monitoring
- Use `scripts/list-resources.sh` to show disk, memory, and large directories
- Works on both LXD containers and OrbStack VMs
- Displays resource usage for all running containers/VMs

### 6. Batch Operations
- Use `scripts/batch-execute.sh '<command>' [--all|<container1> <container2>...]`
- Execute commands across multiple containers
- Works on both Linux and macOS

### 7. Direct Container Access
- Linux: `lxc exec <name> -- /bin/bash` for shell access
- macOS: `orb shell <name>` for SSH access
- Skill automatically uses appropriate command

## Scripts Available

All scripts auto-detect the OS and container system, working seamlessly on both Linux (LXD) and macOS (OrbStack):

- `scripts/machine-view-manager.sh` - **PRIMARY**: Manage machine-level tmux sessions
  - Start/stop/restart desktop-view, mobile-view, and machine-manage sessions
  - Used via `mm` alias for convenience

- `scripts/cleanup-dev.sh <container>` - Clean development artifacts
  - Remove build artifacts, caches, temporary files
  - Works on both LXD and OrbStack

- `scripts/clone-vm.sh <source> <dest> [--cleanup]` - Clone containers/VMs
  - Optional cleanup before cloning
  - Efficient cloning on both platforms

- `scripts/batch-execute.sh '<cmd>' [--all|<containers>]` - Execute commands across multiple containers
  - Run on all running containers, or specific ones
  - Cross-platform compatible

- `scripts/list-resources.sh` - Show disk, memory, and resource usage
  - Works on both LXD containers and OrbStack VMs

## Support Library

- `lib/container-helper.sh` - Cross-platform abstraction layer
  - Detects OS and container system automatically
  - Provides unified container management functions
  - Used by all scripts for platform-agnostic operations

## Usage

When the user asks to manage containers, VMs, or tmux sessions:

1. **Detect the context**: Is this about tmux sessions, container lifecycle, or resources?
2. **Tmux sessions** (primary skill):
   - Use `mm` command for desktop-view, mobile-view, and machine-manage sessions
   - Ensure containers/VMs are running before starting sessions
3. **Container operations**:
   - Auto-detect OS (Linux vs macOS)
   - Use appropriate commands (lxc on Linux, orb on macOS)
   - Use scripts for complex operations (clone, cleanup, batch-execute, list-resources)
4. **Provide clear feedback** about what was done
5. **Suggest related operations** if relevant (e.g., "Would you like me to clean up before cloning?")

## Examples

### Tmux Session Management
- "Start machine view sessions" → `mm start`
- "Show machine view status" → `mm status`
- "Attach to desktop view" → `mm attach desktop`
- "Attach to mobile view" → `mm attach mobile`
- "Restart all sessions" → `mm restart`

### Container Lifecycle
- "List all containers" → Use appropriate command (lxc list on Linux, orb list on macOS)
- "Clone ubuntu to web-dev" → `scripts/clone-vm.sh ubuntu web-dev` (with optional --cleanup)
- "Clean up ubuntu" → `scripts/cleanup-dev.sh ubuntu`
- "Delete a container" → Confirm, then execute with appropriate command
- "Start all stopped containers" → Parse list and start each using correct command

### Resource & Batch Operations
- "Show resource usage" → `scripts/list-resources.sh`
- "Run command on all containers" → `scripts/batch-execute.sh '<command>' --all`
- "Check container status" → Automatically detects OS and uses lxc or orb

## Best Practices

1. Always confirm destructive operations (delete, cleanup)
2. Check container/VM status before operations
3. Suggest cleanup before cloning large containers
4. Auto-detect the operating system and use appropriate commands
5. Provide clear status updates
6. Handle errors gracefully and suggest alternatives
7. When managing machine views, ensure containers are running first

When invoked, help the user manage their containers and machine-level tmux sessions efficiently and safely on both Linux (LXD) and macOS (OrbStack).
