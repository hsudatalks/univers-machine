# univers-machine

物理机层面的虚拟机管理仓库，用于管理 OrbStack 虚拟机和聚合 tmux 会话。

## 功能特性

### 🐳 容器和虚拟机管理 (Container Management)
- VM 生命周期管理（清理、克隆、批量操作）
- 资源监控和统计
- 批量命令执行
- 详见 [.claude/skills/container-manage/](.claude/skills/container-manage/)

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

## 目录结构

```
univers-machine/
├── .claude/skills/container-manage/  # 容器管理 skill
│   ├── configs/                      # Tmux 样式配置
│   ├── scripts/                      # 管理脚本
│   └── README.md                     # 详细文档
├── config/                           # 配置文件目录
│   ├── view-layout.yaml.example      # 视图布局示例（同步）
│   └── README.md                     # 配置说明
└── README.md                         # 本文件
```

## 快速开始

### 1. 管理虚拟机

```bash
# 启动 machine-level 聚合视图
.claude/skills/container-manage/scripts/machine-view-manager.sh start

# 连接到桌面视图（完整信息显示）
.claude/skills/container-manage/scripts/machine-view-manager.sh attach desktop

# 连接到移动视图（简化显示）
.claude/skills/container-manage/scripts/machine-view-manager.sh attach mobile

# 查看所有虚拟机资源使用情况
.claude/skills/container-manage/scripts/list-resources.sh

# 清理虚拟机缓存
.claude/skills/container-manage/scripts/cleanup-dev.sh ubuntu

# 克隆虚拟机
.claude/skills/container-manage/scripts/clone-vm.sh ubuntu new-vm --cleanup
```

### 2. 配置视图布局

```bash
# 复制配置示例
cp config/view-layout.yaml.example config/view-layout.yaml

# 编辑配置
vim config/view-layout.yaml

# 应用配置（TODO: 待实现）
# ./scripts/apply-view-layout.sh <vm-name>
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
