# Claude Code 配置指南

## 容器用户账号规范 ⚠️

在管理容器时，**必须使用正确的用户账号**，否则无法访问用户的 tmux 会话、文件等。

### 默认用户

| 平台 | 容器类型 | 默认用户 | 示例 |
|------|--------|--------|------|
| Linux | LXD 容器 | `ubuntu` | `lxc exec hvac-dev -- su ubuntu -c "..."` |
| macOS | OrbStack VM | `davidxu` | `orb run --machine hvac-dev -- bash -c "..."` |

### ❌ 错误做法

```bash
# ❌ 错误：直接用 lxc exec，会以 root 身份运行
lxc exec hvac-dev -- tmux list-sessions

# ❌ 错误：找不到 tmux 会话（root 看不到 ubuntu 用户的会话）
lxc exec hvac-dev -- bash -c "tmux attach -t container-mobile-view"
```

### ✅ 正确做法

```bash
# ✅ 正确：指定 ubuntu 用户运行
lxc exec hvac-dev -- su ubuntu -c "tmux list-sessions"

# ✅ 正确：以 ubuntu 用户身份连接会话
lxc exec hvac-dev -- su ubuntu -c 'tmux attach -t container-mobile-view'

# ✅ 使用 container_exec 助手函数（推荐）
source lib/container-helper.sh
container_exec hvac-dev "tmux list-sessions"
```

## 使用 container_exec 助手函数

所有脚本都应该使用 `lib/container-helper.sh` 中的统一函数：

### 函数列表

```bash
# 在容器执行命令（自动选择正确用户）
container_exec <container> "<command>"

# 例：
container_exec hvac-dev "tmux list-sessions"
container_exec hvac-dev "tmux attach -t container-mobile-view"

# 检测容器系统（LXD 或 OrbStack）
detect_container_system  # 返回 "lxd" 或 "orbstack"

# 检测操作系统（Linux 或 macOS）
detect_os  # 返回 "linux" 或 "macos"
```

## 脚本编写规范

在编写容器管理脚本时：

1. **始终使用 container_exec**
   ```bash
   source "$SCRIPT_DIR/lib/container-helper.sh"
   container_exec "$container" "command"
   ```

2. **不要直接用 lxc exec**
   ```bash
   # ❌ 避免
   lxc exec $container -- command

   # ✅ 推荐
   container_exec "$container" "command"
   ```

3. **处理 tmux 嵌套问题**
   ```bash
   # 在容器内运行交互式 tmux 时，需要清除 TMUX 环境变量
   container_exec "$container" "unset TMUX && tmux attach -t session-name"
   ```

## machine-manage skill 使用

### 容器访问命令

```bash
# 所有容器操作都通过这些命令
mm status           # 查看状态
mm attach desktop   # 连接到桌面视图
mm attach mobile    # 连接到移动视图
mm refresh          # 刷新窗口列表
```

### container-tmux-attach.sh

用于从宿主机连接到容器内的 tmux 会话。该脚本自动处理：
- ✅ 正确的用户账号
- ✅ TMUX 环境变量清除
- ✅ LXD 和 OrbStack 的差异

```bash
# 从宿主机连接到容器的会话
./scripts/container-tmux-attach.sh hvac-dev container-mobile-view
```

## 常见错误及修复

| 错误 | 原因 | 修复 |
|------|------|------|
| `tmux: can't find session` | 以 root 身份运行，root 看不到 ubuntu 用户的会话 | 使用 `su ubuntu -c` 或 `container_exec` |
| `sessions should be nested with care` | 嵌套 tmux 需要 unset TMUX | 使用 `unset TMUX &&` 或 `container-tmux-attach.sh` |
| 连接后立即返回 shell | tmux attach 在非交互式环境中失败 | 使用 `container-tmux-attach.sh` 提供交互式包装器 |

## 参考资源

- 宿主机 skill: `.claude/skills/machine-manage/`
- 容器库: `univers-container` 仓库中的 `container-manage` skill
- 助手函数: `.claude/skills/machine-manage/lib/container-helper.sh`
