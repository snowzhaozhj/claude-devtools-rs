## MODIFIED Requirements

### Requirement: 会话总数显示口径

Sidebar 顶部 `session-count-num` 元素 SHALL 表达"当前 scope 内一共有多少 session"——**用户不感知客户端分页内部状态**，分页加载进度由 sidebar 底部 `▼ 加载更多 · 剩 N 条` 按钮 + `已显示全部 N 条` 端状态承担（PR-A 已落地）；顶部 count 只显总量 + 搜索命中数两态。

**scope 定义**：
- 多 wt group 选中「全部」chip / 单 wt group / flat fallback：scope = group 全集
- 多 wt group 选中具体 worktree chip：scope = 该 worktree 集合

**两态显示**：

- **默认状态（filterQuery 为空）**：显示单数字 `{scopeTotal}`，例如 `127`（filter=「全部」）或 `8`（filter=具体 wt 且该 wt 共 8 个 session）。`scopeTotal` MUST 按 filter scope 派生：
  - filter=ALL_WORKTREES：`scopeTotal = selectedGroup?.totalSessions ?? sessions.length`（fallback 仅在 race window 内 selectedGroup 未就绪时兜底）
  - filter=具体 worktreeId：`scopeTotal = groupWorktrees.find(w => w.id === filter)?.sessions.length ?? sessions.length`（fallback 同上）
- **搜索激活状态（filterQuery 非空）**：显示 `{matchCount} 匹配`，例如 `5 匹配`。`matchCount` MUST 取 `visibleSessions.length`，即客户端已加载范围内 + filterQuery 命中 + 非隐藏的剩余条数。**搜索的 scope 限制 SHALL 通过 search input 的 `aria-describedby` / `title` 属性以 "在已加载范围内搜索" 文本明示用户**——避免用户把 `5 匹配` 误读为"全 scope 命中数"，特别是仍有未加载页的大 group。当 `sessionsNextCursor` 非 null（仍有未加载页）且 filterQuery 非空时，sidebar 可选择性自动 silent loadMore 直到全 scope 加载完，让 matchCount 收敛到全 scope 命中数（非 MUST，但作为优化方向）。

**hover tooltip**：基础显示一层 `总 {scopeTotal}`；当 `hiddenCount > 0` 时 SHALL 追加 ` · {hiddenCount} 已隐藏`。`hiddenCount === 0` 时 SHALL 仅显示一层（避免 ` · 0 已隐藏` 噪音）。tooltip 不暴露分页已加载条数——加载进度由列表底部 `▼ 加载更多 · 剩 N 条` 按钮承载，避免顶部 + 底部双处表达同一概念造成用户认知冗余。

**`scopeTotal` 数据来源链路（统一权威路径）**：

- `list_repository_groups` IPC 后端返回 `RepositoryGroup.totalSessions`（grouper 计算的 group 跨 wt 真值，**唯一权威源**）+ `RepositoryGroup.worktrees[].sessions: string[]`（每 wt 内 session id 列表）
- 前端 `selectedGroup` 由 `repositoryGroups.find(g => g.id === selectedGroupId)` derived；`groupWorktrees = selectedGroup?.worktrees ?? []` derived
- ALL scope 取 `selectedGroup.totalSessions`；具体 wt scope 取 `groupWorktrees.find(...).sessions.length`——两者都直接从 `list_repository_groups` derived 出，**无需第二个本地 state**

`listSessions` / `list_group_sessions` 翻页 IPC 的 `result.total` 字段含义与 `RepositoryGroup.totalSessions` 在 ALL scope 下等同（后端不变量），但前端 SHALL 直接消费 `selectedGroup.totalSessions` derived，不另行存储 `result.total` 到独立 state（避免命名链路冗余）。

**silent 刷新触发 `list_repository_groups` SWR revalidate 的条件**：

silent 刷新（file-change 事件触发或「有更新」按钮触发）SHALL 仅在 file-change payload 满足 `projectListChanged === true || sessionListChanged === true || deleted === true` 任一条件时才 schedule `loadProjects(refresh: true)`（`list_repository_groups` SWR revalidate）。其它情况（普通 JSONL append + watcher 折叠的 subagent 修改：三个标志全 false）SHALL NOT 触发 `loadProjects`，避免活跃 session 持续追加消息时 sidebar 高频 IPC 噪声（telemetry 数据：原 1437 次 / 2.18h ≈ 每 5.4s 一次，治理后 ≈ 109 次 / 2.18h ≈ 每 72s 一次）。

`loadMoreSessions` 翻页路径 SHALL **不**修改 `selectedGroup.totalSessions`（页内 total 不应改变）。

#### Scenario: 默认状态 + 全部 worktree filter 显 group total
- **WHEN** Sidebar 首次加载某 group（group 实际 127 个 session 跨多 wt），filter 选「全部」
- **AND** filterQuery 为空
- **THEN** `session-count-num` SHALL 显示单数字 `127`，**不**显示分式（`{已加载}/{总}` 形式）也**不**显示已加载条数后缀

#### Scenario: 默认状态 + 选中具体 worktree 显 wt total
- **WHEN** group 含 worktree `wt-A`（8 个 session）/ `wt-B`（120 个 session），用户切到 `⌗wt-A` chip
- **AND** filterQuery 为空
- **THEN** `session-count-num` SHALL 显示单数字 `8`，**不**显示 `128`（用 group 全集会让用户在该 wt scope 下产生"还有 120 条"误读）

#### Scenario: loadMore 翻页不影响顶部总量
- **WHEN** 用户已加载 page 1（20 条）；调用 `loadMoreSessions` 加载 page 2（再 20 条）
- **THEN** `session-count-num` 显示 `60` 始终不变（顶部 count 不参与分页进度信号）
- **AND** 列表底部 `▼ 加载更多 · 剩 N 条` 按钮 SHALL 同步从 `剩 40 条` 变为 `剩 20 条`（PR-A 已落地的端状态）

#### Scenario: 搜索激活状态显 match 命中数
- **WHEN** 用户在 `scopeTotal=127` 状态下输入 filterQuery 命中（`visibleSessions.length === 5`）
- **THEN** `session-count-num` SHALL 显示 `5 匹配`，**不**再显示 `127`
- **AND** search input SHALL 含 `aria-describedby` / `title` 属性以"在已加载范围内搜索"文本明示 scope 限制（避免用户在仍有未加载页时误读为全 scope 命中数）
- **AND** 用户清空 filterQuery 后 SHALL 回到单数字 `127` 默认显示

#### Scenario: hidden=0 时 tooltip 仅显一层
- **WHEN** 用户 hover `session-count-num`，当前 scopeTotal=127 / hiddenCount=0
- **THEN** native tooltip SHALL 显示 `总 127`，**不**显示 `· 0 已隐藏` 后缀

#### Scenario: hidden>0 时 tooltip 追加 hidden
- **WHEN** 用户 hover `session-count-num`，当前 scopeTotal=127 / hiddenCount=5
- **THEN** native tooltip SHALL 显示 `总 127 · 5 已隐藏`

#### Scenario: silent 刷新 sessionListChanged 时 scopeTotal 同步刷新
- **WHEN** 后端 unified invalidator 检测到"已知 project 下新 session 首次出现"（规则 2 命中），enrich `FileChangeEvent` 时把 `session_list_changed` 置为 `true`
- **AND** Tauri host emit `file-change` payload `{ projectId: "pa", sessionId: "sa_new", deleted: false, projectListChanged: false, sessionListChanged: true }`
- **AND** 前端 Sidebar handler 收到 payload，filter=「全部」
- **THEN** Sidebar SHALL schedule `loadProjects(refresh: true)`（`list_repository_groups` SWR revalidate）
- **AND** revalidate 拉到新 `RepositoryGroup.totalSessions = 128`（含 sa_new）
- **AND** `selectedGroup.totalSessions` derived 自动更新为 128
- **AND** 默认状态显示 SHALL 立即从 `127` 切到 `128`

#### Scenario: silent 刷新 deleted 时 scopeTotal 同步下降（ALL scope）
- **WHEN** 后端 unified invalidator 收到 `FileChangeEvent { deleted: true }` 走规则 1，enrich `session_list_changed: true`
- **AND** Tauri host emit `file-change` payload 含 `deleted: true, sessionListChanged: true`
- **AND** filter=「全部」
- **THEN** Sidebar SHALL schedule `loadProjects(refresh: true)`，revalidate 拉到新 `RepositoryGroup.totalSessions = 126`
- **AND** `selectedGroup.totalSessions` derived 自动更新为 126
- **AND** 默认状态显示 SHALL 立即从 `127` 切到 `126`
- **AND** 若被删除的 session 在已加载范围内，sidebar SHALL 同步从 `sessions` 数组移除该条；若不在已加载范围（仍在远端未翻到的部分），仅顶部 count 下降，已加载列表不变

#### Scenario: silent 刷新 sessionListChanged 时 scopeTotal 同步下降（具体 worktree scope）
- **WHEN** filter 选中 `⌗wt-A`（原 `wt-A.sessions.length === 8`）
- **AND** Tauri host emit `file-change` payload 含 `sessionListChanged: true`（wt-A 内 1 个 session 被删除触发）
- **THEN** Sidebar SHALL schedule `loadProjects(refresh: true)`，revalidate 拉到新 `RepositoryGroup.worktrees[0].sessions.length === 7`
- **AND** `groupWorktrees.find(w => w.id === filter)?.sessions.length` derived 自动更新为 7
- **AND** 默认状态显示 SHALL 立即从 `8` 切到 `7`
- **AND** 若被删除的 session 在已加载范围内，sidebar SHALL 同步从 `sessions` 数组移除该条

#### Scenario: 普通 JSONL append SHALL NOT 触发 loadProjects
- **WHEN** 活跃 session 持续追加消息，后端 unified invalidator 走规则 3（`content_append_skipped`），enrich `FileChangeEvent` 时 `session_list_changed: false`
- **AND** Tauri host emit `file-change` payload `{ projectId: "pa", sessionId: "sa", deleted: false, projectListChanged: false, sessionListChanged: false }`
- **AND** 前端 Sidebar handler 收到 payload
- **THEN** Sidebar MUST NOT schedule `loadProjects(refresh: true)`
- **AND** Sidebar 仍 SHALL schedule `loadSessions(currentGroupId, silent: true)`（保持现有 `Auto refresh session list on file change` 契约不变——session 内消息变化仍需刷新当前 group session list）
- **AND** `selectedGroup.totalSessions` 不变（普通 append 不改变 session 集合）

#### Scenario: 旧客户端反序列化缺 sessionListChanged 字段时退化为不触发 loadProjects
- **WHEN** 由于版本不匹配 / SSE 历史回放等原因，前端收到的 file-change payload 缺 `sessionListChanged` 字段
- **THEN** 前端反序列化 SHALL 把缺字段视为 `false`（`#[serde(default)]` 行为）
- **AND** 当 `projectListChanged === false && deleted === false` 时 Sidebar SHALL NOT 触发 `loadProjects(refresh: true)`
- **AND** 该退化行为可接受（structural 信号通过 `projectListChanged` / `deleted` 两字段仍能覆盖 watcher 层信号；遗失的"已知 project 内首次见 session"场景由 5min `LOCAL_CACHE_TTL` 兜底）

### Requirement: Sidebar SHALL 订阅 sse-recovered / sse-lagged 触发 silent refresh

为兜底两类 SSE / IPC 异常路径——`ensureSseReady()` 1000 ms 超时让 patch 永久丢失（codex 二审 issue 1）、backend broadcast 容量打满让 patch 静默丢弃（codex 二审 issue 2）、以及 `LocalDataApi.file_tx` broadcast `Lagged` 让 enriched file-change event 错过（change `enrich-file-change-with-session-list-changed`）——Sidebar SHALL 在 `onMount` 阶段订阅 `sse-recovered` 与 `sse-lagged` 两个 transport 层 pseudo-event。

实现 SHALL 满足：

- 两个 event 共用同一恢复 handler：当前 `selectedProjectId` 非空时调 `listSessions(projectId, Math.max(sessions.length, SESSION_PAGE_SIZE))` 触发后端按**已加载范围**重新扫描——后端 `LocalDataApi::list_sessions` 按 `take(pagination.page_size)` 截断，pageSize=sessions.length 等价于"扫描已加载范围"
- handler SHALL 同时 schedule `loadProjects(refresh: true)`（`list_repository_groups` SWR revalidate）—— lag 期间可能错过 enriched file-change event 的 structural 信号（`projectListChanged` / `sessionListChanged` / `deleted`），保守 SWR revalidate 让 `selectedGroup.totalSessions` 与最新 group 集合对齐
- handler SHALL **消费 response** 通过 `mergeRecoveryResponse(sessions, result.items)` 写回 sessions + store。recovery 路径**不**叠加 `applyPendingMetadata`——buffer 可能保留了 lag 之前的旧 SSE patch（buffer 跨 SSE 异常周期持久），叠加会让 buffer 旧值覆盖刚刚 mergeRecoveryResponse 写入的 response 新真值，stale 自愈失败（codex 二审 round 6）。`mergeRecoveryResponse` 是 SSE 恢复路径**专用**合并（**不**复用 `mergeSilentMetadata` / `mergeSessions`）：
  - **cache hit 真值仅在 response 里**：后端 `try_lookup_cached_metadata` fast-path inline 返完整 metadata，**不**入后台扫描 spawn、**不** emit SSE patch；前端 SHALL 让 response 真值覆盖 prev（即使 prev 已含真值——recovery 触发本身意味着 SSE 中间已断/lag，prev 真值可能是几分钟前的 stale 值，response 来自 cache 当前状态更新）
  - **cache miss 真值**仍走 SSE patch 路径——后端 spawn 后台扫描后广播 `SessionMetadataUpdate`，UI `session-metadata-update` listener 写回；response 里 cache miss 项是骨架，`mergeRecoveryResponse` 在 next 是骨架时保留 prev（可能含已 patched 真值），等 listener 后续写入
  - prev 中不在 next 内的尾部条目 SHALL 保留（防 next.length < prev.length 漏项；删除走专门 ghost reconcile 路径）
- race guard：异步完成时 `projectId !== selectedProjectId` SHALL 跳过写回，避免污染新选中 project
- 设计取舍（codex 二审 4 轮验证后定型）：
  - **round 1**（silent loadSessions 重扫 page 1）→ page 2+ pending 永久卡空（round 2 反例）
  - **round 2**（+ `getSessionSummariesByIds` batch 补齐）→ 该 IPC 是 light skeleton（`title=null` / `messageCount=0`，契约固化于 `crates/cdt-api/tests/ipc_contract.rs::get_session_summaries_by_ids_returns_light_summaries`），无效（round 3 反例）
  - **round 3**（listSessions(已加载范围) fire-and-forget）→ cache hit fast-path inline 返真值 + **不** emit SSE patch，response 丢弃后真值丢失（round 4 反例）
  - **round 4**（listSessions(已加载范围) + 消费 response 走 mergeSessions）→ `mergeSilentMetadata` 总是保留 prev 真值，response 中的"cache hit 最新真值"被 prev 的 stale 真值掩盖（round 5 反例）
  - **round 5**（mergeRecoveryResponse + applyPendingMetadata）→ pendingMetadataUpdates buffer 在 SSE 异常前可能存了旧 patch；applyPendingMetadata 用 buffer 旧值反向覆盖 mergeRecoveryResponse 写入的 response 新真值，stale 仍卡（round 6 反例）
  - **round 6（当前定型）**：仅用 `mergeRecoveryResponse`，**不**叠加 applyPendingMetadata——recovery 路径 sessions 已含全部 sessionId（pageSize=sessions.length），listener 同时走 sessions.map in-place patch，buffer 兜底场景在 recovery 时不会发生；0 新 IPC、0 后端改动、0 contract 破坏
- handler SHALL 在 `onDestroy` 阶段清理 unsubscribe，与 `metadataUnlisten` 同一释放路径

**Tauri runtime 兼容性**：`sse-recovered` 历来由 `BrowserTransport` 内部 synthesize（仅 server-mode 浏览器 client 触发）；`sse-lagged` SHALL 由两路 emit 共同承担：

- **server-mode 浏览器路径**：`BrowserTransport` 在 SSE `BroadcastStream::Lagged` 路径继续 synthesize（既有行为不变）
- **Tauri runtime 路径**：Tauri host 在 `LocalDataApi.file_tx` broadcast bridge 收到 `RecvError::Lagged(_)` 时 `app.emit("sse-lagged", { source: "file-change", missed: n })`；`TauriTransport` SHALL 显式 `listen("sse-lagged", payload)` 后通过 dispatch 路径 fanout 给所有 handler（与 BrowserTransport synthesize 路径形态一致）

前端 Sidebar 的 sse-lagged / sse-recovered 订阅 SHALL **不再**包在 `if (!isTauriRuntime())` 门禁内——两 runtime 下 handler 都注册：

- Tauri 下 `sse-lagged` 通过 `TauriTransport.listen` 路径触发 handler；`sse-recovered` 在 Tauri 下不会被 emit（IPC channel 不会"恢复"），但订阅 noop 无副作用
- server-mode 下 `sse-recovered` / `sse-lagged` 通过 `BrowserTransport` synthesize 路径触发 handler（既有行为）

#### Scenario: sse-recovered 触发当前 project 的 silent refresh

- **WHEN** Sidebar 已 mount + `selectedProjectId === "A"`
- **AND** transport 层因 `ensureSseReady` 超时设置 `sseRecoveryPending=true`，随后 SSE 真正 OPEN，emit 一次 `sse-recovered` event
- **THEN** Sidebar SHALL 调 `loadSessions("A", true)` 触发 silent refresh
- **AND** Sidebar SHALL 同时 schedule `loadProjects(refresh: true)` 让 `selectedGroup.totalSessions` 与最新真值对齐
- **AND** silent merge SHALL 保留之前已 patch 的 metadata 真值不被骨架值覆盖

#### Scenario: sse-lagged 同样触发 silent refresh（server-mode 浏览器）

- **WHEN** SSE handler 因 `BroadcastStream::Lagged` 推送 `{"type":"sse_lagged"}` event 给浏览器 client
- **THEN** transport 层 SHALL 转 `sse-lagged` event name 派发给 Sidebar handler
- **AND** Sidebar SHALL 调 `loadSessions(selectedProjectId, true)` 触发 silent refresh
- **AND** Sidebar SHALL 同时 schedule `loadProjects(refresh: true)` 兜底 lag 期间错过的 structural 信号
- **AND** 后续后端重新扫描 emit 的 `SessionMetadataUpdate` SHALL 通过 SSE patch 路径正常写回

#### Scenario: Tauri runtime 下 file_tx Lagged 触发 sse-lagged

- **WHEN** Tauri host 的 file-change bridge 调 `LocalDataApi::subscribe_file_changes()` 后 `recv()` 返回 `Err(RecvError::Lagged(n))`（`LocalDataApi.file_tx` capacity=256 满 + slow renderer）
- **THEN** Tauri host bridge SHALL 调 `app.emit("sse-lagged", { source: "file-change", missed: n })` 让 webview 通过 `listen("sse-lagged", ...)` 收到
- **AND** `TauriTransport` SHALL 通过 `app.listen("sse-lagged", ...)` 桥接到 dispatch 路径让所有 handler 收到（与 BrowserTransport synthesize 形态一致）
- **AND** Sidebar 通过 Tauri `subscribeEvents` 订阅的 `sse-lagged` handler SHALL 触发，调 `loadSessions(selectedProjectId, true)` + `loadProjects(refresh: true)`
- **AND** bridge SHALL NOT 退出 loop，继续处理后续 event

#### Scenario: Sidebar sse 订阅在 Tauri runtime 下也注册

- **WHEN** Sidebar `onMount` 在 Tauri runtime 下执行
- **THEN** sse-lagged / sse-recovered 订阅注册 SHALL **不**被 `isTauriRuntime()` 门禁包裹，handler 注册路径在两 runtime 下统一
- **AND** Tauri runtime 下 `sse-recovered` 不会被触发（订阅 noop 无副作用）；`sse-lagged` 在 Tauri host bridge 检测到 `file_tx` Lagged 时通过 `app.emit` 触发

#### Scenario: 已翻到 page 2+ 时 SSE 异常仍补齐尾部 metadata

- **WHEN** 用户已 scroll 到底加载 page 2 / page 3（`sessions` 数组含 page 1+2+3 共 60 条）
- **AND** transport 收到 `sse-recovered` 或 `sse-lagged` 事件
- **THEN** Sidebar SHALL 调 `listSessions(projectId, 60)` 触发后端按已加载 60 条范围扫描
- **AND** Sidebar SHALL **消费** response 通过 `mergeRecoveryResponse(_, result.items)` 写回 sessions 与 store 缓存——cache hit 项的真值从 response 拿（让 response 真值覆盖 prev stale 真值）；**不**叠加 `applyPendingMetadata`（避免 buffer 旧值反向覆盖）
- **AND** cache miss 项的真值仍走后端 spawn 扫描 + SSE 广播 `SessionMetadataUpdate` 的 listener patch 路径写回；`mergeRecoveryResponse` 在 next 是骨架时保留 prev（可能含已 patched 真值）
