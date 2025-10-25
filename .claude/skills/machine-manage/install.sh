#!/bin/bash
# Machine Management Skill Installation Script
# 机器管理 Skill 安装脚本

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
print_header() { echo -e "${BLUE}📦 $1${NC}"; }

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
ZSH_CONFIG="$SCRIPT_DIR/machine-manage.zsh"
ZSHRC="$HOME/.zshrc"

echo "=== Machine Management Skill 安装程序 ==="
echo
print_info "仓库路径: $REPO_ROOT"
print_info "Zsh 配置: $ZSH_CONFIG"
echo

# 1. Add to ~/.zshrc
print_header "步骤 1/3: 配置 machine-manager 命令到 zsh"

if [ ! -f "$ZSHRC" ]; then
    print_warning "~/.zshrc 不存在，将创建新文件"
    touch "$ZSHRC"
fi

# Check if already sourced
SOURCE_LINE="source $ZSH_CONFIG"
if grep -qF "$SOURCE_LINE" "$ZSHRC"; then
    print_success "machine-manage.zsh 已在 ~/.zshrc 中配置"
else
    print_info "正在添加配置到 ~/.zshrc..."
    echo "" >> "$ZSHRC"
    echo "# Machine Management Skill" >> "$ZSHRC"
    echo "$SOURCE_LINE" >> "$ZSHRC"
    print_success "已添加配置到 ~/.zshrc"
fi

echo

# 2. Create config file if not exists
print_header "步骤 2/3: 初始化配置文件"
CONFIG_DIR="$REPO_ROOT/config"
CONFIG_FILE="$CONFIG_DIR/vms.yaml"
CONFIG_EXAMPLE="$CONFIG_DIR/vms.yaml.example"

if [ -f "$CONFIG_FILE" ]; then
    print_success "配置文件已存在: $CONFIG_FILE"
else
    if [ -f "$CONFIG_EXAMPLE" ]; then
        cp "$CONFIG_EXAMPLE" "$CONFIG_FILE"
        print_success "已从示例创建配置文件: $CONFIG_FILE"
        print_info "请根据实际情况编辑此文件"
    else
        print_warning "配置示例文件不存在，跳过配置文件创建"
    fi
fi

echo

# 3. Verify installation
print_header "步骤 3/3: 验证安装"

# Check if zsh config file exists and is readable
if [ -f "$ZSH_CONFIG" ] && [ -r "$ZSH_CONFIG" ]; then
    print_success "machine-manage.zsh 配置文件存在且可读"
else
    print_error "machine-manage.zsh 配置文件不存在或不可读"
    exit 1
fi

# Check if scripts are executable
SCRIPT_COUNT=$(find "$SCRIPT_DIR/scripts" -name "*.sh" -type f | wc -l)
print_success "发现 $SCRIPT_COUNT 个脚本文件"

# Check if machine-view-manager.sh is executable
if [ -x "$SCRIPT_DIR/scripts/machine-view-manager.sh" ]; then
    print_success "machine-view-manager.sh 可执行"
else
    print_warning "machine-view-manager.sh 不可执行，正在修复..."
    chmod +x "$SCRIPT_DIR/scripts/machine-view-manager.sh"
    print_success "已修复执行权限"
fi

echo
echo "=== 安装完成 ==="
echo
print_success "machine-manage skill 已成功安装！"
echo
print_warning "重要：需要重新加载 shell 配置："
echo "  source ~/.zshrc"
echo
echo "或者关闭并重新打开终端"
echo
print_info "安装后可用命令："
echo "  machine-manager start          # 启动所有 machine view 会话"
echo "  machine-manager stop           # 停止所有会话"
echo "  machine-manager restart        # 重启所有会话"
echo "  machine-manager status         # 查看状态"
echo "  machine-manager attach desktop # 连接桌面视图"
echo "  machine-manager attach mobile  # 连接移动视图"
echo
print_info "配置文件："
echo "  $CONFIG_FILE"
echo
print_info "下一步："
echo "  1. 运行 'source ~/.zshrc' 加载配置"
echo "  2. 编辑配置文件设置你的虚拟机列表"
echo "  3. 运行 'machine-manager start' 启动聚合视图"
echo
