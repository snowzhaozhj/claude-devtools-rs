## ADDED Requirements

### Requirement: 骨架列表快速加载

Sidebar 切换项目或初次加载时 SHALL 在骨架数据（`sessionId` / `projectId` / `timestamp`）到达后立即渲染完整列表骨架；元数据字段（`title` / `messageCount` / `isOngoing`）pending 时 SHALL 使用占位回退（fallback 到 sessionId 前 8 位 + "…" / `C` 空计数 / 无 ongoing 圆点）。元数据 pending 期间 SHALL NOT 显示"加载中..."遮罩，以避免阻挡已可交互的会话项。

#### Scenario: 骨架到达后立即渲染列表

- **WHEN** `listSessions(projectId)` 返回的 `SessionSummary[]` 已填充 `sessionId` / `timestamp` 但 `title` 为 null、`messageCount` 为 0、`isOngoing` 为 false
- **THEN** Sidebar SHALL 立即渲染会话项（按 timestamp 分组），每项标题 SHALL fallback 到 `sessionId` 前 8 位加 "…"，元数据行 SHALL 显示 `C`（无计数）+ 相对时间

#### Scenario: 骨架态不显示加载中遮罩

- **WHEN** 骨架数据已返回但元数据 patch 尚未全部到达
- **THEN** 会话列表区域 SHALL NOT 显示 "加载中..." 文字；列表 SHALL 展示已有骨架

#### Scenario: 仅骨架未返回时才显示加载中

- **WHEN** 切换项目后 `listSessions` 首次调用尚未 resolve（骨架未到）
- **THEN** 会话列表区域 SHALL 显示 "加载中..." 文字，直至骨架返回

### Requirement: 会话元数据增量 patch

Sidebar SHALL 订阅后端 `session-metadata-update` 事件，按 `sessionId` 定位列表中的 `SessionSummary` 并 in-place 替换其 `title` / `messageCount` / `isOngoing` 字段。patch 操作 SHALL 保持 `{#each}` 的稳定 key（`sessionId`）不变，避免整行 DOM 重建。非当前 `selectedProjectId` 的 event SHALL 被忽略。

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

### Requirement: 会话列表虚拟滚动承载

Sidebar 会话列表 SHALL 以 windowing 方式渲染：仅渲染视口内及上下 overscan 区间内的列表项。所有列表项（PINNED 分区头 + pinned sessions + 日期分组头 + 各日期分组内的 sessions）SHALL 摊平为单一固定行高的 flat 列表参与同一 windowing 单元。滚动位置变化时 SHALL 不引发会话项 DOM 整块重建（依赖 `{#each}` 稳定 key + 上下 spacer 占位元素）。

#### Scenario: 超出视口的会话项不渲染

- **WHEN** 当前项目有 200 个可见会话，视口高度可容纳 20 项
- **THEN** DOM 内实际渲染的 `session-item` 节点数 SHALL 为 `20 + 2 * overscan`（overscan 固定 5，即 30 个以内），其余位置 SHALL 由占位（spacer）高度填充

#### Scenario: 滚动不重建复用节点

- **WHEN** 用户滚动列表
- **THEN** 已渲染的会话项 DOM 节点 SHALL 被 Svelte `{#each (item.key)}` 复用（同 sessionId 进入视口时重用原节点），不触发 OngoingIndicator 的 `animate-ping` 动画重启

#### Scenario: 分组头滚动出视口时同步裁剪

- **WHEN** 用户向下滚动到某个分组头已离开视口
- **THEN** 该 `date-group-label` 元素 SHALL 与同步滚出的 session 项一起退出 DOM（参与 windowing 而非 sticky 渲染），spacer 高度 SHALL 正确反映被裁剪的总高度

#### Scenario: 高亮项位于视口外不触发自动滚动

- **WHEN** `activeSessionId` 对应的会话项当前不在视口内
- **THEN** Sidebar SHALL NOT 自动滚动到该项；用户滚动到该位置时 SHALL 正确显示高亮样式

#### Scenario: file-change 刷新保持滚动位置

- **WHEN** file-change 触发 `silent=true` 刷新并替换 sessions
- **THEN** 视口滚动位置 SHALL 保持不变（`scrollTop` 不重置）
