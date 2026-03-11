# univers-machine

物理机层面的虚拟机管理仓库，用于管理 OrbStack 虚拟机和聚合 tmux 会话。

## 功能特性

### 🐳 机器管理 (Machine Management)
- VM 生命周期管理（清理、克隆、批量操作）
- 资源监控和统计
- 批量命令执行
- 详见 [.claude/skills/machine-manage/](.claude/skills/machine-manage/)

### 🖥️ 机器层面的 Tmux 聚合视图 (Machine-Level Views)
- **machine-desktop-view**: 聚合所有虚拟机的桌面视图
- **machine-mobile-view**: 聚合所有虚拟机的移动视图
- 蓝色主题样式，区别于虚拟机的橙黄色系
- 快速在多个虚拟机间切换

### ⚙️ 配置管理 (Configuration)
- 视图布局配置（YAML 格式）
- 配置示例同步到 git
- 实际配置文件个人化（不同步）
- 详见 [config/](config/)

### Rust Daemon 组件
- **univers-daemon-core**: 共享 agent/session、system info、tmux service、installer 能力
- **univers-machine-daemon**: 面向物理机和虚拟机的 HTTP daemon
- **univers-container-daemon**: 面向容器环境的 HTTP daemon
- **univers-ark-developer**: 本地桌面端，用于管理目标机器、容器和工作台

## 目录结构

``` 
univers-machine/
├── apps/
│   ├── univers-ark-developer/        # 目标机器/容器管理桌面端
│   ├── univers-daemon-core/          # daemon 共享核心库
│   ├── univers-machine-daemon/       # 物理机/虚拟机管理 daemon
│   └── univers-container-daemon/     # 容器管理 daemon
├── .claude/skills/machine-manage/    # 机器管理 skill
│   ├── configs/                      # Tmux 样式配置
│   ├── scripts/                      # 管理脚本
│   └── README.md                     # 详细文档
├── config/                           # 配置文件目录
│   ├── vms.yaml.example              # VM 配置示例（同步）
│   └── README.md                     # 配置说明
└── README.md                         # 本文件
```

## 快速开始

### 0. 安装

首次使用需要运行安装脚本：

```bash
.claude/skills/machine-manage/install.sh
source ~/.zshrc  # 重新加载 shell 配置
```

### 1. 管理虚拟机

```bash
# 启动 machine-level 聚合视图
machine-manager start

# 连接到桌面视图（完整信息显示）
machine-manager attach desktop

# 连接到移动视图（简化显示）
machine-manager attach mobile

# 查看所有虚拟机资源使用情况
.claude/skills/machine-manage/scripts/list-resources.sh

# 清理虚拟机缓存
.claude/skills/machine-manage/scripts/cleanup-dev.sh ubuntu

# 克隆虚拟机
.claude/skills/machine-manage/scripts/clone-vm.sh ubuntu new-vm --cleanup
```

### 2. 配置虚拟机列表

```bash
# 复制配置示例
cp config/vms.yaml.example config/vms.yaml

# 编辑配置（设置哪些 VM 显示在聚合视图中）
vim config/vms.yaml

# 重启 machine views 使配置生效
machine-manager restart
```

## 虚拟机列表

当前管理的开发虚拟机：
- **integration-dev**: 集成测试开发环境
- **web-dev**: Web 开发环境
- **control-dev**: 控制系统开发环境
- **validation-dev**: 验证测试环境

## 相关仓库

- [hvac-workbench](../hvac-workbench): 主要开发仓库
- [hvac-operation](../hvac-operation): 运维工具仓库
- [univers-container](../univers-container): 容器管理仓库

## 贡献

这个仓库使用 Claude Code 管理和维护。
