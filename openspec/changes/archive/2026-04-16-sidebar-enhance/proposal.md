## Why

Sidebar 是用户与会话列表交互的核心入口。当前只支持浏览和过滤，缺少三个高频操作：
1. **宽度调整**：不同项目名/会话标题长度不同，固定 280px 不够灵活
2. **右键菜单**：无法快速执行"在新标签页打开""复制 ID"等操作，只能通过 Sidebar 左键点击
3. **Pin/Hide**：长会话列表中无法置顶关注的会话或隐藏不需要的会话

这三个功能在原版 claude-devtools 中已实现，是 P3 第一批 UI 对齐工作。

## What Changes

- 新增 `sidebarStore.svelte.ts`：管理 sidebar 宽度（200~500px 拖拽范围）、per-project 的 Pin/Hide 状态、showHidden 开关
- 新增 `SessionContextMenu.svelte`：fixed 浮层右键菜单，5 个操作项（新标签页打开、置顶/取消、隐藏/取消、复制 ID、复制恢复命令），viewport 边缘 clamping
- 改造 `Sidebar.svelte`：集成宽度拖拽 resize handle、contextmenu 触发、Pin 分区（PINNED 标签+蓝色图标置顶）、Hide 过滤+眼睛图标切换
- 更新 `sidebar-navigation` spec：新增 Pin/Hide/右键菜单/宽度调整 4 个 Requirement

### 局限

- Pin/Hide 状态为内存级（不跨重启持久化），后续可接后端 config 持久化
- 无多选批量 Pin/Hide（原版有，留给后续迭代）
- 无虚拟滚动（当前会话数量不需要，后续按需加）

## Capabilities

### New Capabilities

（无——纯前端 UI 功能）

### Modified Capabilities

- **sidebar-navigation**：新增 4 个 Requirement（Pin/Hide/右键菜单/宽度调整），共 14 个新 Scenario

## Impact

- **前端文件**：新增 `sidebarStore.svelte.ts`、`SessionContextMenu.svelte`；改造 `Sidebar.svelte`
- **后端**：无改动
- **依赖**：无新增依赖
