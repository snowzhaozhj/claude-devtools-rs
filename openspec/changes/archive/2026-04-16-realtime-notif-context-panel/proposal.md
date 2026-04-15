## Why

当前两个功能有明显短板：
1. **通知无实时性**：通知列表只在打开 tab 时加载一次，TabBar badge 不会自动更新，用户必须手动刷新才能看到新通知
2. **Context Panel 功能单一**：只有按类别分组的列表视图，缺少原版的 Ranked（按 token 排序）视图、DirectoryTree（CLAUDE.md 目录树）、分类颜色标签

## What Changes

### 通知实时更新

采用**前端轮询 + Tauri event 双通道**方案：

- **前端轮询**：TabBar 每 30 秒调用 `getNotifications(1, 0)` 更新 unreadCount badge
- **Tauri event**：后端在 `mark_notification_read` 时通过 `app.emit("notification-update", ...)` 推送事件，前端立即响应刷新
- **前端监听**：App.svelte 通过 `@tauri-apps/api/event` 的 `listen()` 监听 `notification-update` 事件

**不含**（后续迭代）：
- cdt-watch FileWatcher 集成（文件变更 → 自动扫描 trigger → 创建通知）需要完整的扫描管道，留给 P4
- 系统通知（macOS Notification Center 推送）

### Context Panel 增强

- **视图模式切换**：新增 Header 的 Category / Ranked 切换按钮
- **Ranked 视图**：所有 injection 按 estimatedTokens 降序平铺显示，每项带分类颜色标签（6 色系统对齐原版）
- **DirectoryTree**：CLAUDE.md 类别区从扁平列表改为递归目录树（buildDirectoryTree → DirectoryTreeNode 组件）
- **分类颜色系统**：claude-md 紫蓝 / file 绿 / tool 黄 / thinking 紫 / team 橙 / user 蓝
- **Token 统计增强**：Header 显示总 token 数和注入项计数

## Capabilities

### Modified Capabilities

- **notification-ui**：新增实时更新相关 Requirements
- **session-display**：新增 Context Panel Ranked/DirectoryTree Requirements

## Impact

- **前端文件**：新增 `DirectoryTree.svelte`；改造 `ContextPanel.svelte`、`TabBar.svelte`、`App.svelte`、`contextExtractor.ts`
- **后端**：`src-tauri/src/lib.rs` 的 `mark_notification_read` 添加 Tauri event emit
- **依赖**：`@tauri-apps/api/event`（已在 `@tauri-apps/api` 包中）
