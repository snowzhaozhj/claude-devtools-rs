## 1. 通知 badge 自动刷新

- [x] 1.1 TabBar.svelte 添加 30 秒轮询：`setInterval` 调用 `getNotifications(1, 0)` 更新 `setUnreadCount`，onDestroy 清理
- [x] 1.2 src-tauri/src/lib.rs：`mark_notification_read` 成功后通过 `app.emit("notification-update", ...)` 推送事件（需 `use tauri::Emitter`）
- [x] 1.3 App.svelte：通过 `@tauri-apps/api/event` 的 `listen("notification-update")` 监听事件，刷新 unreadCount，onDestroy 取消监听

## 2. Context Panel 增强

- [x] 2.1 contextExtractor.ts 增强：为 ContextEntry 添加 `categoryKey`/`path` 字段，新增 `CATEGORY_COLORS` 常量（6 色系统）
- [x] 2.2 ContextPanel.svelte 视图模式切换：Header 添加 Category / Ranked 按钮，`viewMode` 状态切换
- [x] 2.3 Ranked 视图：所有 entries 按 estimatedTokens 降序平铺，每项显示分类颜色标签 + label + token 数
- [x] 2.4 创建 `DirectoryTree.svelte`：递归目录树组件（buildTree 构建 + snippet 递归渲染），目录可折叠，文件显示 token 数，排序：文件优先+字母序
- [x] 2.5 ContextPanel Category 视图：CLAUDE.md 类别用 DirectoryTree 替换扁平列表，Mentioned Files 单独分组

## 3. 验证

- [x] 3.1 `npm run check --prefix ui` 类型检查通过（0 错误）
- [x] 3.2 `cargo check`（src-tauri）后端检查通过
