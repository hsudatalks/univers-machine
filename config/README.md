# Configuration Directory

这个目录存放 univers-machine 的配置文件。

## 配置文件说明

### `view-layout.yaml` (不同步到 git)

定义虚拟机内部的 tmux 视图窗口布局。

**使用方法:**

1. 复制示例文件：
   ```bash
   cp config/view-layout.yaml.example config/view-layout.yaml
   ```

2. 编辑配置文件：
   ```bash
   # 修改窗口布局、添加或删除窗口等
   vim config/view-layout.yaml
   ```

3. 应用配置到虚拟机：
   ```bash
   # TODO: 创建应用脚本
   # ./scripts/apply-view-layout.sh <vm-name>
   ```

### `view-layout.yaml.example` (同步到 git)

配置文件示例，包含默认的窗口布局配置。

**特点:**
- 同步到 git，作为参考
- 包含详细的注释说明
- 定义了标准的 desktop-view 和 mobile-view 布局

## Git 同步策略

| 文件 | 是否同步 | 说明 |
|------|---------|------|
| `*.example` | ✅ 同步 | 配置示例文件 |
| `*.yaml` | ❌ 不同步 | 实际配置文件（个人化） |
| `*.yml` | ❌ 不同步 | 实际配置文件（个人化） |
| `README.md` | ✅ 同步 | 文档 |

`.gitignore` 规则:
```
# 配置文件不同步
config/*.yaml
config/*.yml

# 保留示例文件
!config/*.example
```

## 设计理念

这种设计模式参考了常见的配置文件管理方式：
- `.env.example` vs `.env`
- `config.sample.json` vs `config.json`

**优势:**
1. **个人化配置**: 每个人可以有自己的窗口布局
2. **参考示例**: example 文件提供默认配置参考
3. **避免冲突**: 实际配置不同步，避免 git 冲突
4. **版本控制**: 示例文件的变更可以追踪

## 未来扩展

可以在此目录添加更多配置：
- `machine-layout.yaml` - 物理机层面的窗口布局
- `vm-mapping.yaml` - 虚拟机映射配置
- `shortcuts.yaml` - 快捷键配置
- 等等...
