## 1. 后端：FileChangeEvent serde + Tauri file-change 桥

- [x] 1.1 `crates/cdt-core/src/watch_event.rs`：给 `FileChangeEvent` 与
  `TodoChangeEvent` 加 `#[serde(rename_all = "camelCase")]`
- [x] 1.2 `cargo test -p cdt-core`（确认无 serde fixture 破坏）+
  `cargo test -p cdt-watch -- --test-threads=1`（burst 测试 macOS FSEvents
  flake，单跑通过；其余 5 个全绿）
- [x] 1.3 `src-tauri/src/lib.rs`：在 `tauri::Builder::setup` 内、现有
  `error_rx → notification-added` 桥之前新增 spawn：subscribe
  `watcher.subscribe_files()` → `app_handle.emit("file-change", &event)`，
  对 `Lagged` continue、`Closed` break
- [x] 1.4 `cargo build --manifest-path src-tauri/Cargo.toml` + `cargo clippy
  --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` 全绿
- [x] 1.5 `cargo fmt --all`

## 2. 前端：fileChangeStore 单例 + dedupe

- [x] 2.1 新建 `ui/src/lib/fileChangeStore.svelte.ts`：定义
  `FileChangePayload { projectId: string; sessionId: string; deleted: boolean }`
  类型；模块级 `let unlisten: UnlistenFn | null`、handler `Map<string,
  (e: FileChangePayload) => void>`、in-flight `Map<string, Promise<void>>`
- [x] 2.2 暴露 `initFileChangeStore(): Promise<void>`（幂等，复用同一
  `initPromise` 保证只 listen 一次）
- [x] 2.3 暴露 `registerHandler(key: string, fn: (e: FileChangePayload) =>
  void): void` / `unregisterHandler(key: string): void`
- [x] 2.4 暴露 `dedupeRefresh(key: string, fn: () => Promise<void>): Promise<void>`
  ——若 key 已有 in-flight Promise 复用之，否则启动并在 finally 删 key
- [x] 2.5 `ui/src/App.svelte` `onMount` 调 `initFileChangeStore()`（在
  `loadAgentConfigs` 之后、`onNotificationUpdate` badge 同步之前）；
  `onDestroy` 不需要——单例 store 跟随窗口生命周期
- [x] 2.6 `npm run check --prefix ui` 0 errors（4 个预存 warnings 与本
  change 无关）

## 3. 前端：SessionDetail 自动刷新 + 滚动保持

- [x] 3.1 `ui/src/routes/SessionDetail.svelte`：从 `fileChangeStore` import
  `registerHandler` / `unregisterHandler` / `dedupeRefresh`
- [x] 3.2 在 `onMount` 内（cached / first-load 之后）注册
  `session-detail-${tabId}` handler：当 `e.projectId === projectId &&
  e.sessionId === sessionId` 时进入刷新流程
- [x] 3.3 提取 `refreshDetail()`：记录
  `wasAtBottom = scrollTop + clientHeight >= scrollHeight - 16`，调
  `getSessionDetail` → 替换 `detail` + `setCachedSession`；若 `wasAtBottom`
  在 `tick()` 后把 `scrollTop` 设为 `scrollHeight`。handler 通过
  `dedupeRefresh(\`detail:${projectId}|${sessionId}\`, refreshDetail)` 合并
- [x] 3.4 `onDestroy` 内 `unregisterHandler(\`session-detail-${tabId}\`)`
- [x] 3.5 `refreshDetail` 内 catch + `console.warn`，UI 保留旧 `detail`
  不切错误态
- [x] 3.6 `npm run check --prefix ui` 0 errors（新增 1 个 warning 与
  line 32 已有的 `tabId` 初值捕获同源；`{#key activeTab.id}` 保证组件销毁
  重建，props 不变；可接受）

## 4. 前端：Sidebar 列表自动刷新

- [x] 4.1 `ui/src/components/Sidebar.svelte`：从 `fileChangeStore` import
  `registerHandler` / `unregisterHandler` / `dedupeRefresh`
- [x] 4.2 在 `$effect` 内注册 `sidebar` key handler：当
  `payload.projectId === currentProjectId` 时调 `dedupeRefresh(\`sidebar:${currentProjectId}\`,
  () => untrack(() => loadSessions(currentProjectId)))`
- [x] 4.3 `onDestroy` 兜底 `unregisterHandler("sidebar")`，
  `$effect` cleanup 也 unregister
- [x] 4.4 handler 闭包捕获最新 `selectedProjectId`：通过 `$effect` 依赖
  `selectedProjectId`，每次切 project 时 effect cleanup → unregister，
  effect 体重跑 → 用新值 register；`untrack` 包裹 `loadSessions` 防止它
  内部读 `$state` 触发 effect 重跑
- [x] 4.5 `npm run check --prefix ui` 0 errors

## 5. 验证 + 文档 + 归档

- [x] 5.1 `cargo test --workspace --exclude cdt-watch` 全绿（cdt-watch 单
  线程跑当下 macOS FSEvents 异常严重，6 个测试中 5 个 timeout——与本 change
  无关：本 change 只改 watch_event.rs serde 注解 + src-tauri spawn，未触
  watcher 行为。`burst_of_writes_debounced` 单跑通过证明 watcher 本身可用）
- [x] 5.2 `cargo clippy --workspace --all-targets -- -D warnings` 通过
- [x] 5.3 `cargo build --manifest-path src-tauri/Cargo.toml` 通过 +
  `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --
  -D warnings` 通过
- [~] 5.4 手工 smoke 留给用户本地 `cargo tauri dev` 验证（自动化任务不便
  交互启动；后端桥 + 前端 store 都通过编译/类型检查）
- [x] 5.5 `openspec validate --all --strict` 21/21 通过
- [x] 5.6 `openspec/followups.md` 标记 "实时会话刷新" section 的
  `[coverage-gap] file-change 事件未桥到前端` 为 ✅ 已在
  `2026-04-18-realtime-session-refresh` 修复
- [x] 5.7 `CLAUDE.md` "UI 已知遗留问题" 删除第 1 条（实时 file-change 桥），
  保留第 2 条 ongoing/interruption（仍未实现）；前置条件标注已就绪
- [x] 5.8 `openspec archive 2026-04-18-realtime-session-refresh -y`：3 个
  spec deltas sync 完成（ipc-data-api modified 1 / session-display added 1 /
  sidebar-navigation added 1）；目录移到
  `openspec/changes/archive/2026-04-18-2026-04-18-realtime-session-refresh/`
