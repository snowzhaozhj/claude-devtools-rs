## Why

当前应用缺少两个全局导航入口：
1. **无快速跳转**：只能通过 Sidebar 逐级点击项目→会话，无法跨项目快速定位
2. **空状态无引导**：无 tab 打开时显示"选择一个项目开始"静态文本，浪费屏幕空间。Settings 里的 defaultTab "dashboard" 选项无实际效果

原版 Command Palette（Cmd+K）支持三层搜索（项目/会话/跨项目），Dashboard 以卡片网格展示项目概览。

## What Changes

### Command Palette（Cmd+K 全局搜索面板）

- **触发**：全局 Cmd+K 快捷键，弹出模态面板
- **搜索模式**：组合视图——上半区显示匹配的项目（本地过滤），下半区显示当前项目的会话（本地过滤 title/sessionId）
- **键盘导航**：↑↓ 选择、Enter 确认（项目→选中+加载会话；会话→openTab）、Esc 关闭
- **数据来源**：复用已有 `listProjects()` + `listSessions()` IPC，无需新增后端 API
- **限制**：不含跨项目全文搜索（需 `searchAllProjects` API）、不含 UUID/Fragment 精确匹配（需 `findSessionById` API），留给后续迭代

### Dashboard（项目概览页）

- **位置**：替换当前无 tab 时的空状态，不引入新 tab 类型
- **内容**：项目卡片网格（名称、路径缩写、会话数量），本地搜索过滤
- **交互**：点击卡片 → 在 Sidebar 中选中该项目并加载会话列表
- **响应式**：2 列默认（sidebar 占位后主区域约 800px）

### 不含

- 跨项目全文搜索（Cmd+G 全局模式）
- UUID/Fragment 直接定位
- Skeleton 加载骨架
- NewProjectCard（"添加项目"入口）

## Capabilities

### Modified Capabilities

- **ui-search**：新增 Command Palette Requirements（触发、搜索模式、键盘导航、结果选择）
- **session-display**：新增 Dashboard Requirement（空状态替换为项目网格）

## Impact

- **前端文件**：新增 `CommandPalette.svelte`、`DashboardView.svelte`；改造 `App.svelte`
- **后端**：无改动
- **依赖**：无新增
