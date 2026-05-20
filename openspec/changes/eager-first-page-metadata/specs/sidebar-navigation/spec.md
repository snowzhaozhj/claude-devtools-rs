## MODIFIED Requirements

### Requirement: 骨架列表快速加载

Sidebar 切换项目或初次加载时 `listSessions(projectId, pageSize)` 首页（`cursor=null`）IPC 路径 SHALL 在响应到达后立即渲染**含真值元数据**的列表——`title` / `messageCount` / `isOngoing` / `gitBranch` SHALL 已是真值（与后端 `ipc-data-api` spec §"Expose project and session queries" 中"首页 cursor=None 路径同步等真值"对应）。Sidebar SHALL NOT 在首页路径上显示 sessionId UUID fallback 占位（除非个别条目后端 metadata 解析失败降级为占位——这是预期降级路径，仍可继承 `.metadata-pending` shimmer 视觉提示）。

翻页路径（`cursor=Some`）IPC 响应仍可能含骨架占位条目；Sidebar 翻页加载时 SHALL 沿用既有"骨架立即渲染 + 后续 `session-metadata-update` patch"语义，详见本 spec `会话元数据增量 patch` Requirement。

非骨架尚未返回（`listSessions` IPC 仍 pending）期间 SHALL 显示 `SkeletonList` 占位（既有行为）；首页 IPC return 时一次性切换到含真值列表，**无两阶段渲染**——避免"sessionId 长 UUID → 真实 title"突变的视觉跳变。

#### Scenario: 首页 cursor=null 列表立即渲染真值

- **WHEN** Sidebar 切换项目触发 `listSessions(projectId, pageSize=20)` 首次调用，`cursor=null`，IPC return 后响应 `SessionSummary[]` 每条已含真值 `title` / `messageCount` / `isOngoing` / `gitBranch`
- **THEN** Sidebar SHALL 立即渲染会话项（按 timestamp 分组），每项标题 SHALL 直接显示真实 `title` 字符串（**非** `sessionId` UUID fallback）
- **AND** 元数据行（msgCount / time / branch）SHALL 直接显示真实数据（`messageCount` 数字 / `gitBranch` 名）

#### Scenario: 首页路径条目 metadata 解析失败降级到占位

- **WHEN** 首页 IPC 响应中某条 `SessionSummary` 因 jsonl 损坏 / IO 错误 / 单条 timeout 等导致后端 metadata 解析未能完成，字段为占位值（`title=null` / `messageCount=0` / `isOngoing=false` / `gitBranch=null`）
- **THEN** Sidebar SHALL 把该条标题 fallback 到完整 `sessionId` 字符串（依赖 CSS `text-overflow: ellipsis` 截断显示），与主 spec `会话项展示` Requirement 现行行为一致；同时触发 `.metadata-pending` shimmer，与翻页路径上骨架条目视觉一致；其它真值条目正常显示
- **AND** 整页加载 SHALL NOT 失败（该条降级不阻塞其它条目渲染）

#### Scenario: 翻页路径仍走骨架立即渲染

- **WHEN** 用户滚动触发 `loadMoreSessions` 调用 `listSessions(projectId, pageSize=20, cursor=C)`，IPC return 响应 `SessionSummary[]` 中 `title` 为 null、`messageCount` 为 0、`isOngoing` 为 false 的骨架条目
- **THEN** Sidebar SHALL 立即把骨架条目追加到列表（按 timestamp 倒序合并），每项标题 SHALL fallback 到完整 `sessionId` 字符串（依赖 CSS `text-overflow: ellipsis` 截断），元数据行 SHALL 显示空计数 + 相对时间，并触发 `.metadata-pending` shimmer 等待 `session-metadata-update` patch

#### Scenario: 骨架态不显示加载中遮罩

- **WHEN** 翻页路径骨架数据已返回但元数据 patch 尚未全部到达
- **THEN** 会话列表区域 SHALL NOT 显示 "加载中..." 文字；列表 SHALL 展示已有骨架

#### Scenario: 仅骨架未返回时才显示加载中

- **WHEN** 切换项目后 `listSessions` 首页调用尚未 resolve（IPC 仍 pending，包含首页同步等待 metadata 解析的窗口）
- **THEN** 会话列表区域 SHALL 显示 `SkeletonList` 占位（既有行为不变），直至 IPC return

### Requirement: 会话元数据增量 patch

Sidebar SHALL 订阅后端 `session-metadata-update` 事件，按 `sessionId` 定位列表中的 `SessionSummary` 并 in-place 替换其 `title` / `messageCount` / `isOngoing` / `gitBranch` 字段。patch 操作 SHALL 保持 `{#each}` 的稳定 key（`sessionId`）不变，避免整行 DOM 重建。非当前 `selectedProjectId` 的 event SHALL 被忽略。

Sidebar SHALL 维护一个 `pendingMetadataUpdates: Map<sessionId, SessionMetadataUpdate>` 缓冲区——listener 每收到一条 update 都 SHALL 写入该 buffer（按 sessionId 覆盖最新值），**无论**当前 `sessions` 数组是否已包含该 sessionId。`sessions` 数组每次写入（非 silent 加载首页 / silent 刷新 / `loadMoreSessions` 翻页扩展）后 SHALL 立即对新数组应用 buffer 中匹配 sessionId 的 update。这是兜底 broadcast 在 IPC return 之前到达时 `sessions.map` 找不到目标的 race——`broadcast::Sender::send` 在前端 listener 已订阅但 sessions 数组还没扩展到新页时，update 会静默丢失（broadcast 不重发），导致 session 永远卡在 sessionId 占位。eager 首页路径下 buffer 主要承载**翻页 race**与 **eager deferred retry**（首页 timeout / 失败条 spawn 的单条 retry）的兜底。

切 project / 首次加载（非 silent 路径）SHALL 在调用 `await listSessions(...)` **之前**清空 `pendingMetadataUpdates`，避免旧 project 的 update 残留；同时这一 clear SHALL 早于 await 阻塞窗口，让 listener 在 `await listSessions(...)` 期间收到的新 project update 能被 buffer 保留并在后续 `applyPendingMetadata` 应用上去——后端 `list_sessions` eager 路径下首页 IPC return 时 items 已含真值（buffer 中的 update 与 response 对同一 sessionId 都是真值，apply buffer 是幂等 no-op）；翻页路径上 spawn 扫描任务可能 broadcast emit，clear 若放在 await 后会把这些"早到的"新 project update 一起清掉。silent 刷新与 loadMore SHALL NOT 清空 buffer（buffer 中已有的 update 仍可能匹配 prev sessions 中尚未 patch 的 sessionId）。

`loadSessions(projectId, silent=true)` 路径（file-change 触发或用户点击"有更新"按钮）走的是 `listSessions(projectId, SESSION_PAGE_SIZE)` 即 `cursor=None` 首页 eager 路径——response items 已含**真值** metadata。Sidebar SHALL 把第一页结果合并到现有 `sessions` 数组而非整体替换，且合并语义 SHALL 为 **response 真值覆盖 prev stale**（`mergeRecoveryResponse` 语义，PR #177 codex round 5 引入）：

- prev 中超出第一页（cursor 之后）的尾部 sessions SHALL 被保留
- prev 中与新第一页 sessionId 相同的条目，**若 new entry 含真值**（任一 metadata 字段非占位）SHALL 用 new entry 覆盖；**若 new entry 是占位**（仅在 deferred retry 仍失败的极端场景）则保留 prev 已 patch 的真值

silent 路径 SHALL NOT 重置 `sessionsNextCursor`，保留用户已翻到的分页位置。

`applySilentRefresh` 实现 SHALL 调 `mergeRecoveryResponse(prev, firstPage)` 而**不是**原 `mergeSilentMetadata`（"prev 旧元数据优先"语义）——后者在 eager 路径下会让 silent refresh 后已 patch 真值被 prev stale 永久压住。

非 silent 路径（用户切 project / 首次加载）行为不变：仍然替换式加载第一页（response 已含真值），`sessionsNextCursor` 取本次响应的 `nextCursor`。

#### Scenario: 元数据事件按 sessionId 匹配并 patch

- **WHEN** 当前 `selectedProjectId = "projectA"`，前端收到 payload `{ projectId: "projectA", sessionId: "s1", title: "重构 auth", messageCount: 42, isOngoing: false }`
- **THEN** Sidebar SHALL 找到 `sessions[i].sessionId === "s1"` 的条目，将其 `title` 更新为 "重构 auth"、`messageCount` 更新为 42、`isOngoing` 更新为 false；其他条目 SHALL 不变

#### Scenario: 元数据事件不改变列表顺序或重建 DOM

- **WHEN** 一条 `session-metadata-update` patch 到达
- **THEN** 被 patch 的会话项 SHALL 保持在原位置，DOM 节点 SHALL 被复用（Svelte `{#each}` 的 `(session.sessionId)` key 保障），不触发 OngoingIndicator 动画重启或 pin 图标闪烁

#### Scenario: 非当前 project 的事件被忽略

- **WHEN** 当前 `selectedProjectId = "projectA"`，收到 payload `{ projectId: "projectB", sessionId: "sX", ... }`
- **THEN** Sidebar SHALL NOT 修改本地 `sessions` 状态

#### Scenario: 翻页路径更新到达时 sessions 还未包含 sessionId 时缓冲到 pending buffer

- **WHEN** Sidebar 已加载 page 1（20 条），用户滚动触发 `loadMoreSessions` 启动 page 2 的 `list_sessions` IPC（**翻页路径** cursor=Some）；后端 page 2 的扫描任务先于 IPC return 完成对 `sessionId = "s_new"`（page 2 尾部一条）的 metadata 扫描并 broadcast emit
- **AND** 前端 listener 收到 `s_new` 的 update 时 `sessions` 数组仍为 page 1 的 20 条（不含 `s_new`）
- **THEN** listener SHALL 把 update 写入 `pendingMetadataUpdates`，且对当前 `sessions` 跑一遍 `map`（无效 patch，因为 sessionId 不在）
- **AND** 当 page 2 IPC return 后 `sessions = mergeSessions(prev, result.items, false)` 写入完成，Sidebar SHALL 立即对新数组应用 buffer 中 `s_new` 的 update，使 `s_new` 立即显示真实 title 而非占位

#### Scenario: 切 project 时在 await 之前清空 pending buffer

- **WHEN** 当前 `selectedProjectId = "projectA"`，`pendingMetadataUpdates` 缓冲了若干 projectA 的 update；用户切到 `selectedProjectId = "projectB"`
- **THEN** `loadSessions("projectB", silent=false)` 进入时 SHALL 在调用 `await listSessions(...)` **之前** `pendingMetadataUpdates.clear()`
- **AND** clear 之后 listener 在 `await listSessions("projectB", ...)` 阻塞期间收到的 projectB update SHALL 被 buffer 保留下来；非 silent 路径的 `applyPendingMetadata(fresh, pendingMetadataUpdates)` 会在 IPC return 后立即应用这些"早到的" update（首页 eager 路径下 response 已含真值，buffer apply 通常是幂等 no-op）

#### Scenario: file-change silent 刷新走 eager 让 response 真值覆盖 prev stale

- **WHEN** file-change 触发 `loadSessions(projectId, silent=true)` 调用 `listSessions(projectId, SESSION_PAGE_SIZE)`（cursor=None 首页 eager 路径）；response items 含真实最新 metadata（如 jsonl 已被追加新消息，`messageCount` / `gitBranch` 等已更新）
- **THEN** Sidebar SHALL 调 `mergeRecoveryResponse(prev, firstPage)` 而**不是** `mergeSilentMetadata`：response 中 sessionId 相同的条目用 response **真值覆盖** prev 同 sessionId 的旧元数据；prev 中超出第一页的尾部 sessions 保留
- **AND** silent 刷新完成后 `sessions` 数组中已 patch 真值 SHALL 与后端最新 jsonl 状态一致（不再被 prev stale 压住）

#### Scenario: silent 刷新保留尾部已翻页 sessions

- **WHEN** 用户已通过 `loadMoreSessions` 翻页加载到 `sessions.length === 60`（首页 20 + 第二页 20 + 第三页 20），随后 file-change 触发 `loadSessions(projectId, silent=true)`，silent 请求只返回第一页的 20 条（含真值）
- **THEN** silent 刷新完成后 `sessions.length` SHALL ≥ 60（含 prev 中超出第一页的所有 sessionId）；前 20 条按合并后 `timestamp` 倒序，response 真值覆盖 prev 同 sessionId
- **AND** `sessions.length` SHALL NOT 在 silent 刷新后瞬间缩水到 20 余条又被 `maybeLoadMoreSessions` 补回——这是"计数来回跳变"反模式

#### Scenario: silent 刷新不重置分页 cursor

- **WHEN** 用户已翻到第三页（`sessionsNextCursor === cursor3`），silent 刷新返回 `result.nextCursor === cursor1`
- **THEN** silent 完成后 `sessionsNextCursor` SHALL 仍为 `cursor3`，下一次 `loadMoreSessions` 用 `cursor3` 请求未看过的第四页，而非用 `cursor1` 重复请求已加载的第二页

#### Scenario: silent 刷新不丢失任何 prev sessionId

- **WHEN** silent 刷新（含 file-change 触发与"有更新"按钮触发两条入口）合并第一页结果到 prev sessions
- **THEN** 合并后 `sessions` SHALL 包含 prev 中所有 `sessionId`（无论该 sessionId 是否出现在新第一页响应里），保证 prev 已渲染会话项的 `{#each (item.key)}` 节点在 DOM 中被复用、`scrollTop` 锚定的会话项仍可定位
- **AND** 滚动位置不变的视觉约束 SHALL 由本 Scenario（合并不丢条目）联合既有 Scenario "file-change 刷新保持滚动位置"（`scrollTop` 不重置）共同保证；Sidebar SHALL NOT 在 silent 刷新完成后自动 `scrollTo({ top: 0 })`
