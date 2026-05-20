## Why

PR #177（unify-session-list-loading-strategy）让 `list_sessions` 走"骨架 + push patch"模式：IPC 立即返回 `title=null / messageCount=0 / isOngoing=false / gitBranch=null` 的骨架列表，后台 `JoinSet + Semaphore(8)` 并发解析每条 jsonl 的 metadata 后通过 `broadcast → session-metadata-update` push 给前端 in-place patch。

代价是用户**首屏看到列表的两阶段渲染**：先显示 sessionId UUID 占位 → patch 到达后突变为真实 title（中文/英文）。即使加上 `.metadata-pending` shimmer + opacity 半透明，文字内容从 UUID 突变到标题的视觉跳变仍然刺眼，切项目和首次打开都明显。

修法把"等首页 N 条 metadata 解析完"放到 IPC return 之前——sidebar 可见的 20 条直接是真值无两阶段；翻到第二页才有骨架+push（不在视野内）。代价是首屏多 ~150ms 等待（538 session corpus 估算），换"无文字跳变"的视觉收益。

## What Changes

- **MODIFIED** `LocalDataApi::list_sessions`（`crates/cdt-api/src/ipc/local.rs`）：当 `pagination.cursor == None` 时（首页），调 `list_sessions_skeleton` 拿骨架后对前 `EAGER_FIRST_PAGE_LIMIT = 20` 条**同步** `join_all` 等 metadata（每条 `tokio::time::timeout(500ms)`）；剩余 `page_size - 20` 条仍走骨架 + spawn + broadcast。所有 eager 同步等到的 items 直接填回真值，**不**对它们 emit broadcast；超时 / 解析失败的条目走 deferred 单条 retry。`cursor != None` 时（翻页）保持原"骨架 + 后台 spawn + broadcast push"路径不变。
- **MODIFIED** trait `DataApi::list_sessions` doc comment：显式刻入 cursor 分叉契约（cursor=None 含真值；cursor=Some 可骨架），约束未来 implementer。
- **MODIFIED** `extract_session_metadata_cached`：解析失败（字段全占位）SHALL NOT 写入 `MetadataCache`，避免占位真值卡住后续 lookup。
- **MODIFIED** HTTP `GET /api/projects/{id}/sessions` handler（`crates/cdt-api/src/http/routes.rs`）：复用同一个 `DataApi::list_sessions`，由 trait 实现内部 `cursor` 判断 → IPC / HTTP 行为对称。
- **MODIFIED** 浏览器 transport `BrowserTransport.invokeHttp`：`list_sessions(cursor=null)` 跳过 `ensureSseReady()` 1000ms gate（首页路径不依赖 SSE）；`list_sessions(cursor=Some)` / `list_repository_groups` / `list_worktree_sessions` 仍 await SSE-ready。
- **MODIFIED** 前端 `sessionMerge.ts::applySilentRefresh` / `mergeSilentMetadata`：silent refresh 路径走 eager 同步等真值后改用 `mergeRecoveryResponse` 语义（response 含真值则 response 优先；response 是占位才保留旧值）。沿用 PR #177 codex 二审 round 5 引入的 `mergeRecoveryResponse` 实现。
- **MODIFIED** 切 project 时 `LocalDataApi::list_sessions(currentProjectId, cursor=None)` 进入 eager 路径前 SHALL 遍历 `active_scans`，按 `key.split('|').next()` 解析每个 entry 的 projectId，abort 所有 `projectId != currentProjectId` 的旧扫描 entry（不限于单一旧 project），让出 semaphore permits 给 currentProjectId 首页 eager。
- **broadcast push 基础设施保留不动**：翻页 / SSE 仍依赖；改动**只**收敛在"首页 cursor=None 时改走同步等真值 + 失败条 deferred retry 仍走 broadcast"这一路径。
- **删除前端 `.metadata-pending` 在首页路径上的实际触发**——首页 IPC 返回时 items 都是真值，`!session.title && messageCount === 0` 不再为 true → CSS class 不再触发 → 前端无需改 CSS / markup（行为自然消失）。翻页路径上仍可能有少量首屏前 patch，pending shimmer 仍然合理。
- **新增 perf bench** `crates/cdt-api/tests/perf_eager_first_page.rs`：量测 `list_sessions(cursor=None, page_size=20)` 与 `page_size=50`（验证 D5b 切割）的 wall-clock + user/sys + RSS，建立基线 + CI gate。

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `ipc-data-api`: `list_sessions` 首页 cursor=None 路径 SHALL 在 IPC return 前同步等待前 `EAGER_FIRST_PAGE_LIMIT = 20` 条 items 的 metadata 解析完成（带 500ms per-session timeout）；超时 / 失败条走 deferred 单条 retry；`Emit session metadata updates` Requirement 行为变更——eager 同步等到的条目**不**走 broadcast，超时 / 失败条仍 emit；`Session list pagination avoids duplicate full scans` 的 skeleton-first 契约 SHALL 限定为 cursor=Some 翻页路径；切 project 时新 project 首页 eager 启动前 abort 旧 project 翻页扫描让出 permits。
- `http-data-api`: `GET /api/projects/{id}/sessions` 首页 cursor=None 行为同步对齐——返回的 items 含 metadata 真值字段；首页路径不再触发 SSE `session-metadata-update` 事件流；浏览器 transport `list_sessions(cursor=null)` 跳过 `ensureSseReady()` 1000ms gate（首页不依赖 SSE）。
- `sidebar-navigation`: "骨架列表快速加载" Requirement 改为"首页元数据真值列表快速加载 + 翻页骨架+push"双路径语义；"会话元数据增量 patch" silent refresh 行为反转——`mergeSilentMetadata` 改用 `mergeRecoveryResponse` 语义让 response 真值覆盖 prev stale，避免旧 metadata 永久压住新 eager response。`pendingMetadataUpdates` buffer 仍保留兜底（翻页 race + eager deferred retry 仍可能触发）。

## Impact

- **Code（后端）**：
  - `crates/cdt-api/src/ipc/local.rs::list_sessions`（核心改动 + abort 旧 project 翻页扫描）
  - `crates/cdt-api/src/ipc/session_metadata.rs::extract_session_metadata_cached`（D6b 失败 metadata 不写 cache）
  - `crates/cdt-api/src/ipc/traits.rs::list_sessions` doc comment（D8 cursor 分叉契约）
  - `crates/cdt-api/tests/http_list_sessions_skeleton_then_sse.rs`（首页 expectation 改：直接断 items 含真值；新增 cursor != None 翻页用例覆盖原骨架+SSE 路径）
  - `crates/cdt-api/tests/session_metadata_stream.rs`（首页 cursor=None 不再 emit broadcast；超时 / 失败条 deferred retry 仍 emit；失败 metadata 不缓存重试）
  - `crates/cdt-api/tests/ipc_contract.rs::list_sessions`（contract 形状不变，但**值含义**变——首页 items 字段非占位）
  - 新建 `crates/cdt-api/tests/perf_eager_first_page.rs`（page_size=20 + page_size=50 D5b 切割 + projectA 翻页扫描 + projectB 首页 eager 复合场景）
- **Code（前端）**：
  - `ui/src/lib/transport.ts::BrowserTransport.invokeHttp`（D9 list_sessions cursor=null 跳过 ensureSseReady）
  - `ui/src/lib/transport.test.ts`（cursor 分叉 SSE-ready gate 用例）
  - `ui/src/lib/sessionMerge.ts::applySilentRefresh`（D3b 改用 `mergeRecoveryResponse` 语义）
  - `ui/src/lib/sessionMerge.test.ts`（silent refresh response 真值覆盖 prev stale 用例）
- **Perf**：首屏 IPC return 延迟分档（codex v2 复审 issue 6）——**p50 < 300ms / p95 < 500ms / worst < 1500ms（含 D7 timeout 回退）**。538 session corpus 前 20 条 wall 实测预计 p50 ~150-250ms；超 timeout 时 worst 受 `EAGER_PER_SESSION_TIMEOUT = 500ms` × `ceil(20/8)` = ~1500ms 上限保护。`.claude/rules/perf.md` 的"冷启动 list_repository_groups"基线不变，新增 list_sessions 首页基线。
- **API contract**：`SessionSummary` 字段不变；语义变——首页响应前 20 条字段已是真值（不再是占位）；超时 / 失败条仍可能占位 + 后续 broadcast emit 兜底。
- **Frontend**：sidebar `metadata-pending` shimmer 在首页 cursor=None 路径上不再触发（行为自然降级），翻页 / file-change silent refresh 路径仍然触发；silent refresh 走 eager 后体感同步无跳变。
- **不影响**：HTTP SSE `/api/events` channel 基础设施、broadcast channel capacity、Tauri emit 桥、`session-metadata-update` payload 结构、`.metadata-pending` CSS / markup。
