# Machine Management Skill - Zsh Configuration
# 机器管理 Skill - Zsh 配置
#
# 使用方法：
# 1. 将此文件添加到 ~/.zshrc：
#    echo "source /Users/davidxu/repos/univers-machine/.claude/skills/machine-manage/machine-manage.zsh" >> ~/.zshrc
# 2. 重新加载配置：
#    source ~/.zshrc
# 3. 使用命令：
#    machine-manager start

# Get the script directory
MACHINE_MANAGE_SKILL_DIR="${0:A:h}"
MACHINE_MANAGE_SCRIPT="$MACHINE_MANAGE_SKILL_DIR/scripts/machine-view-manager.sh"

# machine-manager command
machine-manager() {
    if [[ ! -x "$MACHINE_MANAGE_SCRIPT" ]]; then
        echo "错误: 找不到 machine-view-manager.sh 或文件不可执行"
        echo "路径: $MACHINE_MANAGE_SCRIPT"
        return 1
    fi

    "$MACHINE_MANAGE_SCRIPT" "$@"
}

# Optional: Add completion for machine-manager
_machine_manager() {
    local -a commands
    commands=(
        'start:启动所有 machine view 会话'
        'stop:停止所有会话'
        'restart:重启所有会话'
        'status:查看状态'
        'attach:连接到会话 (desktop/mobile)'
    )

    _describe 'command' commands
}

compdef _machine_manager machine-manager
