#!/bin/bash
# Enhanced system stats script for tmux status bar
# Returns more accurate memory usage for macOS

# Get CPU usage (macOS)
CPU_USAGE=$(top -l 1 -n 0 | grep "CPU usage" | awk '{print $3}' | sed 's/%//')

# Get total memory from system (more reliable than calculation)
TOTAL_GB=$(system_profiler SPHardwareDataType | grep "Memory:" | awk '{print $2}')

# Get memory usage from top (more accurate for used memory)
MEM_INFO=$(top -l 1 -n 0 | grep "PhysMem:")
USED_MEM=$(echo "$MEM_INFO" | awk '{print $2}')

# Convert used memory to GB
# Handle different formats like "95G", "4984M", etc.
if [[ "$USED_MEM" == *"G"* ]]; then
    USED_GB=$(echo "$USED_MEM" | sed 's/G//')
elif [[ "$USED_MEM" == *"M"* ]]; then
    USED_MB=$(echo "$USED_MEM" | sed 's/M//')
    USED_GB=$(echo "scale=1; $USED_MB / 1024" | bc)
elif [[ "$USED_MEM" == *"K"* ]]; then
    USED_KB=$(echo "$USED_MEM" | sed 's/K//')
    USED_GB=$(echo "scale=1; $USED_KB / 1024 / 1024" | bc)
else
    USED_GB="0"
fi

# Round to integer for cleaner display
USED_GB=$(echo "scale=0; $USED_GB / 1" | bc)

# Calculate free memory
FREE_GB=$(echo "scale=1; $TOTAL_GB - $USED_GB" | bc)

# Calculate memory pressure (excluding cached/inactive memory)
# Real memory pressure = Used / Total
PRESSURE_PCT=$(echo "scale=0; $USED_GB * 100 / $TOTAL_GB" | bc)

# Get Swap usage percentage (macOS)
SWAP_TOTAL_MB=$(sysctl vm.swapusage | awk '{print $4}' | sed 's/\.00M,//')
SWAP_USED_MB=$(sysctl vm.swapusage | awk '{print $7}' | sed 's/\.00M,//')
SWAP_PCT=$(echo "scale=0; $SWAP_USED_MB * 100 / $SWAP_TOTAL_MB" | bc 2>/dev/null || echo "0")

# Determine memory status
if [ $PRESSURE_PCT -gt 85 ]; then
    MEM_STATUS="🔴"
elif [ $PRESSURE_PCT -gt 70 ]; then
    MEM_STATUS="🟡"
elif [ $PRESSURE_PCT -gt 50 ]; then
    MEM_STATUS="🟢"
else
    MEM_STATUS="✅"
fi

# Format output with memory pressure indicator
echo "${CPU_USAGE}% ${USED_GB}/${TOTAL_GB}G ${SWAP_PCT}% ${MEM_STATUS}"