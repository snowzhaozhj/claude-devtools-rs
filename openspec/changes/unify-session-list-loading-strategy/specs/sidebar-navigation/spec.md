## ADDED Requirements

### Requirement: Sessions store stale-while-revalidate 缓存

Sidebar SHALL 通过新增的模块级单例 store（`ui/src/lib/sessionListStore.svelte.ts`）以 `projectId` 为 key 缓存最近访问过的 `PaginatedResponse<SessionSummary>` 列表（含已 patch 的 metadata）。切 project 时（含初次访问 / 来回切换）Sidebar 触发 `loadSessions` SHALL 先从 store 同步读取缓存：

- **命中**（store 有该 `projectId` 条目）：Sidebar SHALL 立即用缓存数据 hydrate 列表（`sessions` / `sessionsNextCursor` / `sessionsTotal` 三态），**不**经过"加载中..."文本中间态；同时后台 SHALL 触发 SWR refresh（重新调 `listSessions` 拉首页），refresh 返回时 SHALL 通过下文规约的"首页 refresh ghost reconcile"路径 merge 进当前列表（保留尾部、保留分页 cursor），与现有 file-change 兜底刷新路径行为一致。
- **未命中**（store 无该 `projectId` 条目）：Sidebar SHALL 走现有"非 silent 替换式加载"路径——`sessionsLoading=true` + 等 `listSessions` resolve + replace 首页；resolve 后 store SHALL 写入该 `projectId` 条目。

**首页 refresh ghost reconcile**：SWR refresh 是首页（`cursor=null`）请求时，store 与 Sidebar `sessions` 数组的合并 SHALL 满足：
- **新 page 内出现的 sessionId** SHALL 用 refresh 数据覆盖（含 metadata 字段）
- **新 page 的 `pageSize` 范围内但** refresh 数据中**缺失**的 sessionId（即落在 mtime 倒序前 `pageSize` 条但服务端已不返回的）SHALL 从 store 与 Sidebar `sessions` 中**移除**——表示该 session 文件已被删除 / 重命名 / 移出首页范围
- 超出首页 `pageSize` 范围的尾部条目 SHALL 保留（pinned/hidden reconcile 与翻页累加的尾部不受 refresh 影响）

非首页（`cursor !== null`）的 refresh / loadMore SHALL NOT 触发上述删除 reconcile——仅作为追加列表使用，保留既有 `applySilentRefresh` "merge 保留尾部 + 保留 cursor" 行为不变。

Store 容量 SHALL 按 LRU 上限 16 个 `projectId` 淘汰；命中时 SHALL bump 到队首避免冷热混淆。Store **不**持久化到磁盘——进程重启时为空，依赖后端 `MetadataCache` 持久化（详见 `ipc-data-api` spec §"`MetadataCache` 启动 hydrate 与退出 dump"）让冷启时骨架阶段直接命中真值。

Metadata patch 路径（`session-metadata-update` event listener）SHALL 同时写入 store —— in-place mutate 缓存条目内对应 sessionId 的字段，保持 store 与显示列表的一致性，避免下次切回此 project 时缓存返回过期值。

**已知 stale-update race（接受作为最佳努力）**：用户在 A → B → A 快速切换路径 + 期间 A 项目某 session 文件变更时，第一次 A 访问触发的旧扫描可能在 abort 之前已 emit 出旧值的 `SessionMetadataUpdate`，事件在 Tauri queue / SSE wire 上滞后到用户切回 A 时才被 listener 处理，旧 update 会**短暂覆盖**新 metadata 值。该 race 的触发窗口窄（200–500 ms 内 A→B→A + 文件同期变更），file-change watcher debounce 100 ms 后会触发 silent refresh 拉回真值兜底。本 capability **不**引入额外 IPC schema 字段（如 `scanToken` / `generationId`）来精确丢弃 stale update——接受为已知 race，不规约 listener 侧的 scanToken 校验逻辑。

#### Scenario: 切回曾访问的 project 时立即展示缓存

- **WHEN** 用户先选中 project A 触发 `loadSessions("A")` 完成（store 写入 A 的 `SessionListEntry`），然后选中 project B 触发 `loadSessions("B")`，再次选中 project A
- **THEN** Sidebar SHALL 立即用 store 中 A 的缓存数据 hydrate 列表（`sessions` 数组复用缓存项，DOM 复用稳定 key），**不**显示"加载中..."文本中间态
- **AND** 后台 SHALL 触发对 A 的 SWR refresh（再次调 `listSessions("A", 20)`），返回时通过 `applySilentRefresh` merge

#### Scenario: 首次访问 project 走非 silent 加载

- **WHEN** 用户首次选中某 project，store 中无该 `projectId` 条目
- **THEN** Sidebar SHALL 走非 silent 替换式加载路径（`sessionsLoading=true` + 等 `listSessions` resolve）
- **AND** resolve 后 store SHALL 写入该 `projectId` 的 `SessionListEntry`

#### Scenario: Metadata patch 同步更新 store

- **WHEN** `session-metadata-update` listener 收到 sessionId 为 `S` 的更新
- **THEN** 系统 SHALL 同时对 store 中该 `projectId` 条目内的 `S` session 字段 in-place mutate（`title` / `messageCount` / `isOngoing` / `gitBranch`）
- **AND** 下次切回此 project 走 store cache hit 路径时，SHALL 直接展示已 patch 的真值

#### Scenario: Store LRU 超过 16 个 project 时淘汰

- **WHEN** Store 已含 16 个 `projectId` 条目，用户访问第 17 个 project 触发新条目写入
- **THEN** Store SHALL 淘汰当前最久未访问的条目后再写入新条目，store 大小始终 ≤ 16

#### Scenario: 首页 SWR refresh 删除已不存在的 session

- **WHEN** Store 中 project A 缓存含 sessionId `s1, s2, s3, s4, s5`（pageSize=20，全部在首页范围内）；用户切回 A，后台 SWR refresh 首页（cursor=null）返回 `s1, s2, s4, s5, s6`（`s3` 已被删除、`s6` 是新增）
- **THEN** Store 与 Sidebar `sessions` 数组 SHALL：保留 / 覆盖 `s1, s2, s4, s5`；移除 `s3`；插入 `s6`
- **AND** 显示的 `sessionsTotal` SHALL 用 refresh response 的 `result.total` 覆盖

#### Scenario: 非首页 refresh 不触发删除 reconcile

- **WHEN** Store 中 project A 已加载 page 1+2（cursor 已推进），随后 file-change 触发 silent refresh，但前端按"首页 only"策略仅 refresh `cursor=null`
- **THEN** refresh 返回的首页数据 SHALL 用 ghost reconcile 路径合并；page 2 尾部 sessionId 在 refresh 数据外的 SHALL **保留**（不被误删，因为它们超出 refresh 的 pageSize 范围）

### Requirement: Store `loadFirstPage` / `loadMore` 内部 generation token 取消机制

`sessionListStore` 的 `loadFirstPage(projectId, ...)` / `loadMore(projectId)` API SHALL 用 **generation token** 机制取消已过时的 in-flight 请求，让 store 自身的并发 SWR refresh / 翻页路径在快速调用时不会让旧 response 错误地覆盖更新的 entry 状态。

实现 SHALL 满足：

- store 在每个 `SessionListEntry` 上维护 `generation: number` 字段，每次 `loadFirstPage` / `loadMore` 启动时 `++entry.generation` 并记录 `my = entry.generation`
- 调用 `listSessions(...)` resolve 时 SHALL 检查 `entry.generation === my`，不等则丢弃 response（不写入 store）
- **浏览器 runtime** SHALL 额外创建 `AbortController` 挂到 fetch 路径；新 generation 启动时 SHALL `previousController.abort()` 让网络层立即释放连接
- **Tauri runtime** 由于 `invoke()` 不支持 abort，generation token 是唯一手段；后端 `LocalDataApi::list_sessions` 既有 `active_scans` per-`(projectId, cursor)` abort 机制 SHALL 自然处理后台扫描去重，前端无需主动通知后端

**Sidebar 集成边界**：Sidebar 当前**未**强制通过 store API 调 `loadFirstPage` / `loadMore`——继续走自己的 `listSessions` 直调 + `selectedProjectId` / `sessionsNextCursor` 校验路径，并通过 `sessionsLoadingMore` flag 防同 cursor 重复加载。store API 的 cancel 机制保留作为 SWR refresh 调用 + 未来 sidebar 完全使用 store 重构时的契约（详 design.md D4b）。

#### Scenario: store 内部并发 loadFirstPage 仅保留最新 response

- **WHEN** 调用方对同一 projectId 在第一次 `loadFirstPage` IPC 未 resolve 时再次调用 `loadFirstPage`
- **THEN** store SHALL `++entry.generation` 让第一次 response resolve 时被 `generation` 校验丢弃
- **AND** 浏览器 runtime SHALL `controller_first.abort()` 让网络层立即释放连接
- **AND** 仅最后一次 response SHALL 写入 `entry.sessions` / `entry.nextCursor` / `entry.total`

#### Scenario: store loadMore 同 cursor 不重复 fetch

- **WHEN** store `loadMore("A")` 启动 cursor=`C1` 的请求；请求未 resolve 时再次调 `loadMore("A")`（cursor 未推进，仍是 `C1`）
- **THEN** 第二次调用 SHALL 因 inflight short-circuit 直接 return，不产生新 IPC

### Requirement: Store `loadMore` 实现 leading + trailing debounce 100 ms

`sessionListStore.loadMore(projectId)` API 自身 SHALL 实现 **leading + trailing 组合** debounce 100 ms，让调用方在高频触发场景下（如未来 sidebar 把 `maybeLoadMoreSessions` 直接转发到 store）无需自己实现限频。

1. **Inflight short-circuit（最先判断）**：`entry.inflightCursor === currentCursor` 时直接 return（已有相同 cursor 的请求在飞），**不**进 debounce 队列
2. **Leading**：当前 `lastFiredAt` 距 now ≥ 100 ms（不在 cooldown 窗口内）→ 立即 fire fetch，记录 `lastFiredAt = now`
3. **Trailing**：当前 `lastFiredAt` 距 now < 100 ms（在 cooldown 窗口内）→ 重置 trailing timer 到 `lastFiredAt + 100 ms`；timer 触发时**再次走 inflight short-circuit 判定**，若仍未 inflight 才发 fetch；timer 已 pending 则不重复 schedule

100 ms 是人类感知滚动停顿阈值（< 100 ms 视为连续滚动；> 100 ms 视为停顿）。

**Sidebar 集成边界**：当前 Sidebar 的 `loadMoreSessions` **不**直接调 `store.loadMore`——继续走原 `listSessions(projectId, pageSize, cursor)` IPC 直调路径，并通过 `sessionsLoadingMore` flag 提供 leading-fire + inflight short-circuit 等效保护。Sidebar 现有 `maybeLoadMoreSessions` 由 scroll 事件触发，scroll 事件在用户停下后会自然停止，trailing-fire 的边际收益（仅在用户停顿在 threshold 边缘 99 ms 后又继续滚的极端情况）相对引入 store-sidebar reactive 同步复杂度（subscribe / unsubscribe / pendingMetadataUpdates buffer 与 store entry 的双写）不划算（详 design.md D5b）。store.loadMore 的 leading+trailing 实现保留作为可选 API + 未来重构契约。

#### Scenario: store loadMore leading 立即触发 + inflight short-circuit

- **WHEN** 调用方连续调 `store.loadMore("A")` 共 5 次，每次间隔 20 ms（总 100 ms 内）
- **AND** 第一次 fetch 仍在飞（未 resolve）
- **THEN** store SHALL 在第 1 次调用立即 fire 1 次 IPC（leading）；第 2-5 次调用 SHALL 因 inflight short-circuit 全部丢弃

#### Scenario: store loadMore cooldown 内多次调用合并为一次 trailing fire

- **WHEN** 第一次 `store.loadMore("A")` leading fire 后 fetch 已 resolve（不再 inflight）；接下来 100 ms cooldown 内调用方再调 3 次 `loadMore("A")`
- **THEN** 第 2-4 次调用 SHALL 合并为单一 trailing timer；trailing 触发时若仍未 inflight，SHALL 再 fire 1 次 fetch
- **AND** 总 fetch 数 SHALL ≤ 2（leading 1 + trailing 1）

#### Scenario: store loadMore 单次调用后停顿不重复 fire

- **WHEN** 调用方调 `store.loadMore("A")` 一次（leading fire），fetch 200 ms 后 resolve；其后调用方停止调用
- **THEN** store SHALL NOT 在 cooldown 结束时再次触发 fetch（无 pending trailing timer）

### Requirement: Sidebar SHALL 订阅 sse-recovered / sse-lagged 触发 silent refresh

为兜底两类 SSE 异常路径——`ensureSseReady()` 1000 ms 超时让 patch 永久丢失（codex 二审 issue 1）与 backend broadcast 容量打满让 patch 静默丢弃（codex 二审 issue 2）——Sidebar SHALL 在 `onMount` 阶段订阅 `sse-recovered` 与 `sse-lagged` 两个 transport 层 pseudo-event。

实现 SHALL 满足：

- 两个 event 共用同一恢复 handler，**两步并发**：
  - **(a) 首页重扫**：当前 `selectedProjectId` 非空时调 `loadSessions(selectedProjectId, true)` 走 silent merge 路径，让后端重启 page 1 扫描 + 重新 emit `SessionMetadataUpdate` patch（此时 SSE 已 OPEN / Lagged 之后 stream 已恢复），listener 将真值写回 sessions 与 store 缓存
  - **(b) page 2+ 精准补齐**：后端 `LocalDataApi::list_sessions` 仅扫 `take(pagination.page_size)` 当前页，silent reload 不覆盖**已加载** page 2+ 范围内仍 pending 的 sessionId（codex 二审 round 2 指出该 corner——用户翻到底 + SSE 在 page 2 IPC 期间异常时，尾部永远卡空）。Sidebar SHALL 收集 sessions 数组中**仍占位** sessionId（条件与 `.metadata-pending` shimmer 一致：`!title && messageCount === 0 && !isOngoing`），调一次 `getSessionSummariesByIds(projectId, pendingIds)` 同步补齐，返回的 summary 通过 `mergeSessions(_, _, false)` 写回 sessions 与 store 缓存
  - 异步完成时 race guard：`projectId !== selectedProjectId` SHALL 跳过写回避免污染新选中的 project
- silent merge / `mergeSessions` 都 SHALL 保留已 patch 真值不被骨架 `null` 覆盖
- handler SHALL 在 `onDestroy` 阶段清理 unsubscribe，与 `metadataUnlisten` 同一释放路径
- Tauri runtime（`TauriTransport`）的 `subscribeEvents` 不会派发 `sse-recovered` / `sse-lagged`（这两个 event 是 `BrowserTransport` 内部 synthesize），handler 不会被触发——本 Requirement 仅在 server-mode 浏览器 client 生效

#### Scenario: sse-recovered 触发当前 project 的 silent refresh

- **WHEN** Sidebar 已 mount + `selectedProjectId === "A"`
- **AND** transport 层因 `ensureSseReady` 超时设置 `sseRecoveryPending=true`，随后 SSE 真正 OPEN，emit 一次 `sse-recovered` event
- **THEN** Sidebar SHALL 调 `loadSessions("A", true)` 触发 silent refresh
- **AND** silent merge SHALL 保留之前已 patch 的 metadata 真值不被骨架值覆盖

#### Scenario: sse-lagged 同样触发 silent refresh

- **WHEN** SSE handler 因 `BroadcastStream::Lagged` 推送 `{"type":"sse_lagged"}` event 给浏览器 client
- **THEN** transport 层 SHALL 转 `sse-lagged` event name 派发给 Sidebar handler
- **AND** Sidebar SHALL 调 `loadSessions(selectedProjectId, true)` 触发 silent refresh
- **AND** 后续后端重新扫描 emit 的 `SessionMetadataUpdate` SHALL 通过 SSE patch 路径正常写回

#### Scenario: 已翻到 page 2+ 时 SSE 异常仍补齐尾部 metadata

- **WHEN** 用户已 scroll 到底加载 page 2 / page 3（`sessions` 数组含 page 1+2+3 共 60 条）
- **AND** transport 收到 `sse-recovered` 或 `sse-lagged` 事件
- **THEN** Sidebar SHALL 同时执行两步：
  1. silent `loadSessions(projectId, true)` 重扫 page 1（覆盖 page 1 内仍 pending 项）
  2. 收集 sessions 中所有仍 pending 的 sessionId（不限 page 1）调 `getSessionSummariesByIds(projectId, pendingIds)` 同步补齐
- **AND** 第 2 步返回的 summary 通过 `mergeSessions` 与 `cacheSessions` 写回，完成后 page 2/3 范围内此前因 SSE 异常丢失的 metadata SHALL 已显示真值

### Requirement: Metadata 占位字段视觉渐显

为避免 metadata 字段（`title` / `messageCount` / `isOngoing` / `gitBranch`）从骨架占位（`null` / `0` / `false`）到真值的瞬变带来视觉断层，骨架行 SHALL 用条件 CSS class `.metadata-pending` 标识占位状态，class 上 SHALL 挂统一的 shimmer 占位动画（如 `linear-gradient` 横向移动 1.5 s 循环）让"加载中"语义视觉化；元数据 patch 到达后 SHALL 移除 class，触发 CSS `transition: opacity 150ms ease-out` 让真值 fade-in。

实现 SHALL 满足：

- 每条 session 渲染时 SHALL 通过 `class:metadata-pending={!session.title && session.messageCount === 0 && !session.isOngoing}` 判定（与既有占位回退路径同条件）
- `transition` SHALL 用 CSS 而**非** Svelte `transition:fade`——metadata patch 是字段 mutate 不重建 DOM 节点，Svelte transition 指令绑定 mount/unmount 不触发
- 渐显时长 SHALL 在 100 ms ≤ X ≤ 200 ms 区间（取 150 ms 作为默认值）；过短等同瞬变无渐显感，过长让用户感到"卡顿等待"
- shimmer 动画 SHALL 与 metadata-pending class 同时存在 / 同时消失，**不**得在真值显示时仍闪烁

#### Scenario: 骨架行渲染时显示 shimmer + 占位文字

- **WHEN** Sidebar 渲染一条骨架 session（`title=null`，`messageCount=0`，`isOngoing=false`）
- **THEN** 该行 SHALL 携带 `.metadata-pending` class 触发 shimmer 动画
- **AND** title 区显示既有占位回退（如 sessionId 前 8 位 + "…"）

#### Scenario: Metadata patch 到达后字段渐显

- **WHEN** `session-metadata-update` listener 收到 sessionId 为 `S` 的更新，更新该 session 的 `title` 为 `"My Session"`
- **THEN** 该行 SHALL 在 patch 同帧移除 `.metadata-pending` class
- **AND** title 文本 SHALL 通过 CSS `transition: opacity 150ms ease-out` 从透明渐变到不透明
- **AND** 渐显完成后 shimmer 动画 SHALL 已停止
