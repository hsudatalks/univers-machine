#!/bin/bash
# Tmux Web Share - 通过浏览器分享 tmux 会话
# 使用 ttyd 提供 web 访问

set -e

# 颜色定义
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

# 获取脚本目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$SCRIPT_DIR/lib/container-helper.sh"

# 默认配置
DEFAULT_PORT=7681
DEFAULT_MODE="readonly"

# 显示帮助
show_help() {
    cat << EOF
Tmux Web Share - 通过浏览器分享 tmux 会话

用法:
  $0 <container> <session> [options]

参数:
  container         容器名称 (如: compute-dev)
  session           tmux 会话名称 (如: container-mobile-view)

选项:
  -p, --port PORT       监听端口 (默认: $DEFAULT_PORT)
  -w, --writable        允许写入 (默认只读)
  -u, --user USER       认证用户名
  -P, --password PASS   认证密码
  -t, --title TITLE     浏览器标题
  --public              允许外部访问 (默认只监听 localhost)
  -h, --help            显示此帮助

示例:
  # 只读分享 compute-dev 的 mobile view (仅本地访问)
  $0 compute-dev container-mobile-view

  # 可写模式，带认证 (仅本地访问)
  $0 compute-dev container-mobile-view -w -u admin -P secret123

  # 允许外部访问 (通过 Tailscale)
  $0 compute-dev container-mobile-view --public -u viewer -P pass123

  # 自定义端口和标题
  $0 compute-dev container-desktop-view -p 8080 -t "Dev Desktop"

安全建议:
  1. 默认只监听 localhost，通过 SSH 端口转发访问最安全
  2. 如果使用 --public，务必设置用户名和密码
  3. 考虑通过 Tailscale VPN 访问，而不是暴露到公网

访问方式:
  # 本地访问
  http://localhost:7681

  # 通过 SSH 端口转发（推荐）
  ssh -L 7681:localhost:7681 david@100.90.151.62
  然后在本地浏览器访问: http://localhost:7681

  # 通过 Tailscale（如果使用 --public）
  http://100.90.151.62:7681

EOF
}

# 检查 ttyd 是否安装
check_ttyd() {
    if ! command -v ttyd &> /dev/null; then
        print_error "ttyd 未安装"
        echo
        print_info "正在安装 ttyd..."

        # 检测系统架构
        ARCH=$(uname -m)
        case $ARCH in
            x86_64)
                TTYD_ARCH="x86_64"
                ;;
            aarch64|arm64)
                TTYD_ARCH="aarch64"
                ;;
            *)
                print_error "不支持的架构: $ARCH"
                exit 1
                ;;
        esac

        # 下载最新版本
        TTYD_VERSION="1.7.7"
        TTYD_URL="https://github.com/tsl0922/ttyd/releases/download/${TTYD_VERSION}/ttyd.${TTYD_ARCH}"

        print_info "下载 ttyd ${TTYD_VERSION} for ${TTYD_ARCH}..."
        sudo curl -L -o /usr/local/bin/ttyd "$TTYD_URL"
        sudo chmod +x /usr/local/bin/ttyd

        print_success "ttyd 安装完成"
    else
        print_success "ttyd 已安装: $(ttyd --version)"
    fi
}

# 解析参数
parse_args() {
    if [ $# -lt 2 ]; then
        show_help
        exit 1
    fi

    CONTAINER="$1"
    SESSION="$2"
    shift 2

    PORT=$DEFAULT_PORT
    MODE=$DEFAULT_MODE
    USERNAME=""
    PASSWORD=""
    TITLE="$CONTAINER - $SESSION"
    BIND_ADDR="127.0.0.1"

    while [ $# -gt 0 ]; do
        case "$1" in
            -p|--port)
                PORT="$2"
                shift 2
                ;;
            -w|--writable)
                MODE="writable"
                shift
                ;;
            -u|--user)
                USERNAME="$2"
                shift 2
                ;;
            -P|--password)
                PASSWORD="$2"
                shift 2
                ;;
            -t|--title)
                TITLE="$2"
                shift 2
                ;;
            --public)
                BIND_ADDR="0.0.0.0"
                shift
                ;;
            -h|--help)
                show_help
                exit 0
                ;;
            *)
                print_error "未知选项: $1"
                show_help
                exit 1
                ;;
        esac
    done
}

# 启动 web 分享
start_share() {
    print_header "启动 Tmux Web 分享"
    echo

    # 检查容器是否存在
    if ! container_exists "$CONTAINER"; then
        print_error "容器 $CONTAINER 不存在"
        exit 1
    fi

    # 检查容器是否运行
    if ! container_is_running "$CONTAINER"; then
        print_error "容器 $CONTAINER 未运行"
        exit 1
    fi

    print_info "容器: $CONTAINER"
    print_info "会话: $SESSION"
    print_info "端口: $PORT"
    print_info "模式: $MODE"
    print_info "监听: $BIND_ADDR"

    # 构建 ttyd 命令
    TTYD_OPTS="-p $PORT -i $BIND_ADDR -t titleFixed=\"$TITLE\""

    # 只读模式 (ttyd 默认就是只读，只有可写模式才需要加 -W 参数)
    if [ "$MODE" = "readonly" ]; then
        print_info "只读模式: 观看者无法输入"
    else
        TTYD_OPTS="$TTYD_OPTS -W"
        print_warning "可写模式: 观看者可以操作会话"
    fi

    # 认证
    if [ -n "$USERNAME" ] && [ -n "$PASSWORD" ]; then
        TTYD_OPTS="$TTYD_OPTS -c $USERNAME:$PASSWORD"
        print_info "认证: 需要用户名和密码"
    else
        if [ "$BIND_ADDR" = "0.0.0.0" ]; then
            print_warning "未设置认证，任何人都可以访问！"
            print_warning "建议使用 -u 和 -P 选项设置认证"
        fi
    fi

    # 构建连接命令
    CONTAINER_SYSTEM="$(detect_container_system)"
    case "$CONTAINER_SYSTEM" in
        lxd)
            CMD="lxc exec $CONTAINER -- su - ubuntu -c 'tmux attach -t $SESSION'"
            ;;
        orbstack)
            CMD="orb run --machine $CONTAINER bash -l -c 'tmux attach -t $SESSION'"
            ;;
        *)
            print_error "未检测到容器系统"
            exit 1
            ;;
    esac

    echo
    print_success "Web 分享已启动！"
    echo
    print_info "访问地址："

    if [ "$BIND_ADDR" = "127.0.0.1" ]; then
        echo "  本地: http://localhost:$PORT"
        echo
        print_info "通过 SSH 端口转发访问（推荐）："
        echo "  ssh -L $PORT:localhost:$PORT david@100.90.151.62"
        echo "  然后访问: http://localhost:$PORT"
    else
        echo "  本地: http://localhost:$PORT"
        echo "  Tailscale: http://100.90.151.62:$PORT"
        LOCAL_IP=$(ip -4 addr show eth0 | grep -oP '(?<=inet\s)\d+(\.\d+){3}')
        if [ -n "$LOCAL_IP" ]; then
            echo "  局域网: http://$LOCAL_IP:$PORT"
        fi
    fi

    echo
    print_info "按 Ctrl+C 停止分享"
    echo

    # 启动 ttyd
    eval "ttyd $TTYD_OPTS $CMD"
}

# 主函数
main() {
    if [ "$1" = "-h" ] || [ "$1" = "--help" ] || [ $# -eq 0 ]; then
        show_help
        exit 0
    fi

    check_ttyd
    echo
    parse_args "$@"
    start_share
}

main "$@"
