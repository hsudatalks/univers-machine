#!/bin/bash
# System stats script for tmux status bar
# Returns CPU, memory and swap usage in minimal format

# Get CPU usage (macOS)
CPU_USAGE=$(top -l 1 -n 0 | grep "CPU usage" | awk '{print $3}' | sed 's/%//')

# Get Memory usage in GB (macOS)
MEM_USAGE=$(top -l 1 -n 0 | grep "PhysMem" | awk '{print $2}' | sed 's/G//')

# Get Swap usage percentage (macOS)
SWAP_TOTAL_MB=$(sysctl vm.swapusage | awk '{print $4}' | sed 's/\.00M,//')
SWAP_USED_MB=$(sysctl vm.swapusage | awk '{print $7}' | sed 's/\.00M,//')
SWAP_PCT=$(echo "scale=0; $SWAP_USED_MB * 100 / $SWAP_TOTAL_MB" | bc 2>/dev/null || echo "0")

# Format output (minimal)
echo "${CPU_USAGE}% ${MEM_USAGE}G ${SWAP_PCT}%"