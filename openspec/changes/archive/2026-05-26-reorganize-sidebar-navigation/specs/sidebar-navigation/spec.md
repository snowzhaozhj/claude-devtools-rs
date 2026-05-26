## REMOVED Requirements

### Requirement: 会话选择与 Tab 联动

**Reason**：该 Requirement 的行为归属 Tab 生命周期 / Sidebar 与 Tab 联动 owner，迁入或归并到 `tab-management`，避免 Sidebar 与 Tab 双 owner。

**Migration**：行为契约 100% 不变；其中 2 个 Scenario 迁入 `tab-management` 的 `打开 session tab` 与 `Sidebar 与 Tab 联动` Requirement；`无 active tab 时无高亮` 由 `tab-management` 既有 `无 active tab 时 Sidebar 无高亮` Scenario 覆盖。同一 change 的另 1 个迁入 Scenario 来自 `selectedGroupId 与 worktree id 分层维护`。

## MODIFIED Requirements

### Requirement: selectedGroupId 与 worktree id 分层维护

Sidebar 当前选中的项目入口 SHALL 用 `selectedGroupId` 字段持 `RepositoryGroup.id`，用于顶层导航 / 列表分页 / push event 过滤 / 用户配置持久化。Session tab identity 与详情 API 入参归 `[[tab-management]]` owner；Sidebar 仅消费会话列表项携带的 worktree / group / session 三元信息来发起打开行为，不在本 Requirement 重复定义 tab 字段。

Sidebar 通过 `SessionSummary.worktreeId` + `SessionSummary.groupId` + `SessionSummary.sessionId` 三元组把 group 级列表项与具体 worktree session 关联：sidebar 顶层 `selectedGroupId` 保持 group id；涉及 tab 创建、tab 高亮归属与详情加载 identity 的行为由 `[[tab-management]]` 对应 Requirement 守护。

收敛点行为契约：

- 顶层导航状态 SHALL 持 group id（不再是 worktree id）
- session 列表分页 SHALL 调 `listGroupSessions(groupId, pageSize, cursor)` 拉合并 sessions
- session 列表缓存 cache key SHALL 用 `(groupId, filterWorktreeId | null)` 复合 key 区分 filter 维度，否则切 filter 串台
- push event `session-metadata-update` SHALL 按 `[[push-events]]` 定义的 group id 语义过滤：前端 filter 按 event 所属 group 等于 `selectedGroupId` 匹配；worktree id 字段仅供 `[[tab-management]]` / detail 路径消费
- 后台任务 `active_scans` per-key cancel 分两类 key：detail 拉取 = `(project_id /*worktree id*/, session_id)`（不变）；group 分页拉取 = `(group_id, page_cursor_hash)`（新加）
- session 打开后的 tab identity 与详情 API 入参 SHALL 由 `[[tab-management]]` 守护；Sidebar 不重复定义 tab 字段
- Command Palette 全局搜索 SHALL 改调 `listGroupSessions(selectedGroupId, pageSize, null)` 拿合并候选；候选项 onclick 时 SHALL 按 `candidate.worktreeId` 交给 `[[tab-management]]` 创建 tab
- 用户配置 `selected_project_id` 改 `selected_group_id`；启动时若读到老 worktree id，按 grouper 反查 group id 后改写一次（迁移）
- 项目 memory / prefs（如有 per-project state）SHALL **不变**（维持 per-worktree，与 detail API 一致）

单 worktree group 时 group id 与 worktree id 字符串相同（grouper 在 standalone project 场景下 `group.id = project.id`），单 worktree 项目用户无感知 ID 变化。

#### Scenario: push event 按 groupId filter
- **WHEN** 用户当前 `selectedGroupId = "group-X"`，push 流推送 `session-metadata-update` event 含 `groupId: "group-Y"` + `projectId: "wt-Z"`
- **THEN** 前端 SHALL 丢弃该 event（不 patch 到当前列表）

#### Scenario: push event 同 group 命中
- **WHEN** 用户当前 `selectedGroupId = "group-X"`，event 含 `groupId: "group-X"` + `projectId: "wt-X1"` + sessionId
- **THEN** 前端 SHALL 在列表中找到对应 session 并 patch metadata

#### Scenario: CommandPalette 全局搜索走 group 维度
- **WHEN** 用户在 Command Palette 输入查询词，当前 `selectedGroupId = "group-X"`
- **THEN** Command Palette SHALL 调 `listGroupSessions("group-X", pageSize: 200, cursor: null)` 拿合并候选（覆盖所有 worktree 的 session）
- **AND** 候选项 onclick 时 SHALL 按 `candidate.worktreeId` 创建 tab，tab 内 detail 路径走 worktree id

#### Scenario: 列表缓存 cache key 含 worktree filter
- **WHEN** 用户在 group-X 内切 filter 从 "全部" → "wt-X1" → "全部"
- **THEN** session 列表缓存 SHALL 用 `(groupId, filterWorktreeId | null)` 作为 cache key 区分三个状态的缓存
- **AND** 切回"全部"时 SHALL 命中第一次"全部"的缓存，不重发 IPC

#### Scenario: 单 worktree group group id 等于 worktree id
- **WHEN** standalone project 转化的 RepositoryGroup
- **THEN** `group.id` SHALL 等于 `group.worktrees[0].id`（即 encoded project dir 名）
- **AND** 单 worktree 项目用户配置 `selected_project_id` 在迁移前后字符串相同，无需写迁移
