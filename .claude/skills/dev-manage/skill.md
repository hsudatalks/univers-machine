# Dev Manage Skill

独立管理多层架构开发会话的 skill，专注于 `dev` 层的会话管理。

## 架构

四层 tmux 会话架构：

```
Dev层 (tmux -L ark)          ← dev-manager / dm 命令
  ├─ window: infra   → SSH → infra-dev
  ├─ window: domain  → SSH → domain-dev
  ├─ window: sys     → SSH → mechanism-dev
  ├─ window: ppe     → SSH → ppe-dev
  ├─ window: qa      → SSH → qa-dev
  └─ window: local   → Local machine

Machine层 (tmux -L machine)  ← On each server: machine-mobile-view

Container层 (tmux -L container) ← container-mobile-view

Session层
```

## 与其他工具的关系

- **dev-manager / dm**: 管理多个服务器的 dev 会话（本 skill）
- **machine-manager / mm**: 在单个服务器上管理 machine 层会话
- **container-manager / cm**: 在容器内管理 container 层会话

## 安装

运行安装脚本：

```bash
cd /Users/davidxu/repos/univers-machine/.claude/skills/dev-manage
./install.sh
```

然后重新加载 shell 配置：

```bash
source ~/.zshrc
```

## 使用方式

```bash
# 使用别名（推荐）
dm start                    # 启动默认会话（ark-dev）
dm attach                   # 连接到默认会话
dm status                   # 查看默认会话状态
dm restart ark-dev          # 重启 ark-dev 会话

# 使用完整命令名
dev-manager start           # 启动默认会话
dev-manager start infra-dev # 启动 infra-dev 会话
dev-manager status ark-dev  # 查看 ark-dev 状态
```

### 可用命令

- `start [session]` - 启动 dev 会话
- `stop [session]` - 停止 dev 会话
- `status [session]` - 查看状态
- `attach [session]` - 连接到会话
- `restart [session]` - 重启会话
- `list` - 列出所有可用会话
- `update [session]` - 更新远程服务器仓库
- `mm-start [session] [view]` - 在服务器上启动 machine view
- `mm-manage [session] [action]` - 管理 machine-manage 服务
- `cm-setup [session]` - 在服务器上设置 container-manage

## 特点

1. **独立实例**: 每个会话使用独立的 tmux 服务器
   - ark-dev → tmux -L ark
   - infra-dev → tmux -L infra
2. **多层嵌套**: Dev → Machine → Container → Session
3. **配置驱动**: 所有服务器配置在 `configs/servers.yaml`
4. **自动重连**: SSH 断开后自动重试
5. **独立快捷键**: 每个会话的快捷键独立管理

## 文件结构

```
.claude/skills/dev-manage/
├── dm.zsh                          # 主命令定义
├── README.md                       # 文档
├── INSTALL.md                      # 安装指南
├── skill.md                        # 本文件
├── configs/
│   └── servers.yaml               # 服务器配置
└── scripts/
    └── dev-session-manager.sh     # 核心脚本
```
