# Machine Management Skill

This skill provides comprehensive machine-level management for this physical server, including OrbStack VM lifecycle, tmux session aggregation, and resource monitoring.

## Capabilities

You are an expert at managing containers and virtual machines using OrbStack. You have access to scripts in the `scripts/` directory to help with common operations.

## Available Operations

### 1. List Containers/VMs
- Use `orb list` to show all running containers and VMs
- Parse and display information in a user-friendly format
- Show: name, status, OS, size, IP address

### 2. Container Lifecycle
- **Start**: `orb start <name>`
- **Stop**: `orb stop <name>`
- **Restart**: `orb restart <name>`
- **Delete**: `orb delete <name>`

### 3. Clone VMs
- Clone existing VMs with new names
- Clean up temporary files before cloning (optional)
- Use: `orb clone <source> <destination>`

### 4. Cleanup Operations
Run cleanup scripts to:
- Remove build artifacts (target/ directories)
- Clear sccache caches
- Clean apt caches
- Remove old logs

### 5. Resource Monitoring
- Check disk usage: `orb info <name>`
- SSH into containers: `orb shell <name>`
- Execute commands remotely: `orb run <name> <command>`

### 6. Batch Operations
- Execute commands across multiple containers
- Group operations (start/stop multiple VMs)
- Health checks across all containers

### 7. Machine-Level View Aggregation
- Create machine-level tmux sessions that aggregate VM tmux sessions
- `machine-desktop-view` - Aggregates all `univers-desktop-view` from dev VMs
- `machine-mobile-view` - Aggregates all `univers-mobile-view` from dev VMs
- `univers-machine-manage` - Local session for physical machine management
- Each session has 5 windows: integration-dev, web-dev, control-dev, validation-dev, machine-manage
- Window 4 (machine-manage) runs in /Users/davidxu/repos/univers-machine for physical machine management
- Switch between windows using tmux navigation (Ctrl+B then 0-4)

## Scripts Available

- `scripts/cleanup-dev.sh` - Clean development artifacts from a VM
- `scripts/list-resources.sh` - Show resource usage across all VMs
- `scripts/clone-vm.sh` - Helper for cloning VMs with cleanup
- `scripts/batch-execute.sh` - Run commands across multiple VMs
- `scripts/machine-view-manager.sh` - Manage machine-level tmux view sessions

## Usage

When the user asks to manage containers or VMs:

1. Determine what operation they want to perform
2. Use the appropriate orb command or script
3. Provide clear feedback about what was done
4. Suggest related operations if relevant

## Examples

- "List all VMs" → `orb list`
- "Clone ubuntu to web-dev" → Check if cleanup needed, then `orb clone ubuntu web-dev`
- "Clean up ubuntu VM" → SSH in and run cleanup commands
- "Delete debian VM" → `orb stop debian && orb delete debian`
- "Start all stopped VMs" → Parse `orb list` and start each stopped VM
- "Start machine view sessions" → `scripts/machine-view-manager.sh start`
- "Show machine view status" → `scripts/machine-view-manager.sh status`
- "Attach to desktop view" → `scripts/machine-view-manager.sh attach desktop`

## Best Practices

1. Always confirm destructive operations (delete, cleanup)
2. Check VM status before operations
3. Suggest cleanup before cloning large VMs
4. Provide clear status updates
5. Handle errors gracefully

When invoked, help the user manage their OrbStack containers efficiently and safely.
