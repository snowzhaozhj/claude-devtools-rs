## MODIFIED Requirements

### Requirement: 会话元数据增量 patch

Sidebar SHALL 订阅后端 `session-metadata-update` 事件，按 `sessionId` 定位列表中的 `SessionSummary` 并 in-place 替换其 `title` / `messageCount` / `isOngoing` 字段。patch 操作 SHALL 保持 `{#each}` 的稳定 key（`sessionId`）不变，避免整行 DOM 重建。非当前 `selectedProjectId` 的 event SHALL 被忽略。

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
