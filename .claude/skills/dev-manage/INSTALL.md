# Dev Manage Skill 安装指南

## 快速安装

将以下行添加到 `~/.zshrc`：

```bash
echo "source $HOME/repos/univers-machine/.claude/skills/dev-manage/dm.zsh" >> ~/.zshrc
source ~/.zshrc
```

## 手动安装

1. 编辑 `~/.zshrc`：
   ```bash
   nano ~/.zshrc
   ```

2. 添加以下内容：
   ```bash
   # Dev Manage Skill
   source $HOME/repos/univers-machine/.claude/skills/dev-manage/dm.zsh
   ```

3. 重新加载配置：
   ```bash
   source ~/.zshrc
   ```

## 验证安装

运行以下命令验证安装：

```bash
# 查看 dm 命令帮助
dm

# 查看 ark-dev 会话状态
dm ark-dev status

# 应该显示类似：
# 📦 Dev Session Status: ark-dev
# ✅ Session is RUNNING
#   1: infra (1 panes) [80x24] @0
#   2: domain (1 panes) [80x24] @1
#   ...
```

## 配置

配置文件位于：
`$HOME/repos/univers-machine/.claude/skills/dev-manage/configs/servers.yaml`

编辑该文件可添加或修改开发服务器配置。

## 使用示例

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

## 故障排除

如果 dm 命令不可用：

1. 检查 ~/.zshrc 是否已加载：
   ```bash
   grep "dev-manage/dm.zsh" ~/.zshrc
   ```

2. 检查文件是否存在：
   ```bash
   ls -la $HOME/repos/univers-machine/.claude/skills/dev-manage/dm.zsh
   ```

3. 手动 source：
   ```bash
   source $HOME/repos/univers-machine/.claude/skills/dev-manage/dm.zsh
   ```

## 四层架构说明

```
Dev层 (dm 命令)
  ↓ SSH + tmux attach
Machine层 (mm 命令在服务器上)
  ↓ unset TMUX
Container层 (cm 命令在容器中)
  ↓ Sessions
Session层
```

- **Dev层**: `dm ark-dev` 管理多个服务器的会话
- **Machine层**: `mm` 在单个服务器上管理 VM/容器会话
- **Container层**: `cm` 在容器内管理 session
- **Session层**: 实际的开发会话
