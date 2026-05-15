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
