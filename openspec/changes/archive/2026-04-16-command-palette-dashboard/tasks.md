## 1. DashboardView 组件

- [x] 1.1 创建 `ui/src/routes/DashboardView.svelte`：搜索框 + 项目卡片网格（名称、路径缩写、会话数），本地过滤，点击 → `onSelectProject` 回调
- [x] 1.2 App.svelte 空状态替换：无 active tab 时渲染 DashboardView 替代静态文本

## 2. CommandPalette 组件

- [x] 2.1 创建 `ui/src/components/CommandPalette.svelte`：模态面板（fixed overlay + backdrop），搜索框 autofocus
- [x] 2.2 数据加载：onMount 调用 `listProjects()` + `listSessions(selectedProjectId)`
- [x] 2.3 组合搜索视图：项目区（最多 5 条）+ 会话区（最多 20 条，仅 selectedProjectId 存在时显示），本地过滤 displayName/path/title/sessionId
- [x] 2.4 键盘导航：↑↓ 跨区域移动高亮、Enter 确认（项目→选中+关闭、会话→openTab+关闭）、Esc 关闭
- [x] 2.5 查询变化重置 selectedIndex 为 0

## 3. App 集成

- [x] 3.1 App.svelte 添加 Cmd+K 全局快捷键：toggle `commandPaletteOpen` 状态
- [x] 3.2 渲染 CommandPalette（传入 selectedProjectId、onSelectProject、onClose）

## 4. Delta Spec 同步

- [x] 4.1 同步 ui-search 和 session-display 主 spec

## 5. 验证

- [x] 5.1 `npm run check --prefix ui` 类型检查通过
