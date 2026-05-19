## Why

会话列表页在桌面 IPC 与 server mode HTTP 两条链路上的加载行为完全分叉：IPC 走骨架快返 + 后台 metadata broadcast emit（~5–20 ms 首帧），HTTP 走 `list_sessions_sync` 完整同步扫描（100–500 ms 阻塞 fetch），导致 server mode 翻页明显卡顿；桌面端虽快但骨架字段（`title=null` / `messageCount=0` / `isOngoing=false`）到真值之间的"字段跳现"以及切 project 时的 race window 带来可感知的闪烁；冷启动 `MetadataCache` 全 miss 时全部 session 字段都要等后台扫描 emit 才能填上真值，这个 200–500 ms 的过渡期视觉上**只要表达为"加载完成"而不是"内容跳变"**——靠 shimmer 占位 + `transition: opacity` fade-in 视觉策略覆盖，**不**引入磁盘持久化（临时数据不上磁盘，跨 process / 跨平台 / 版本演进多维度风险都不值）。两条路径的 spec 前提（`ipc-data-api` §"HTTP 保留同步完整返回"理由是"HTTP 无 push 通道"）已被 `add-server-mode` change 引入的 `/api/events` SSE bridge + `session_metadata_update` 推送通道推翻，统一两条链路既能消除卡顿、又能让两端体验一致，并去掉一个长期的协议分叉。

## What Changes

- **MODIFIED** `ipc-data-api`：删除"HTTP 路径保留同步完整返回（不适用骨架化）"的旧 Requirement；后端 `LocalDataApi::list_sessions` 骨架 + `try_lookup_cached_metadata` lookup-fast-path + `broadcast::Sender<SessionMetadataUpdate>` push 的语义对 **IPC 与 HTTP 共用同一实现**（HTTP handler 改走骨架版方法，不再调 `list_sessions_sync`）。
- **MODIFIED** `http-data-api`：`GET /api/projects/{projectId}/sessions` Requirement 改为"返回骨架 `PaginatedResponse<SessionSummary>` + 元数据字段（`title` / `messageCount` / `isOngoing` / `gitBranch`）通过 `/api/events` SSE `session_metadata_update` 事件异步 patch"；浏览器 client SHALL 在首次 `GET /api/projects/{id}/sessions` 之前确保 `EventSource('/api/events')` 已 OPEN，避免 emit 抢在订阅前丢失。
- **MODIFIED** `sidebar-navigation`：
  - 新增 store 级 sessions 缓存（by `projectId`，LRU + 内存级，**不**持久化磁盘），切 project SHALL stale-first 立即展示缓存 + 后台 SWR refresh；
  - 新增 in-flight cancel（AbortController for HTTP / generation token for IPC），快速切 project / 快速翻页 SHALL 取消上一轮未完成请求；
  - `loadMoreSessions` SHALL 加 100 ms trailing debounce，防止快速滚动时多次排队 IPC/fetch；
  - metadata 字段从占位到真值的视觉过渡 SHALL 用 CSS `transition: opacity` 渐显（≤ 150 ms），避免字段瞬变；骨架占位 SHALL 用统一的 placeholder shimmer 样式，让"加载中"语义视觉化。
- **BREAKING**：`LocalDataApi::list_sessions_sync` trait method 保留为可选 fallback（其他非 SSE-aware HTTP 客户端如 `cdt-cli` 仍可用），但 `cdt-api::http::routes::list_sessions` handler 不再调用它；后端实现侧无 BREAKING，HTTP wire contract 由"完整 metadata 一次返"变为"骨架先返 + SSE patch"，对**仅有的 in-tree 浏览器客户端**透明（Sidebar 已通过 `transport.ts` 订阅 SSE）。

## Capabilities

### New Capabilities
- 无（行为契约延伸落在现有 capability，无需新建）

### Modified Capabilities
- `ipc-data-api`：删除 HTTP 同步完整返回豁免；统一骨架 + push 语义为唯一规约
- `http-data-api`：`GET /api/projects/{projectId}/sessions` 改为骨架 + SSE 异步 patch；新增"列表请求前 SHALL 已建立 SSE 订阅"语义
- `sidebar-navigation`：新增 sessions store SWR 缓存、in-flight cancel、loadMore debounce、metadata 视觉渐显四条 Requirement

## Impact

- **代码**：
  - `crates/cdt-api/src/http/routes.rs::list_sessions` 单行改：`list_sessions_sync` → `list_sessions`
  - `ui/src/lib/transport.ts::BrowserTransport`：首次 `invoke('list_sessions', ...)` 前 SHALL await SSE OPEN（新建 `ensureSseReady()` 内部方法）
  - 新增 `ui/src/lib/sessionListStore.svelte.ts`：by-projectId LRU cache + SWR + generation token（**内存级，不持久化磁盘**）
  - `ui/src/components/Sidebar.svelte`：`loadSessions` / `loadMoreSessions` 改走 store；加 AbortController + debounce；metadata 占位条加 shimmer placeholder + fade-in CSS
- **测试**：
  - `crates/cdt-api/tests/`: 新增 `http_list_sessions_skeleton_then_sse.rs` 集成测试覆盖"GET 拿骨架 → SSE 收到 metadata patch"
  - `ui/src/lib/sessionListStore.test.ts`: store unit test（SWR / cancel / debounce）
  - `ui/tests/e2e/`: 新增 Playwright spec 覆盖快速翻页不卡顿 + 切 project 不闪烁 + 冷启 shimmer 占位与 fade-in 过渡顺序正确
- **依赖**：无新增（AbortController / EventSource 均为浏览器原生，generation token / LRU 自实现）
- **性能**：server mode 翻页 100–500 ms → ~10–20 ms（与 IPC 同档）；两条链路 user/real / RSS 预算不变；新增前端 store 内存预算：< 1 MB（默认 LRU capacity 16 个 project × 平均 200 条骨架 × ~120 byte ≈ 380 KB）；**不**引入任何磁盘持久化（临时数据不上磁盘，避免长时间不开 app 后 stale 数据导致显示与磁盘真实状态不一致 + 跨 process/版本演进等多维度风险）
- **协调**：`add-server-mode` change 尚未 archive；本 change SHALL 在它 merge 之后再发 PR，spec delta 基于 `add-server-mode` archive 后的 `http-data-api` / `ipc-data-api` 主 spec 落定态写
