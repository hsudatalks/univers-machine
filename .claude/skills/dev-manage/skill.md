# Dev Manage Skill

独立管理多层架构开发会话的 skill，专注于 `dev` 层的会话管理。

## 架构

四层 tmux 会话架构：

```
Dev层 (tmux -L ark)          ← dm 命令
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

- **dm (dev-manage)**: 管理多个服务器的 dev 会话（本 skill）
- **mm (machine-manage)**: 在单个服务器上管理 machine 层会话
- **cm (container-manage)**: 在容器内管理 container 层会话

## 使用方式

```bash
# 启动会话
dm ark-dev start

# 连接到会话（在真实终端中）
dm ark-dev attach

# 查看状态
dm ark-dev status

# 停止会话
dm ark-dev stop

# 重启会话
dm ark-dev restart
```

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
