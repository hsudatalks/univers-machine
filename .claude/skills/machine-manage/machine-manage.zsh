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
MACHINE_MANAGE_SKILL_DIR="/Users/davidxu/repos/univers-machine/.claude/skills/machine-manage"
MACHINE_MANAGE_SCRIPT="$MACHINE_MANAGE_SKILL_DIR/scripts/machine-view-manager.sh"

# machine-manager command
machine-manager() {
    # Handle shell subcommand specially
    if [[ "$1" == "shell" ]]; then
        if [[ -z "$2" ]]; then
            echo "用法: mm shell <vm-name>"
            echo "示例: mm shell web-dev"
            return 1
        fi

        local vm_name="$2"
        shift 2  # Remove 'shell' and vm_name from arguments

        echo "🔗 连接到虚拟机: $vm_name (用户: davidxu, shell: zsh)"

        # If additional arguments, run them; otherwise start interactive shell
        if [[ $# -gt 0 ]]; then
            orbctl run --machine "$vm_name" --user davidxu zsh -l -c "$*"
        else
            orbctl run --machine "$vm_name" --user davidxu zsh -l
        fi
        return $?
    fi

    # For other commands, delegate to machine-view-manager.sh
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
        'shell:进入虚拟机 shell (用户: davidxu, shell: zsh)'
    )

    _describe 'command' commands
}

compdef _machine_manager machine-manager
compdef _machine_manager mm
