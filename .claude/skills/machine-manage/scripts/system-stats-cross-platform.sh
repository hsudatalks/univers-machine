#!/bin/bash
# Cross-platform system stats script for tmux status bar
# Returns CPU, memory and swap usage with platform-specific optimizations

# Detect operating system
OS="$(uname -s)"

# Get CPU usage - platform specific
case "$OS" in
    Darwin*)
        # macOS: Use top command
        CPU_USAGE=$(top -l 1 -n 0 | grep "CPU usage" | awk '{print $3}' | sed 's/%//' || echo "0")
        ;;
    Linux*)
        # Linux: Use /proc/stat or top
        CPU_USAGE=$(top -bn1 | grep "Cpu(s)" | sed "s/.*, *\([0-9.]*\)%* id.*/\1/" | awk '{print 100 - $1}' | cut -d"." -f1 || echo "0")
        ;;
    *)
        CPU_USAGE="0"
        ;;
esac

# Get memory usage - platform specific
case "$OS" in
    Darwin*)
        # macOS: Detailed memory analysis
        # Get total memory
        TOTAL_GB=$(system_profiler SPHardwareDataType | grep "Memory:" | awk '{print $2}' || echo "16")
        
        # Get memory breakdown from vm_stat
        PAGES_ACTIVE=$(vm_stat | grep "Pages active:" | awk '{print $3}' | sed 's/\.//' || echo "0")
        PAGES_WIRED=$(vm_stat | grep "Pages wired down:" | awk '{print $4}' | sed 's/\.//' || echo "0")
        PAGES_COMPRESSED=$(vm_stat | grep "Pages occupied by compressor:" | awk '{print $4}' | sed 's/\.//' || echo "0")
        
        PAGE_SIZE=16384
        ACTIVE_GB=$(echo "scale=1; $PAGES_ACTIVE * $PAGE_SIZE / 1024 / 1024 / 1024" | bc 2>/dev/null || echo "0")
        WIRED_GB=$(echo "scale=1; $PAGES_WIRED * $PAGE_SIZE / 1024 / 1024 / 1024" | bc 2>/dev/null || echo "0")
        
        # Real memory usage = Active + Wired (excluding cache and compressed)
        REAL_USED_GB=$(echo "scale=1; $ACTIVE_GB + $WIRED_GB" | bc 2>/dev/null || echo "0")
        USED_GB=$(echo "scale=0; $REAL_USED_GB / 1" | bc 2>/dev/null || echo "0")
        ;;
    Linux*)
        # Linux: Use /proc/meminfo
        MEM_INFO=$(cat /proc/meminfo)
        TOTAL_KB=$(echo "$MEM_INFO" | grep "MemTotal:" | awk '{print $2}' || echo "0")
        AVAILABLE_KB=$(echo "$MEM_INFO" | grep "MemAvailable:" | awk '{print $2}' || echo "0")
        
        TOTAL_GB=$(echo "scale=1; $TOTAL_KB / 1024 / 1024" | bc 2>/dev/null || echo "16")
        AVAILABLE_GB=$(echo "scale=1; $AVAILABLE_KB / 1024 / 1024" | bc 2>/dev/null || echo "8")
        USED_GB=$(echo "scale=0; ($TOTAL_GB - $AVAILABLE_GB) / 1" | bc 2>/dev/null || echo "0")
        ;;
    *)
        TOTAL_GB="16"
        USED_GB="8"
        ;;
esac

# Get swap usage - platform specific
case "$OS" in
    Darwin*)
        # macOS: Use sysctl vm.swapusage
        SWAP_TOTAL_MB=$(sysctl vm.swapusage 2>/dev/null | awk '{print $4}' | sed 's/\.00M,//' || echo "0")
        SWAP_USED_MB=$(sysctl vm.swapusage 2>/dev/null | awk '{print $7}' | sed 's/\.00M,//' || echo "0")
        SWAP_PCT=$(echo "scale=0; $SWAP_USED_MB * 100 / $SWAP_TOTAL_MB" | bc 2>/dev/null || echo "0")
        ;;
    Linux*)
        # Linux: Use /proc/meminfo
        SWAP_INFO=$(cat /proc/meminfo 2>/dev/null | grep "Swap" || echo "SwapTotal: 0 kB" "SwapFree: 0 kB")
        SWAP_TOTAL_KB=$(echo "$SWAP_INFO" | grep "SwapTotal:" | awk '{print $2}' || echo "0")
        SWAP_FREE_KB=$(echo "$SWAP_INFO" | grep "SwapFree:" | awk '{print $2}' || echo "0")
        SWAP_USED_KB=$(echo "scale=0; $SWAP_TOTAL_KB - $SWAP_FREE_KB" | bc 2>/dev/null || echo "0")
        SWAP_PCT=$(echo "scale=0; $SWAP_USED_KB * 100 / $SWAP_TOTAL_KB" | bc 2>/dev/null || echo "0")
        ;;
    *)
        SWAP_PCT="0"
        ;;
esac

# Determine memory status based on real usage vs total
REAL_PRESSURE_PCT=$(echo "scale=0; $USED_GB * 100 / $TOTAL_GB" | bc 2>/dev/null || echo "0")

if [ $REAL_PRESSURE_PCT -gt 80 ]; then
    MEM_STATUS="🔴"
elif [ $REAL_PRESSURE_PCT -gt 60 ]; then
    MEM_STATUS="🟡"  
elif [ $REAL_PRESSURE_PCT -gt 40 ]; then
    MEM_STATUS="🟢"
else
    MEM_STATUS="✅"
fi

# Format output
echo "${CPU_USAGE}% ${USED_GB}/${TOTAL_GB}G ${SWAP_PCT}% ${MEM_STATUS}"