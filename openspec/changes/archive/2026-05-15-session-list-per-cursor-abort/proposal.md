## Why

`list_sessions` 后台元数据扫描的 abort 维度当前是**单 `projectId`**：同一 project 任意一次新调用进入都会 abort 上一轮未完成扫描。结果是首次加载场景下：page 1 IPC 返回后 Sidebar 立即 `queueMicrotask(() => maybeLoadMoreSessions(true))` 自动补满视口 → 触发 page 2 的 `list_sessions` → **page 1 的扫描被 abort**，page 1 还没扫到的 cache miss session 永远拿不到 `title` / `messageCount` / `gitBranch`，列表卡在 `sessionId.slice(0, 8) + "…"` 占位。

副作用：用户看到"sessionId 没替换为标题"和"首次加载非常慢"两类用户可见症状（PR #80、#82、#70 都尝试过修，都没碰到这个根因；现在的实现严格按 spec scenario "同 projectId 新扫描取消旧扫描" 写，所以是 spec 设计层面的 bug）。

## What Changes

- **后端 `LocalDataApi`**：`active_scans` 注册表的 key 由单 `projectId` 改为 **`(projectId, cursor)` 组合**——同 project 同 cursor 抢占（典型场景：silent 刷新重复触发 page 1 扫描）；不同 cursor 并存（page 1 与 page 2 互不干扰）。`metadata_scan_key(project_id, cursor)` 函数对应升级。
- **不**改 broadcast 通道、不改 emit 协议、不改前端 listener 语义。
- **前端 `Sidebar.svelte`**：`totalSessions` 改用 `result.total` 字段而非 `sessions.length`——已经按当前 spec "If the response type keeps a `total` field for compatibility, callers SHALL treat it as informational"（见 `ipc-data-api/spec.md::List sessions uses project-scoped light pagination`）。silent 路径同步刷新本地存的 `totalSessions`，避免首屏数字翻页时累加。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `ipc-data-api`: 改写 "同 projectId 新扫描取消旧扫描" Scenario，明确同 cursor 抢占 / 不同 cursor 并存的语义；新增 Scenario 覆盖 page 1 + page 2 并存。
- `sidebar-navigation`: 新增 Requirement "会话总数显示口径"，明确 `totalSessions` 取后端 `result.total`，silent / loadMore 路径下 totalSessions 行为。

## Impact

- 代码：`crates/cdt-api/src/ipc/local.rs`（`metadata_scan_key` 签名 + `list_sessions` 临界区 + `scan_metadata_for_page` cleanup 逻辑、`active_scans` 类型不变仍是 `HashMap<String, ScanEntry>`，只是 key 字符串编码变了）；`ui/src/components/Sidebar.svelte`（`totalSessions` 派生 + `loadSessions` / `loadMoreSessions` 维护 `sessionsTotal` $state）。
- 测试：`crates/cdt-api/tests/list_sessions.rs`（新增并存 scenario）；`ui/src/lib/sessionMerge.test.ts` 不动；`ui/tests/e2e/` 加一条 sidebar 翻页 + listener patch 不被 abort 的回归。
- 不动：spec 文件以外的其他主 spec / IPC 协议 / 前端事件 listener。
