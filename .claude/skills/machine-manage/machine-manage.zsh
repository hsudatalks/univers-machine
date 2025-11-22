# Machine Management Skill - Zsh Configuration
# 机器管理 Skill - Zsh 配置
#
# 使用方法：
# 1. 将此文件添加到 ~/.zshrc：
#    macOS: echo "source /Users/davidxu/repos/univers-machine/.claude/skills/machine-manage/machine-manage.zsh" >> ~/.zshrc
#    Linux: echo "source /home/david/repos/univers-machine/.claude/skills/machine-manage/machine-manage.zsh" >> ~/.zshrc
# 2. 重新加载配置：
#    source ~/.zshrc
# 3. 使用命令：
#    machine-manager start

# Get the script directory dynamically (works on both macOS and Linux)
# ${(%):-%x} expands to the path of the current script being sourced
MACHINE_MANAGE_SKILL_DIR="${${(%):-%x}:A:h}"
MACHINE_MANAGE_SCRIPT="$MACHINE_MANAGE_SKILL_DIR/scripts/machine-view-manager.sh"

# machine-manager command
# All commands (including shell) are delegated to machine-view-manager.sh
# which automatically detects the OS and uses the correct container system
machine-manager() {
    if [[ ! -x "$MACHINE_MANAGE_SCRIPT" ]]; then
        echo "错误: 找不到 machine-view-manager.sh 或文件不可执行"
        echo "路径: $MACHINE_MANAGE_SCRIPT"
        return 1
    fi

    "$MACHINE_MANAGE_SCRIPT" "$@"
}

# Alias for machine-manager (mm = machine-manage)
alias mm='machine-manager'

# Optional: Add completion for machine-manager
_machine_manager() {
    local -a commands
    commands=(
        'start:启动所有 machine view 会话'
        'stop:停止所有会话'
        'restart:重启所有会话'
        'status:查看状态'
        'attach:连接到会话 (desktop/mobile)'
        'refresh:刷新窗口以匹配当前运行的容器'
        'shell:进入容器/VM shell 或执行命令 (自动检测系统)'
        'share:通过 Web 浏览器分享容器 tmux 会话'
    )

    _describe 'command' commands
}

compdef _machine_manager machine-manager
compdef _machine_manager mm
