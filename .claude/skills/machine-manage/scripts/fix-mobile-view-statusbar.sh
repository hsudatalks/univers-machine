#!/bin/bash
# 修复tmux-mobile-view.sh状态栏配置问题的补丁

echo "🔧 修复tmux-mobile-view.sh状态栏配置问题..."

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_SCRIPT="$SCRIPT_DIR/scripts/tmux-mobile-view.sh"

# 备份原文件
if [ ! -f "$TARGET_SCRIPT.bak" ]; then
    cp "$TARGET_SCRIPT" "$TARGET_SCRIPT.bak"
    echo "✅ 已备份原脚本"
fi

# 修复状态栏配置逻辑
# 问题1: set-option只在dev窗口执行，应该对session执行
# 问题2: 重复读取配置文件5次，应该只读取2次

# 创建修复后的状态栏配置函数
cat > /tmp/fixed_statusbar_function.txt << 'EOF'
    # ========================================
    # 加载状态栏配置（修复版本）
    # ========================================
    log_info "应用状态栏配置..."

    local statusbar_config="$SCRIPT_DIR/configs/mobile-view-statusbar.conf"
    if [ -f "$statusbar_config" ]; then
        # 先应用session级别的配置（只读取一次配置文件）
        while IFS= read -r line || [ -n "$line" ]; do
            [[ "$line" =~ ^[[:space:]]*# ]] && continue
            [[ -z "$line" ]] && continue

            if [[ "$line" =~ ^set-option ]]; then
                # Session级别的配置对所有窗口生效
                eval "tmux set-option -t $SESSION_NAME ${line#set-option }" 2>/dev/null || true
            fi
        done < "$statusbar_config"
        
        # 然后应用window级别的配置（只读取一次配置文件）
        while IFS= read -r line || [ -n "$line" ]; do
            [[ "$line" =~ ^[[:space:]]*# ]] && continue
            [[ -z "$line" ]] && continue

            if [[ "$line" =~ ^setw ]]; then
                # Window级别的配置对所有窗口应用
                for window in dev ops mgr svc ai qa; do
                    eval "tmux set-window-option -t $SESSION_NAME:$window ${line#setw }" 2>/dev/null || true
                done
            fi
        done < "$statusbar_config"
        
        log_info "已加载状态栏配置"
    else
        log_warning "状态栏配置文件未找到: $statusbar_config"
    fi
EOF

# 使用sed替换原脚本中的状态栏配置部分
sed -i '/# 加载状态栏配置/,/log_info "已加载状态栏配置"/{
    /# 加载状态栏配置/{
        r /tmp/fixed_statusbar_function.txt
        a\
    # 选择 dev 窗口
        tmux select-window -t "$SESSION_NAME:dev"
    }
    d
}' "$TARGET_SCRIPT"

# 清理临时文件
rm -f /tmp/fixed_statusbar_function.txt

echo "✅ 状态栏配置逻辑已修复"
echo ""
echo "修复内容："
echo "1. Session级别配置（set-option）现在正确应用到整个session"
echo "2. Window级别配置（setw）现在应用到所有窗口"
echo "3. 配置文件读取次数从5次减少到2次，提高效率"
echo ""
echo "使用方法："
echo "  ./tmux-mobile-view.sh restart  # 重启会话应用修复"