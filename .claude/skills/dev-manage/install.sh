#!/bin/bash
# Dev Management Skill Installation Script
# 开发环境管理 Skill 安装脚本

set -e

# Color codes
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

print_success() { echo -e "${GREEN}✅ $1${NC}"; }
print_info() { echo -e "${BLUE}ℹ️  $1${NC}"; }
print_warning() { echo -e "${YELLOW}⚠️  $1${NC}"; }
print_error() { echo -e "${RED}❌ $1${NC}"; }
print_header() { echo -e "${BLUE}\n📦 $1${NC}\n"; }

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
ZSH_CONFIG="$SCRIPT_DIR/dm.zsh"
ZSHRC="$HOME/.zshrc"

echo "=== Dev Management Skill 安装程序 ==="
echo
print_info "仓库路径: $REPO_ROOT"
print_info "Zsh 配置: $ZSH_CONFIG"
echo

# 1. Add to ~/.zshrc
print_header "步骤 1/3: 配置 dev-manager 命令到 zsh"

if [ ! -f "$ZSHRC" ]; then
    print_warning "~/.zshrc 不存在，将创建新文件"
    touch "$ZSHRC"
fi

# Check if already sourced
SOURCE_LINE="source $ZSH_CONFIG"
if grep -qF "$SOURCE_LINE" "$ZSHRC"; then
    print_success "dm.zsh 已在 ~/.zshrc 中配置"
else
    print_info "正在添加配置到 ~/.zshrc..."
    echo "" >> "$ZSHRC"
    echo "# Dev Management Skill" >> "$ZSHRC"
    echo "$SOURCE_LINE" >> "$ZSHRC"
    print_success "已添加配置到 ~/.zshrc"
fi

echo

# 2. Create config file if not exists
print_header "步骤 2/3: 初始化配置文件"
CONFIG_DIR="$REPO_ROOT/config"
CONFIG_FILE_1="$CONFIG_DIR/servers.yaml"
CONFIG_FILE_2="$CONFIG_DIR/servers.yaml.example"
EXAMPLE_FILE="$SCRIPT_DIR/configs/servers.yaml"

# Create config directory if it doesn't exist
if [ ! -d "$CONFIG_DIR" ]; then
    mkdir -p "$CONFIG_DIR"
    print_success "创建配置目录: $CONFIG_DIR"
fi

# Check for existing config files
if [ -f "$CONFIG_FILE_1" ]; then
    print_success "配置文件已存在: $CONFIG_FILE_1"
elif [ -f "$CONFIG_FILE_2" ]; then
    print_success "配置文件已存在: $CONFIG_FILE_2"
else
    # Copy example file to config
    if [ -f "$EXAMPLE_FILE" ]; then
        cp "$EXAMPLE_FILE" "$CONFIG_FILE_1"
        print_success "已从示例创建配置文件: $CONFIG_FILE_1"
        print_info "请根据实际情况编辑此文件"
        print_info "配置说明："
        echo "  - sessions: 定义开发会话（如 ark-dev, infra-dev）"
        echo "  - servers: 每个会话的服务器列表"
        echo "  - local_window: 本地窗口配置（display_name, enabled等）"
    else
        print_warning "配置示例文件不存在，跳过配置文件创建"
    fi
fi

echo

# 3. Verify installation
print_header "步骤 3/3: 验证安装"

# Check if zsh config file exists and is readable
if [ -f "$ZSH_CONFIG" ] && [ -r "$ZSH_CONFIG" ]; then
    print_success "dm.zsh 配置文件存在且可读"
else
    print_error "dm.zsh 配置文件不存在或不可读"
    exit 1
fi

# Check if scripts are executable
SCRIPT_COUNT=$(find "$SCRIPT_DIR/scripts" -name "*.sh" -type f | wc -l)
print_success "发现 $SCRIPT_COUNT 个脚本文件"

# Check if dev-session-manager.sh is executable
if [ -x "$SCRIPT_DIR/scripts/dev-session-manager.sh" ]; then
    print_success "dev-session-manager.sh 可执行"
else
    print_warning "dev-session-manager.sh 不可执行，正在修复..."
    chmod +x "$SCRIPT_DIR/scripts/dev-session-manager.sh"
    print_success "已修复执行权限"
fi

echo
echo "=== 安装完成 ==="
echo
print_warning "重要：需要重新加载 shell 配置："
echo "  source ~/.zshrc"
echo
echo "或者关闭并重新打开终端"
echo
print_info "安装后可用命令（主函数）："
echo "  dev-manager start [session]    # 启动 dev 会话（默认: ark-dev）"
echo "  dev-manager stop [session]     # 停止 dev 会话"
echo "  dev-manager restart [session]  # 重启 dev 会话"
echo "  dev-manager status [session]   # 查看会话状态"
echo "  dev-manager attach [session]   # 连接到会话"
echo "  dev-manager list               # 列出所有会话"
echo
echo "别名（快捷方式）："
echo "  dm start                       # 等同于 dev-manager start"
echo "  dm attach                      # 等同于 dev-manager attach"
echo "  dm restart ark-dev             # 使用别名重启 ark-dev"
echo
echo "在 dev 会话中："
echo "  Ctrl+N    - 下一个窗口"
echo "  Ctrl+P    - 上一个窗口"
echo "  Ctrl+B D  - 分离会话"
echo
print_info "配置文件："
echo "  $CONFIG_DIR/servers.yaml"
echo
print_info "下一步："
echo "  1. 运行 'source ~/.zshrc' 加载配置"
echo "  2. 编辑配置文件设置你的服务器列表"
echo "  3. 运行 'dev-manager start' 或 'dm start' 启动开发会话"
echo
