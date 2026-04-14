## Why

会话多了之后找不到目标 session，阅读长对话时也无法定位关键内容。搜索是日常使用的基本需求。

## What Changes

- **Sidebar 会话过滤**：在 Sidebar 顶部增加搜索框，按会话标题实时过滤会话列表（前端过滤，无需后端）
- **Session 内搜索 Cmd+F**：移植原版 SearchBar 组件，支持全文搜索当前 session 的文本内容，匹配项高亮（`<mark>`），Enter/Shift+Enter 上下导航

## Capabilities

### New Capabilities

（无——搜索逻辑全部在前端完成，不涉及数据层 capability）

### Modified Capabilities

（无）

## Impact

- `ui/src/components/Sidebar.svelte`：新增搜索输入框 + 过滤逻辑
- `ui/src/components/SearchBar.svelte`：新建，Session 内搜索栏组件
- `ui/src/routes/SessionDetail.svelte`：集成搜索栏、高亮匹配文本
- `ui/src/lib/searchHighlight.ts`：新建，文本匹配 + `<mark>` 高亮工具函数
- 无后端改动、无 Tauri IPC 变更
