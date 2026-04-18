## Why

`cdt-watch::FileWatcher` 已经在监听 `~/.claude/projects/`，并且自动通知管线
（`2026-04-17-auto-notification-pipeline`）已经把 `FileChangeEvent` 用于错误检
测。但 **桌面 UI 自身**还没有订阅同一份事件流——前端打开会话后，新的 user/AI
消息只会写进磁盘，UI 不会刷新；sidebar 的 session 列表也不会随新建会话刷新。
原版 Electron app（`../claude-devtools/src/main/index.ts:127-135` +
`src/renderer/store/index.ts:230-275`）行为是：

- main 进程把 `file-change` 事件 forward 给 renderer
- renderer store 命中当前打开的 session → 重拉 detail；命中当前 project → 刷
  新 sidebar 列表
- `sessionDetailSlice` 做 in-flight dedupe，避免一连串 file change 触发并发
  refetch

这是 `CLAUDE.md` "UI 已知遗留问题" 第 1 条，也是后续 ongoing/interruption
（"绿点变白"）的前置依赖。

## What Changes

- `src-tauri/src/lib.rs` 在 `tauri::Builder::setup` 内新增第三个后台 task：订
  阅 `watcher.subscribe_files()`，把 `FileChangeEvent` `emit("file-change",
  &event)` 推到前端（payload 用现有 `FileChangeEvent` 的 camelCase 序列化，与
  其它 IPC payload 一致）
- 新建 `ui/src/lib/fileChangeStore.svelte.ts`：模块级单例 listen 一次
  `file-change`，对外暴露 `registerHandler(key, fn)` / `unregisterHandler(key)`
  注册回调。内部对每个 `(projectId, sessionId)` 维护 in-flight `Promise`，**同
  一会话的并发刷新合并为一次**（沿用原版 `sessionDetailSlice` 的 dedupe 思路）
- `ui/src/routes/SessionDetail.svelte`：`onMount` 注册 handler，命中当前
  `(projectId, sessionId)` 时重拉 `getSessionDetail` → 替换 `tabStore` 缓存；
  刷新前判断 `conversationEl.scrollTop + clientHeight >= scrollHeight - 16`，
  若是 pinned-to-bottom 则刷新后 `tick()` 内自动滚到底部
- `ui/src/components/Sidebar.svelte`：`onMount` 注册 handler，命中当前
  `selectedProjectId` 时（无论 sessionId 是否在现有列表中）重拉 `listSessions`
- 删除 `CLAUDE.md` "UI 已知遗留问题" 第 1 条；`openspec/followups.md` 实时会
  话刷新 section 标记本 change slug 为已修复

## Capabilities

### Modified Capabilities
- `ipc-data-api`: "Emit push events for file changes and notifications"
  Requirement 增加 Tauri `file-change` event 的具体 Scenario（payload 形状 +
  Tauri 转发契约）
- `session-display`: 新增 "Auto refresh on file change" Requirement，覆盖打开
  会话时的 detail 自动刷新 + pinned-to-bottom 滚动 + in-flight dedupe
- `sidebar-navigation`: 新增 "Auto refresh session list on file change"
  Requirement，覆盖当前 project 命中时的 list 重拉

## Impact

- 代码：`src-tauri/src/lib.rs`（setup spawn 第三个 task）、`ui/src/lib/
  fileChangeStore.svelte.ts`（新文件）、`ui/src/routes/SessionDetail.svelte`
  （注册 handler + pinned-to-bottom 检测）、`ui/src/components/Sidebar.svelte`
  （注册 handler）
- 依赖：无新增；`FileChangeEvent` 已派生 `serde::Serialize` 可直接 emit
- 测试：本 change 是 UI/Tauri runtime 集成，**不引入新的 Rust 单测/集成测试**——
  既有 `cdt-watch::FileWatcher` 的多订阅者测试已经覆盖"加 IPC 订阅者不影响
  notifier 订阅者"的语义；前端走 `npm run check --prefix ui` 类型校验
- 数据迁移：无
- 向后兼容：现有 `notification-update` / `notification-added` 事件不动；新
  `file-change` 事件是纯增量
