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

### 7. Direct Container/VM Access via mm shell (RECOMMENDED)
**IMPORTANT: Always prioritize using `mm shell` for container operations!**

The `mm shell` command provides a unified interface for accessing containers/VMs with automatic:
- OS detection (Linux vs macOS)
- Correct user account handling (ubuntu for LXD, davidxu for OrbStack)
- Proper parameter quoting and escaping
- Support for both interactive shells and command execution

**Usage:**
```bash
# Interactive shell
mm shell hvac-dev

# Execute single command
mm shell hvac-dev "tmux list-sessions"

# Execute command with arguments
mm shell hvac-dev ls -la /home/ubuntu

# Execute complex commands
mm shell hvac-dev "tmux kill-session -t univers-developer && sleep 1 && echo 'Done'"
```

**Why mm shell is better than direct lxc/orb commands:**
1. ✅ Automatic user account selection (ubuntu vs davidxu)
2. ✅ Consistent across Linux and macOS
3. ✅ Proper shell escaping for special characters
4. ✅ Support for interactive and non-interactive modes
5. ✅ Error handling built-in

**Direct container access (legacy - use mm shell instead):**
- Linux: `lxc exec <name> -- su ubuntu -c '<command>'`
- macOS: `orb run --machine <name> bash -c '<command>'`
- ⚠️ These should only be used when mm shell is unavailable

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

1. **Detect the context**: Is this about tmux sessions, container lifecycle, resources, or container commands?

2. **Tmux sessions** (primary skill):
   - Use `mm` command for desktop-view, mobile-view, and machine-manage sessions
   - Ensure containers/VMs are running before starting sessions

3. **Container/VM command execution** (IMPORTANT - use mm shell):
   - ✅ **ALWAYS use `mm shell <container> '<command>'` for running commands in containers**
   - This provides:
     - Automatic OS detection (Linux vs macOS)
     - Correct user account handling (ubuntu for LXD, davidxu for OrbStack)
     - Proper parameter escaping and quoting
   - Examples:
     - `mm shell hvac-dev "tmux list-sessions"`
     - `mm shell hvac-dev "rm -rf /tmp/something"`
     - `mm shell hvac-dev "/path/to/script.sh arg1 arg2"`

4. **Complex container operations**:
   - Use scripts for lifecycle operations (clone, cleanup, batch-execute, list-resources)
   - Rely on `mm shell` for direct command execution
   - These scripts internally use the container-helper abstraction

5. **Provide clear feedback** about what was done

6. **Suggest related operations** if relevant (e.g., "Would you like me to clean up before cloning?")

## Examples

### Tmux Session Management
- "Start machine view sessions" → `mm start`
- "Show machine view status" → `mm status`
- "Attach to desktop view" → `mm attach desktop`
- "Attach to mobile view" → `mm attach mobile`
- "Restart all sessions" → `mm restart`

### Container Command Execution (use mm shell)
- "List tmux sessions in hvac-dev" → `mm shell hvac-dev "tmux list-sessions"`
- "Kill a tmux session" → `mm shell hvac-dev "tmux kill-session -t univers-developer"`
- "Run a script in container" → `mm shell hvac-dev "/path/to/script.sh arg1 arg2"`
- "Execute multiple commands" → `mm shell hvac-dev "cmd1 && cmd2 && cmd3"`
- "Check file status" → `mm shell hvac-dev "ls -la /path/to/file"`

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

1. **ALWAYS use `mm shell` for container commands** - This is the unified interface
   - ✅ `mm shell hvac-dev "tmux list-sessions"`
   - ❌ Don't use `lxc exec hvac-dev -- su ubuntu -c "tmux list-sessions"`
   - Exception: Only use direct lxc/orb commands if mm shell is temporarily unavailable

2. Always confirm destructive operations (delete, cleanup)

3. Check container/VM status before operations

4. Suggest cleanup before cloning large containers

5. Auto-detect the operating system and use appropriate commands (mm shell does this automatically)

6. Provide clear status updates

7. Handle errors gracefully and suggest alternatives

8. When managing machine views, ensure containers are running first

9. For complex container tasks, use specialized scripts (clone-vm.sh, cleanup-dev.sh, batch-execute.sh)

## Implementation Notes for Claude Code

When working with containers in hvac-dev or other LXD/OrbStack containers:
- **PRIORITY 1**: Use `mm shell <container> '<command>'`
- **PRIORITY 2**: Use dedicated scripts for specific tasks (clone, cleanup, etc.)
- **PRIORITY 3**: Use direct container system commands only when mm shell is unavailable
- Never mix direct `lxc` and `orb` commands without checking the OS first

When invoked, help the user manage their containers and machine-level tmux sessions efficiently and safely on both Linux (LXD) and macOS (OrbStack).
