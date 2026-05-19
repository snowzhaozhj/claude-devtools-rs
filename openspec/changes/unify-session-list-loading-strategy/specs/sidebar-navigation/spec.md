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

### Requirement: In-flight 列表请求按 generation token 取消

Sidebar 通过 store 发起的 `loadSessions` / `loadMore` SHALL 用 **generation token** 机制取消已过时的 in-flight 请求，避免快速切 project / 快速翻页时旧 response 错误地覆盖当前列表状态。

实现 SHALL 满足：

- store 在每个 `SessionListEntry` 上维护 `generation: number` 字段，每次 `loadFirstPage(projectId, ...)` / `loadMore(projectId)` 启动时 `++entry.generation` 并记录 `my = entry.generation`
- IPC 或 HTTP fetch resolve 时 SHALL 检查 `entry.generation === my`，不等则丢弃 response（不写入 store、不更新 Sidebar 显示）
- **浏览器 runtime** SHALL 额外创建 `AbortController` 挂到 `fetch(..., { signal: controller.signal })`；新 generation 启动时 SHALL `previousController.abort()` 让网络层立即释放连接
- **Tauri runtime** 由于 `invoke()` 不支持 abort，generation token 是唯一手段；后端 `LocalDataApi::list_sessions` 既有 `active_scans` per-`(projectId, cursor)` abort 机制 SHALL 自然处理后台扫描去重，前端无需主动通知后端

#### Scenario: 快速切 project 取消旧请求

- **WHEN** 用户在 project A 的 `loadSessions("A")` IPC/fetch 尚未 resolve 时立即切到 project B 触发 `loadSessions("B")`
- **THEN** 浏览器 runtime SHALL `controller_A.abort()`，A 的网络连接立即释放
- **AND** 若 A 的 response 因竞争 resolve，store SHALL 因 `generation` 不等而丢弃，**不**覆盖当前显示的 B 列表

#### Scenario: 快速翻页时旧 cursor response 被丢弃

- **WHEN** 用户滚动触发 `loadMore("A")` 启动 cursor=`C1` 的请求，request 未 resolve 时再次滚动触发 cursor=`C2` 的请求
- **THEN** store SHALL `++entry.generation` 让 `C1` 的 response resolve 时被 `generation` 校验丢弃
- **AND** 仅 `C2` 的 response SHALL 写入 store

### Requirement: `loadMoreSessions` leading + trailing debounce 100 ms

`maybeLoadMoreSessions` 由滚动事件高频触发（每帧 ~16 ms），当 cursor 尚未 resolve 且滚动 threshold 持续命中时若不限频会产生多个 pending promise。**纯 trailing debounce 在用户慢速滚动（每 110 ms 触发一次）时无法合并**——每次都越过 100 ms 边界，仍产生重复 fetch + 后端 active_scans 抢占浪费。**纯 leading debounce 在 cooldown 结束后停顿用户的下一次滚动信号**会被丢弃，感受到"翻页迟钝"。Sidebar SHALL 在 store 的 `loadMore(projectId)` 入口实现 **leading + trailing 组合** debounce 100 ms：

1. **Inflight short-circuit（最先判断）**：`entry.inflightCursor === currentCursor` 时直接 return（已有相同 cursor 的请求在飞），**不**进 debounce 队列
2. **Leading**：当前 `lastFiredAt` 距 now ≥ 100 ms（不在 cooldown 窗口内）→ 立即 fire fetch，记录 `lastFiredAt = now`
3. **Trailing**：当前 `lastFiredAt` 距 now < 100 ms（在 cooldown 窗口内）→ 重置 trailing timer 到 `lastFiredAt + 100 ms`；timer 触发时**再次走 inflight short-circuit 判定**，若仍未 inflight 才发 fetch；timer 已 pending 则不重复 schedule

inflight short-circuit SHALL **在 debounce 触发前判定**——leading fire 前先 short-circuit，避免后端 active_scans 多走一次 spawn/abort 循环；trailing timer 触发时也 SHALL 重判 inflight。100 ms 是人类感知滚动停顿阈值（< 100 ms 视为连续滚动；> 100 ms 视为停顿）。

#### Scenario: 快速滚动期间 leading 立即触发 + trailing 合并

- **WHEN** 用户连续滚动触发 `maybeLoadMoreSessions` 调 `store.loadMore("A")` 共 5 次，每次间隔 20 ms（总 100 ms 内）
- **THEN** store SHALL 在第 1 次调用立即 fire 1 次 IPC/fetch（leading）；第 2-5 次调用 SHALL 因 inflight short-circuit 或 trailing 合并而**不**产生新 fetch
- **AND** 若第 1 次 fetch 在第 5 次调用前已 resolve（不再 inflight），trailing timer 触发时 SHALL 重判 inflight short-circuit 仍未占用、才发 1 次 trailing fetch

#### Scenario: 慢速滚动每 110 ms 触发但 inflight 短路

- **WHEN** 用户慢速滚动触发 `store.loadMore("A")` 每 110 ms 一次，连续 5 次（总 550 ms）
- **AND** 每次 fetch 实际耗时 200 ms（远大于 110 ms 间隔，请求持续 inflight）
- **THEN** 第 1 次调用 leading fire；第 2-5 次调用 SHALL 因 inflight short-circuit 全部丢弃（同 cursor 已在飞）
- **AND** 总 IPC/fetch 次数 SHALL ≤ 2（leading 1 次 + 可能的 trailing 1 次）

#### Scenario: 单次滚动后停顿 100 ms 不重复 fire

- **WHEN** 用户滚动触发一次 `loadMore("A")` leading 立即 fire，fetch 200 ms 后 resolve；其后用户停止滚动
- **THEN** store SHALL NOT 在 cooldown 结束时再次触发 fetch（无 pending trailing timer）

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
