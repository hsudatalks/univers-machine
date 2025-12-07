#!/bin/bash
# Fix tmux-agents.sh script for container environment

TMUX_AGENTS_SH="/home/ubuntu/repos/univers-container/.claude/skills/univers-core/ops/tmux-agents.sh"

echo "Fixing tmux-agents.sh script..."

# Backup original
sudo cp "$TMUX_AGENTS_SH" "${TMUX_AGENTS_SH}.backup"

# Replace hardcoded /home/davidxu/ with /home/ubuntu/
sudo sed -i 's|/home/davidxu/|/home/ubuntu/|g' "$TMUX_AGENTS_SH"

# Fix the UNIVERS_CORE path specifically
sudo sed -i 's|UNIVERS_CORE="/home/davidxu/repos/univers-container/.claude/skills/univers-core/lib"|UNIVERS_CORE="/home/ubuntu/repos/univers-container/.claude/skills/univers-core/lib"|g' "$TMUX_AGENTS_SH"

echo "Fixed tmux-agents.sh script. Changes made:"
echo "- Changed /home/davidxu/ to /home/ubuntu/"
echo "- Updated UNIVERS_CORE path"

# Show the fixed lines
echo ""
echo "Fixed UNIVERS_CORE line:"
sudo grep -n "UNIVERS_CORE=" "$TMUX_AGENTS_SH"