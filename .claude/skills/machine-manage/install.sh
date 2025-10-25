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

echo "=== Machine Management Skill 安装程序 ==="
echo
print_info "仓库路径: $REPO_ROOT"
echo

# 1. Create machine-manager symlink
print_header "步骤 1/3: 创建 machine-manager 快捷命令"
SYMLINK_PATH="/usr/local/bin/machine-manager"
TARGET_SCRIPT="$SCRIPT_DIR/scripts/machine-view-manager.sh"

if [ -L "$SYMLINK_PATH" ]; then
    CURRENT_TARGET=$(readlink "$SYMLINK_PATH")
    if [ "$CURRENT_TARGET" = "$TARGET_SCRIPT" ]; then
        print_success "machine-manager 快捷命令已存在且指向正确"
    else
        print_warning "machine-manager 已存在但指向不同位置"
        print_info "当前指向: $CURRENT_TARGET"
        print_info "将更新为: $TARGET_SCRIPT"
        sudo rm "$SYMLINK_PATH"
        sudo ln -sf "$TARGET_SCRIPT" "$SYMLINK_PATH"
        print_success "machine-manager 快捷命令已更新"
    fi
elif [ -e "$SYMLINK_PATH" ]; then
    print_error "$SYMLINK_PATH 已存在但不是符号链接"
    exit 1
else
    print_info "正在创建 machine-manager 快捷命令..."
    print_info "需要 sudo 权限来写入 /usr/local/bin/"
    sudo ln -sf "$TARGET_SCRIPT" "$SYMLINK_PATH"
    print_success "machine-manager 快捷命令已创建"
fi

# Verify symlink
if [ -x "$SYMLINK_PATH" ]; then
    print_success "快捷命令可执行权限正常"
else
    print_error "快捷命令不可执行"
    exit 1
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

# Check if machine-manager command works
if command -v machine-manager &> /dev/null; then
    print_success "machine-manager 命令可用"
    MANAGER_VERSION=$(machine-manager --version 2>/dev/null || echo "无版本信息")
else
    print_error "machine-manager 命令不可用"
    exit 1
fi

# Check if scripts are executable
SCRIPT_COUNT=$(find "$SCRIPT_DIR/scripts" -name "*.sh" -type f | wc -l)
print_success "发现 $SCRIPT_COUNT 个脚本文件"

echo
echo "=== 安装完成 ==="
echo
print_success "machine-manage skill 已成功安装！"
echo
print_info "可用命令："
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
echo "  1. 编辑配置文件设置你的虚拟机列表"
echo "  2. 运行 'machine-manager start' 启动聚合视图"
echo
