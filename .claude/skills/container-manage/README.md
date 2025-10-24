# Container Management Skill

This skill provides container and VM management capabilities for OrbStack.

## What This Skill Does

When you invoke the `container-manage` skill in Claude Code, it gives Claude specialized knowledge and tools to manage your OrbStack VMs efficiently.

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
- `machine-desktop-view` - Aggregates all `univers-desktop-view` sessions from dev VMs
- `machine-mobile-view` - Aggregates all `univers-mobile-view` sessions from dev VMs
- `univers-machine-manage` - A local session for managing the physical machine (added to both views)

Each session has 5 windows:
- Window 0: integration-dev (VM)
- Window 1: web-dev (VM)
- Window 2: control-dev (VM)
- Window 3: validation-dev (VM)
- Window 4: machine-manage (物理机管理 - local session)

**Examples:**
```bash
# Start machine view sessions
./scripts/machine-view-manager.sh start

# Check status
./scripts/machine-view-manager.sh status

# Attach to desktop view
./scripts/machine-view-manager.sh attach desktop

# Attach to mobile view
./scripts/machine-view-manager.sh attach mobile

# Restart all sessions
./scripts/machine-view-manager.sh restart

# Stop all sessions
./scripts/machine-view-manager.sh stop
```

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
cd .claude/skills/container-manage
./scripts/list-resources.sh
```

## OrbStack Commands Reference

The skill also knows these common OrbStack commands:

- `orb list` - List all VMs
- `orb start <name>` - Start a VM
- `orb stop <name>` - Stop a VM
- `orb restart <name>` - Restart a VM
- `orb delete <name>` - Delete a VM
- `orb clone <source> <dest>` - Clone a VM
- `orb shell <name>` - SSH into a VM
- `orb run <name> <command>` - Run command in VM
- `orb info <name>` - Show VM info

## Tips

1. **Before Cloning Large VMs**: Always check disk usage first with `list-resources.sh`, then clean up with `cleanup-dev.sh --cleanup`
2. **Batch Operations**: Use `batch-execute.sh` to save time when updating multiple VMs
3. **Dry Run**: Use `--dry-run` with cleanup to see what would be removed first
4. **Regular Maintenance**: Run `list-resources.sh` periodically to monitor disk usage
