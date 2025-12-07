#!/bin/bash
# Enhanced system stats script for tmux status bar
# Returns more accurate memory usage for macOS

# Get CPU usage (macOS)
CPU_USAGE=$(top -l 1 -n 0 | grep "CPU usage" | awk '{print $3}' | sed 's/%//')

# Get total memory from system (more reliable than calculation)
TOTAL_GB=$(system_profiler SPHardwareDataType | grep "Memory:" | awk '{print $2}')

# Get detailed memory breakdown from vm_stat (more accurate)
PAGES_ACTIVE=$(vm_stat | grep "Pages active:" | awk '{print $3}' | sed 's/\.//')
PAGES_INACTIVE=$(vm_stat | grep "Pages inactive:" | awk '{print $3}' | sed 's/\.//')
PAGES_WIRED=$(vm_stat | grep "Pages wired down:" | awk '{print $4}' | sed 's/\.//')
PAGES_COMPRESSED=$(vm_stat | grep "Pages occupied by compressor:" | awk '{print $4}' | sed 's/\.//')

PAGE_SIZE=16384

# Calculate actual memory usage categories
ACTIVE_GB=$(echo "scale=1; $PAGES_ACTIVE * $PAGE_SIZE / 1024 / 1024 / 1024" | bc)
INACTIVE_GB=$(echo "scale=1; $PAGES_INACTIVE * $PAGE_SIZE / 1024 / 1024 / 1024" | bc)
WIRED_GB=$(echo "scale=1; $PAGES_WIRED * $PAGE_SIZE / 1024 / 1024 / 1024" | bc)
COMPRESSED_GB=$(echo "scale=1; $PAGES_COMPRESSED * $PAGE_SIZE / 1024 / 1024 / 1024" | bc)

# Real memory pressure = Active + Wired (excluding cache and compressed)
REAL_USED_GB=$(echo "scale=1; $ACTIVE_GB + $WIRED_GB" | bc)
USED_GB=$(echo "scale=0; $REAL_USED_GB / 1" | bc)

# Calculate free memory
FREE_GB=$(echo "scale=1; $TOTAL_GB - $USED_GB" | bc)

# Calculate memory pressure (excluding cached/inactive memory)
# Real memory pressure = Used / Total
PRESSURE_PCT=$(echo "scale=0; $USED_GB * 100 / $TOTAL_GB" | bc)

# Get Swap usage percentage (macOS)
SWAP_TOTAL_MB=$(sysctl vm.swapusage | awk '{print $4}' | sed 's/\.00M,//')
SWAP_USED_MB=$(sysctl vm.swapusage | awk '{print $7}' | sed 's/\.00M,//')
SWAP_PCT=$(echo "scale=0; $SWAP_USED_MB * 100 / $SWAP_TOTAL_MB" | bc 2>/dev/null || echo "0")

# Determine memory status based on real usage vs total
REAL_PRESSURE_PCT=$(echo "scale=0; $REAL_USED_GB * 100 / $TOTAL_GB" | bc)

if [ $REAL_PRESSURE_PCT -gt 80 ]; then
    MEM_STATUS="🔴"
elif [ $REAL_PRESSURE_PCT -gt 60 ]; then
    MEM_STATUS="🟡"  
elif [ $REAL_PRESSURE_PCT -gt 40 ]; then
    MEM_STATUS="🟢"
else
    MEM_STATUS="✅"
fi

# Format output with memory pressure indicator
echo "${CPU_USAGE}% ${USED_GB}/${TOTAL_GB}G ${SWAP_PCT}% ${MEM_STATUS}"