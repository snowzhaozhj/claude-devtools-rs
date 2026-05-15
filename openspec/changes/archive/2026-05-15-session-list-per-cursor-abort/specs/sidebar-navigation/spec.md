## MODIFIED Requirements

### Requirement: 会话项展示

每个会话项 SHALL 显示标题和元数据（消息计数、相对时间、git 分支）。标题 SHALL 优先使用后端提供的 title 字段，无 title 时 fallback 到**完整 sessionId**——CSS 的 `text-overflow: ellipsis` 自然截断超出宽度的部分；同时 SHALL 在该元素上设置 HTML `title` 属性（`title || sessionId` 完整值），让用户 hover 时浏览器原生 tooltip 显示完整字符串。**禁止**在 JS 侧再手动 `slice(0, 8) + "…"`——双重截断让用户看到的是"前 8 字符 + …"既不能复制粘贴定位 session、也丢失了 CSS 自然 ellipsis 提供的 hover 全展能力。

消息计数（`SessionSummary.messageCount`）SHALL 等于该 session 文件中**真实 user-chunk 消息**与配对 assistant 消息的总数——后端 `cdt-api/src/ipc/session_metadata.rs::extract_session_metadata` MUST 用对齐原版 `claude-devtools/src/main/types/messages.ts::isParsedUserChunkMessage` 的过滤函数判定 user 消息：`category != User` 或 `is_meta = true` 或 `MessageContent::Blocks` 不含任何 `Text` / `Image` block（即纯 `tool_result`-only 行）SHALL NOT 计入。配对计数规则保持原状：每个 user-chunk 后，紧接的第一个非 synthetic 非 sidechain 的 assistant 消息计 1（与 `awaitingAIGroup` 状态机一致）。

git 分支（`SessionSummary.gitBranch`）SHALL 在每条会话项第二行 meta 末尾以 `· <GitBranch icon> {branch}` chip 形式渲染；`gitBranch` 为 `null` 时 SHALL NOT 渲染该 chip（不留分隔符 `·`、不留空位）。该 chip MUST 跟随 `session-metadata-update` 事件 patch 的 `gitBranch` 即时更新。

#### Scenario: 有标题的会话
- **WHEN** SessionSummary.title 非空
- **THEN** SHALL 显示 title，文本溢出时由 CSS `text-overflow: ellipsis` 自动截断；HTML `title` 属性 SHALL 等于完整 title 让 hover 显示

#### Scenario: 无标题的会话
- **WHEN** SessionSummary.title 为 null
- **THEN** SHALL 显示**完整 sessionId**（CSS ellipsis 截断超出部分）；HTML `title` 属性 SHALL 等于完整 sessionId 让 hover 显示
- **AND** SHALL NOT 显示 "前 8 字符 + …" 形式的 JS 手动截断结果

#### Scenario: 元数据显示
- **WHEN** 会话项渲染，`gitBranch` 为 null
- **THEN** SHALL 显示消息计数（`<MessageSquare icon> {N}` 格式）和相对时间（"刚刚"/"Nm"/"Nh"/"Nd"/日期），中间用 `·` 分隔

#### Scenario: 元数据含 git 分支
- **WHEN** 会话项渲染，`gitBranch = "feat/x"`
- **THEN** SHALL 在 messageCount + 时间之后追加 `· <GitBranch icon> feat/x`

#### Scenario: 消息计数排除 tool_result-only user 行
- **WHEN** session JSONL 含 1 条真实用户输入（`{role:"user", content:"hi"}`）+ 1 条 assistant tool_use + 1 条 user tool_result（`{role:"user", content: [{type:"tool_result", ...}]}`）+ 1 条 assistant 收尾
- **THEN** `extract_session_metadata` 返回的 `messageCount` SHALL 为 `2`（真实 user + 配对 assistant），**不**计入 tool_result-only 行

#### Scenario: 消息计数包含含 text+tool_result 混合 user 行
- **WHEN** user 消息 `MessageContent::Blocks` 同时含 `Text` block 与 `ToolResult` block
- **THEN** SHALL 计入 messageCount（与原版 `isParsedUserChunkMessage` 行为一致，"Must contain text or image blocks"）

#### Scenario: 消息计数包含 image-only user 行
- **WHEN** user 消息 `MessageContent::Blocks` 只含 `Image` block（用户粘贴截图，无文字）
- **THEN** SHALL 计入 messageCount

#### Scenario: 消息计数排除 is_meta=true 的 user 行
- **WHEN** user 消息 `is_meta = true`
- **THEN** SHALL NOT 计入 messageCount

### Requirement: 会话元数据增量 patch

Sidebar SHALL 订阅后端 `session-metadata-update` 事件，按 `sessionId` 定位列表中的 `SessionSummary` 并 in-place 替换其 `title` / `messageCount` / `isOngoing` / `gitBranch` 字段。patch 操作 SHALL 保持 `{#each}` 的稳定 key（`sessionId`）不变，避免整行 DOM 重建。非当前 `selectedProjectId` 的 event SHALL 被忽略。

Sidebar SHALL 维护一个 `pendingMetadataUpdates: Map<sessionId, SessionMetadataUpdate>` 缓冲区——listener 每收到一条 update 都 SHALL 写入该 buffer（按 sessionId 覆盖最新值），**无论**当前 `sessions` 数组是否已包含该 sessionId。`sessions` 数组每次写入（非 silent 加载首页 / silent 刷新 / `loadMoreSessions` 翻页扩展）后 SHALL 立即对新数组应用 buffer 中匹配 sessionId 的 update。这是兜底 broadcast 在 IPC return 之前到达时 `sessions.map` 找不到目标的 race——`broadcast::Sender::send` 在前端 listener 已订阅但 sessions 数组还没扩展到新页时，update 会静默丢失（broadcast 不重发），导致 session 永远卡在 sessionId 占位。

切 project / 首次加载（非 silent 路径）SHALL 在调用 `await listSessions(...)` **之前**清空 `pendingMetadataUpdates`，避免旧 project 的 update 残留；同时这一 clear SHALL 早于 await 阻塞窗口，让 listener 在 `await listSessions(...)` 期间收到的新 project update 能被 buffer 保留并在后续 `applyPendingMetadata` 应用上去——后端 `list_sessions` 在 IPC return 之前已 spawn 扫描任务并可能 broadcast emit，clear 若放在 await 后会把这些"早到的"新 project update 一起清掉。silent 刷新与 loadMore SHALL NOT 清空 buffer（buffer 中已有的 update 仍可能匹配 prev sessions 中尚未 patch 的 sessionId）。

`loadSessions(projectId, silent=true)` 路径（file-change 触发或用户点击"有更新"按钮）SHALL 把第一页结果合并到现有 `sessions` 数组而非整体替换：prev 中超出第一页（cursor 之后）的尾部 sessions SHALL 被保留；prev 中与新第一页 sessionId 相同的条目 SHALL 保留已 patch 元数据（与既有"silent 刷新保留已获取元数据"语义一致）。silent 路径 SHALL NOT 重置 `sessionsNextCursor`，保留用户已翻到的分页位置。

非 silent 路径（用户切 project / 首次加载）行为不变：仍然替换式加载第一页，`sessionsNextCursor` 取本次响应的 `nextCursor`。

#### Scenario: 元数据事件按 sessionId 匹配并 patch

- **WHEN** 当前 `selectedProjectId = "projectA"`，前端收到 payload `{ projectId: "projectA", sessionId: "s1", title: "重构 auth", messageCount: 42, isOngoing: false }`
- **THEN** Sidebar SHALL 找到 `sessions[i].sessionId === "s1"` 的条目，将其 `title` 更新为 "重构 auth"、`messageCount` 更新为 42、`isOngoing` 更新为 false；其他条目 SHALL 不变

#### Scenario: 元数据事件不改变列表顺序或重建 DOM

- **WHEN** 一条 `session-metadata-update` patch 到达
- **THEN** 被 patch 的会话项 SHALL 保持在原位置，DOM 节点 SHALL 被复用（Svelte `{#each}` 的 `(session.sessionId)` key 保障），不触发 OngoingIndicator 动画重启或 pin 图标闪烁

#### Scenario: 非当前 project 的事件被忽略

- **WHEN** 当前 `selectedProjectId = "projectA"`，收到 payload `{ projectId: "projectB", sessionId: "sX", ... }`
- **THEN** Sidebar SHALL NOT 修改本地 `sessions` 状态

#### Scenario: 更新到达时 sessions 还未包含 sessionId 时缓冲到 pending buffer

- **WHEN** Sidebar 已加载 page 1（20 条），用户滚动触发 `loadMoreSessions` 启动 page 2 的 `list_sessions` IPC；后端 page 2 的扫描任务先于 IPC return 完成对 `sessionId = "s_new"`（page 2 尾部一条）的 metadata 扫描并 broadcast emit
- **AND** 前端 listener 收到 `s_new` 的 update 时 `sessions` 数组仍为 page 1 的 20 条（不含 `s_new`）
- **THEN** listener SHALL 把 update 写入 `pendingMetadataUpdates`，且对当前 `sessions` 跑一遍 `map`（无效 patch，因为 sessionId 不在）
- **AND** 当 page 2 IPC return 后 `sessions = mergeSessions(prev, result.items, false)` 写入完成，Sidebar SHALL 立即对新数组应用 buffer 中 `s_new` 的 update，使 `s_new` 立即显示真实 title 而非占位

#### Scenario: 切 project 时在 await 之前清空 pending buffer

- **WHEN** 当前 `selectedProjectId = "projectA"`，`pendingMetadataUpdates` 缓冲了若干 projectA 的 update；用户切到 `selectedProjectId = "projectB"`
- **THEN** `loadSessions("projectB", silent=false)` 进入时 SHALL 在调用 `await listSessions(...)` **之前** `pendingMetadataUpdates.clear()`
- **AND** clear 之后 listener 在 `await listSessions("projectB", ...)` 阻塞期间收到的 projectB update SHALL 被 buffer 保留下来；非 silent 路径的 `applyPendingMetadata(fresh, pendingMetadataUpdates)` 会在 IPC return 后立即应用这些"早到的" update，让 projectB 中后端先扫到的 session 不会卡占位
- **AND** clear 放在 `await listSessions(...)` 之**后**是 bug：会把 await 期间到达的 projectB update 一并清掉，等于绕过 race buffer 修复

#### Scenario: file-change silent 刷新保留已获取元数据

- **WHEN** file-change 触发 `loadSessions(projectId, silent=true)` 并返回新骨架（title/messageCount/isOngoing 全部重置为占位）
- **THEN** Sidebar SHALL 按 `sessionId` 将旧 `sessions` 的元数据字段 merge 进新骨架（旧有值的 session 元数据字段不被重置为占位），直到新的 `session-metadata-update` 到达再覆盖

#### Scenario: silent 刷新保留尾部已翻页 sessions

- **WHEN** 用户已通过 `loadMoreSessions` 翻页加载到 `sessions.length === 60`（首页 20 + 第二页 20 + 第三页 20），随后 file-change 触发 `loadSessions(projectId, silent=true)`，silent 请求只返回第一页的 20 条
- **THEN** silent 刷新完成后 `sessions.length` SHALL ≥ 60（含 prev 中超出第一页的所有 sessionId）；前 20 条按合并后 `timestamp` 倒序，prev 中 sessionId 也出现在新第一页的条目 SHALL 保留 prev 已 patch 的元数据
- **AND** `sessions.length` SHALL NOT 在 silent 刷新后瞬间缩水到 20 余条又被 `maybeLoadMoreSessions` 补回——这是"计数来回跳变"反模式

#### Scenario: silent 刷新不重置分页 cursor

- **WHEN** 用户已翻到第三页（`sessionsNextCursor === cursor3`），silent 刷新返回 `result.nextCursor === cursor1`
- **THEN** silent 完成后 `sessionsNextCursor` SHALL 仍为 `cursor3`，下一次 `loadMoreSessions` 用 `cursor3` 请求未看过的第四页，而非用 `cursor1` 重复请求已加载的第二页

#### Scenario: silent 刷新不丢失任何 prev sessionId

- **WHEN** silent 刷新（含 file-change 触发与"有更新"按钮触发两条入口）合并第一页结果到 prev sessions
- **THEN** 合并后 `sessions` SHALL 包含 prev 中所有 `sessionId`（无论该 sessionId 是否出现在新第一页响应里），保证 prev 已渲染会话项的 `{#each (item.key)}` 节点在 DOM 中被复用、`scrollTop` 锚定的会话项仍可定位
- **AND** 滚动位置不变的视觉约束 SHALL 由本 Scenario（合并不丢条目）联合既有 Scenario "file-change 刷新保持滚动位置"（`scrollTop` 不重置）共同保证；Sidebar SHALL NOT 在 silent 刷新完成后自动 `scrollTo({ top: 0 })`

## ADDED Requirements

### Requirement: 会话总数显示口径

Sidebar 顶部 `session-count-num` 元素显示形如 `{visibleSessions.length}/{totalSessions}`。`totalSessions` SHALL 取自 `listSessions` IPC 响应的 `result.total` 字段（项目维度后端骨架阶段 `read_dir` 统计的全部 session 数），而非已加载到本地的 `sessions.length`（后者会随用户翻页累加 20 → 40 → 60 跳变）。

非 silent 路径（首次加载 / 切 project）的 `loadSessions` SHALL 在 IPC 返回后用 `result.total` 覆盖本地 `sessionsTotal`。silent 路径（file-change 触发或"有更新"按钮触发）SHALL 在合并完成后同样用 `result.total` 覆盖（silent 拿到的也是后端最新全量计数）。`loadMoreSessions` 翻页路径 SHALL **不**覆盖 `sessionsTotal`（页内 total 不应改变；首次加载时已有正确值）。

#### Scenario: 首次加载时 totalSessions 取后端 result.total

- **WHEN** Sidebar 首次加载某 project（项目实际 60 个 session）
- **AND** `listSessions(projectId, 20)` 返回 `{ items: [...20 条...], nextCursor: "20", total: 60 }`
- **THEN** `session-count-num` SHALL 显示 `20/60`，**不**显示 `20/20`

#### Scenario: 翻页后 totalSessions 不随 sessions.length 变化

- **WHEN** 用户已加载 page 1（20 条）；调用 `loadMoreSessions` 加载 page 2（再 20 条）
- **THEN** `sessions.length` 从 20 增至 40；`totalSessions` SHALL 保持 60；`session-count-num` 显示 `40/60`，不再出现 `20 → 40 → 60` 跳变

#### Scenario: silent 刷新时 totalSessions 同步刷新

- **WHEN** silent 刷新（file-change 或"有更新"按钮触发）成功，`result.total` 由 60 变为 61（后端检测到新增 session）
- **THEN** Sidebar SHALL 把 `totalSessions` 更新为 61；不破坏既有 silent 刷新对 sessions 数组合并保留尾部的语义
