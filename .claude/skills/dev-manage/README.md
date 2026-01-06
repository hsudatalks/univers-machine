# Dev Manage Skill

独立管理多层架构开发会话的 skill，提供 `dev-manager` 命令和 `dm` 别名。

## 架构

四层 tmux 会话架构：

```
Dev层 (tmux -L ark)          ← dev-manager ark-dev / dm ark-dev
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

## 安装

### 自动安装（推荐）

```bash
cd /Users/davidxu/repos/univers-machine/.claude/skills/dev-manage
./install.sh
source ~/.zshrc
```

### 手动安装

编辑 ~/.zshrc 并添加：

```bash
echo "source $HOME/repos/univers-machine/.claude/skills/dev-manage/dm.zsh" >> ~/.zshrc
source ~/.zshrc
```

## 用法

### 使用别名 `dm`（推荐）

```bash
# 启动默认会话（ark-dev）
dm start

# 启动特定会话
dm start infra-dev

# 连接到会话
dm attach

# 查看状态
dm status

# 重启会话
dm restart ark-dev

# 列出所有会话
dm list
```

### 使用完整命令名 `dev-manager`

```bash
# 启动默认会话
dev-manager start

# 启动特定会话
dev-manager start infra-dev

# 连接到会话
dev-manager attach

# 查看状态
dev-manager status ark-dev
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

### 会话列表

- `ark-dev` - 多服务器开发环境
- `infra-dev` - 基础设施开发
- `domain-dev` - 领域服务开发

## 特点

1. **多层嵌套**: Dev → Machine → Container → Session
2. **独立实例**: 每个会话使用独立的 tmux 服务器
   - ark-dev → tmux -L ark
   - infra-dev → tmux -L infra
3. **自动SSH**: 自动连接各服务器并附加到 machine-mobile-view
4. **自动重连**: SSH 断开时自动重试
5. **独立快捷键**: 每个会话的快捷键独立管理

## 配置

配置文件: `.claude/skills/dev-manage/configs/servers.yaml`

编辑该文件可添加/修改服务器配置。

## 快捷键

在 dev 会话中：
- Ctrl+N - 下一个窗口
- Ctrl+P - 上一个窗口
- Ctrl+B D - 分离会话

在每个窗口中（连接的服务器）：
- Ctrl+N/P - 切换 Machine 层窗口
- Ctrl+B D - 分离到 Dev 层
