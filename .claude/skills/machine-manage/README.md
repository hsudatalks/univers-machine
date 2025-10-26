# Machine Management Skill

This skill provides comprehensive machine-level management for physical servers, including container/VM lifecycle management, tmux session aggregation, and resource monitoring.

**Supports both:**
- **Linux**: LXD containers (using `lxc` commands)
- **macOS**: OrbStack VMs (using `orb` commands)

## Installation

Run the installation script to set up the `machine-manager` command:

```bash
.claude/skills/machine-manage/install.sh
```

This will:
- Add `machine-manage.zsh` to your `~/.zshrc` (no sudo required)
- Initialize configuration file from example if not exists
- Verify installation

After installation, reload your shell:

```bash
source ~/.zshrc
```

Then you can use `machine-manager` from anywhere:

```bash
machine-manager start    # 启动所有 machine view 会话
machine-manager status   # 查看状态
machine-manager attach desktop  # 连接桌面视图
```

**Note:** The command is registered as a zsh function, so it only works in zsh. Tab completion is also supported.

## What This Skill Does

When you invoke the `machine-manage` skill in Claude Code, it gives Claude specialized knowledge and tools to manage your OrbStack VMs and machine-level tmux sessions efficiently.

## Available Scripts

### 1. `cleanup-dev.sh`
Clean development artifacts and caches from a VM.

```bash
./scripts/cleanup-dev.sh <vm-name> [--dry-run]
```

**What it cleans:**
- Rust/Cargo build artifacts (`target/` directories)
- sccache caches
- Cargo registry caches
- APT package caches
- Temporary files
- Docker system (if installed)

**Example:**
```bash
# Dry run to see what would be cleaned
./scripts/cleanup-dev.sh ubuntu --dry-run

# Actually clean
./scripts/cleanup-dev.sh ubuntu
```

### 2. `list-resources.sh`
Show resource usage across all running VMs.

```bash
./scripts/list-resources.sh
```

**Shows:**
- Disk usage
- Memory usage
- Large directories (>100MB)

### 3. `clone-vm.sh`
Clone a VM with optional cleanup.

```bash
./scripts/clone-vm.sh <source-vm> <new-vm-name> [--cleanup]
```

**Example:**
```bash
# Clone without cleanup
./scripts/clone-vm.sh ubuntu web-dev

# Clone with cleanup first
./scripts/clone-vm.sh ubuntu integration-dev --cleanup
```

### 4. `batch-execute.sh`
Execute commands across multiple VMs.

```bash
./scripts/batch-execute.sh <command> [vm1 vm2 ...] or [--all]
```

**Examples:**
```bash
# Run on all VMs
./scripts/batch-execute.sh 'uptime' --all

# Run on specific VMs
./scripts/batch-execute.sh 'df -h /' web-dev integration-dev

# Update packages on multiple VMs
./scripts/batch-execute.sh 'sudo apt-get update' ubuntu web-dev
```

### 5. `machine-view-manager.sh`
Manage machine-level tmux sessions that aggregate VM tmux sessions.

```bash
./scripts/machine-view-manager.sh <start|stop|status|attach|restart>
```

**Styling:**
Machine-level sessions use custom blue-themed styles to distinguish them from VM sessions (which use orange-yellow theme):

**Desktop View** (`machine-desktop-tmux-style.conf`):
- **Status bar**: Full information display
- **Left**: 🖥️ Machine | session-name |
- **Right**: VM count, window info, date & time
- **Active window**: Bright blue with ✓
- **Inactive window**: Gray-blue

**Mobile View** (`machine-mobile-tmux-style.conf`):
- **Status bar**: Simplified - only window list
- **Left**: (empty)
- **Right**: (empty)
- Focus on window navigation without distractions

**What it does:**
Creates machine-level tmux sessions on your physical Mac that aggregate all VM tmux sessions:
- `machine-desktop-view` - Aggregates all `container-desktop-view` sessions from dev VMs
- `machine-mobile-view` - Aggregates all `container-mobile-view` sessions from dev VMs
- `univers-machine-manage` - A local session for managing the physical machine (added to both views)

Each session has 5 windows:
- Window 0: integration-dev (VM)
- Window 1: web-dev (VM)
- Window 2: control-dev (VM)
- Window 3: validation-dev (VM)
- Window 4: machine-manage (物理机管理 - local session)

**Features:**

- **Dynamic Container Detection**: Automatically detects all running containers/VMs
- **Intelligent Window Management**: Dynamically creates/removes windows based on running containers
- **Live Refresh**: Update windows without exiting tmux using the `refresh` command

**Examples:**
```bash
# Start machine view sessions (auto-detects containers)
./scripts/machine-view-manager.sh start

# Check status
./scripts/machine-view-manager.sh status

# Attach to desktop view
./scripts/machine-view-manager.sh attach desktop

# Attach to mobile view
./scripts/machine-view-manager.sh attach mobile

# Refresh windows while in tmux (without exiting)
# In tmux, you can run:
./scripts/machine-view-manager.sh refresh
# Or via mm alias:
mm refresh

# Restart all sessions
./scripts/machine-view-manager.sh restart

# Stop all sessions
./scripts/machine-view-manager.sh stop
```

**Smart Window Management:**

The machine-view-manager now automatically:
- Detects all currently running containers/VMs
- Creates one window per running container
- Adds/removes windows dynamically when containers start/stop
- Preserves the `machine-manage` window for physical machine management

You can refresh windows without leaving tmux by running `mm refresh` from within a tmux session.

**Tmux Navigation:**
- `Ctrl+B` then `0-4` - Switch to specific window (0-3: VMs, 4: machine-manage)
- `Ctrl+B` then `w` - Show all windows
- `Ctrl+B` then `n` - Next window
- `Ctrl+B` then `p` - Previous window
- `Ctrl+B` then `D` - Detach from session

**Working Directory:**
- The `univers-machine-manage` session runs in `/Users/davidxu/repos/univers-machine`
- This allows you to manage the physical machine configuration and scripts directly

**Technical Details:**
- Window 4 (machine-manage) uses `unset TMUX && tmux attach` to properly nest the `univers-machine-manage` session
- This allows tmux-in-tmux without conflicts

## Usage in Claude Code

Simply mention container or VM management tasks, and Claude will use this skill automatically:

- "List all my VMs and their resource usage"
- "Clone ubuntu to web-dev and clean it up first"
- "Clean up the ubuntu VM before cloning"
- "Run 'docker ps' on all VMs"
- "Delete the debian VM"

## Configuration Files

### `configs/machine-desktop-tmux-style.conf`
Tmux style configuration for machine-desktop-view. Automatically applied when creating the desktop view session.

**Features:**
- Blue color scheme (distinguishes from VM's orange-yellow theme)
- Full status bar with machine identifier, VM count, time, etc.
- Window navigation shortcuts (Ctrl+Y/U, Alt+0-4)
- Scroll shortcuts (Ctrl+J/K)

### `configs/machine-mobile-tmux-style.conf`
Tmux style configuration for machine-mobile-view. Simplified version for focused window navigation.

**Features:**
- Blue color scheme (consistent with desktop view)
- Minimal status bar - only window list (no left/right info)
- Same navigation shortcuts as desktop view
- Optimized for distraction-free window switching

**Customization:**
You can edit these files to change:
- Colors (see color reference in the files)
- Status bar content and format
- Key bindings
- Other tmux options

Changes will apply to newly created sessions.

## Direct Script Usage

You can also run the scripts directly from the terminal:

```bash
cd .claude/skills/machine-manage
./scripts/list-resources.sh
```

## Setup: Container Commands

To ensure cm and other container commands work properly with bash, we create a symlink in /usr/local/bin:

```bash
# This is done automatically during initial container setup
mm shell hvac-dev "sudo ln -sf /home/ubuntu/repos/univers-container/.claude/skills/container-manage/bin/cm /usr/local/bin/cm"
```

This allows cm to be found in PATH regardless of shell or execution context.

## Using mm shell (RECOMMENDED for Container Commands)

The `mm shell` command provides a unified interface for executing commands in containers/VMs with automatic OS detection and proper user account handling.

### Why use mm shell?

✅ **Automatic features:**
- OS detection (Linux vs macOS)
- Correct user account (ubuntu for LXD, davidxu for OrbStack)
- Proper parameter escaping and quoting
- Support for interactive and non-interactive modes

❌ **Don't use:**
- Direct `lxc exec` commands
- Direct `orb run` commands
- These require manual OS detection and user account handling

### Usage Examples

```bash
# Interactive shell
mm shell hvac-dev

# List tmux sessions
mm shell hvac-dev "tmux list-sessions"

# Kill a tmux session
mm shell hvac-dev "tmux kill-session -t univers-developer"

# Run a script
mm shell hvac-dev "/path/to/script.sh arg1 arg2"

# Complex commands
mm shell hvac-dev "cd /home/ubuntu && ls -la"
```

### Error Handling

```bash
# If container doesn't exist
mm shell nonexistent-container "echo test"
# Returns: Error: No container system detected

# If command fails
mm shell hvac-dev "false"
# Returns: exit code 1
```

## Container/VM Commands Reference

### ⚠️ Note: Use mm shell instead!

**For running commands in containers, always use `mm shell` instead of the commands below.**

Direct commands are provided here for reference only, but `mm shell` is:
- Simpler (auto-detects OS)
- Safer (correct user account)
- More reliable (proper escaping)

### On macOS (OrbStack)
- `orb list` - List all VMs
- `orb start <name>` - Start a VM
- `orb stop <name>` - Stop a VM
- `orb restart <name>` - Restart a VM
- `orb delete <name>` - Delete a VM
- `orb clone <source> <dest>` - Clone a VM
- `orb shell <name>` - SSH into a VM
  - **Better alternative:** `mm shell <name>` (for non-interactive commands)
- `orb run <name> <command>` - Run command in VM
  - **Better alternative:** `mm shell <name> "<command>"`
- `orb info <name>` - Show VM info

### On Linux (LXD)
- `lxc list` - List all containers
- `lxc start <name>` - Start a container
- `lxc stop <name>` - Stop a container
- `lxc restart <name>` - Restart a container
- `lxc delete <name> --force` - Delete a container
- `lxc copy <source> <dest>` - Clone a container
- `lxc exec <name> -- /bin/bash` - Open shell in container
  - **Better alternative:** `mm shell <name>` (for interactive)
- `lxc exec <name> -- <command>` - Run command in container
  - **Better alternative:** `mm shell <name> "<command>"`
- `lxc info <name>` - Show container info

### Cross-Platform Scripts

The scripts in this skill automatically detect your OS and container system, using the appropriate commands:

- `cleanup-dev.sh <name>` - Works on both LXD and OrbStack
- `clone-vm.sh <source> <dest>` - Works on both LXD and OrbStack
- `batch-execute.sh <command> [--all|<container1> <container2>...]` - Works on both
- `list-resources.sh` - Works on both LXD and OrbStack
- `machine-view-manager.sh` - Works on both (manages machine-level tmux sessions)

## Tips

1. **Before Cloning Large VMs**: Always check disk usage first with `list-resources.sh`, then clean up with `cleanup-dev.sh --cleanup`
2. **Batch Operations**: Use `batch-execute.sh` to save time when updating multiple VMs
3. **Dry Run**: Use `--dry-run` with cleanup to see what would be removed first
4. **Regular Maintenance**: Run `list-resources.sh` periodically to monitor disk usage
