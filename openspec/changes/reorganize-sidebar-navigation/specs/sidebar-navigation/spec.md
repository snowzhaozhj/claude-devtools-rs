## REMOVED Requirements

### Requirement: 会话选择与 Tab 联动

**Reason**：该 Requirement 的全部 3 个 Scenario 归属 Tab 生命周期 / Sidebar 与 Tab 联动 owner，迁入 `tab-management`，避免 Sidebar 与 Tab 双 owner。

**Migration**：行为契约 100% 不变；3 个 Scenario 的 WHEN / THEN 子句字符级迁移到 `tab-management` 的 `打开 session tab` 与 `Sidebar 与 Tab 联动` Requirement。

## MODIFIED Requirements

### Requirement: selectedGroupId 与 worktree id 分层维护

Sidebar 当前选中的项目入口 SHALL 用 `selectedGroupId` 字段持 `RepositoryGroup.id`，用于顶层导航 / 列表分页 / push event 过滤 / 用户配置持久化。**但** session 详情链路（`get_session_detail` / `get_tool_output` / `get_image_asset` / `get_subagent_trace`）入参的 project id MUST 继续持 worktree id（即底层 `Project.id`，encoded project dir 名），避免 detail API 无法定位具体 session 文件（codex post-propose 二审 #3 驳回一刀切方案）。

两套 id 通过 `SessionSummary.worktreeId` + `SessionSummary.groupId` + `SessionSummary.sessionId` 三元组桥接：UI 列表点击 session 时拿 `session.worktreeId` 注入 tab，tab 内 detail 路径用 worktree id 走老 IPC，sidebar 顶层 `selectedGroupId` 不变。

收敛点行为契约：

- 顶层导航状态 SHALL 持 group id（不再是 worktree id）
- session 列表分页 SHALL 调 `listGroupSessions(groupId, pageSize, cursor)` 拉合并 sessions
- session 列表缓存 cache key SHALL 用 `(groupId, filterWorktreeId | null)` 复合 key 区分 filter 维度，否则切 filter 串台
- push event payload `session-metadata-update` SHALL **新增** `groupId` 字段；前端 filter 按 `payload.groupId === selectedGroupId` 匹配；保留 `projectId` 字段供 detail 路径用
- 后台任务 `active_scans` per-key cancel 分两类 key：detail 拉取 = `(project_id /*worktree id*/, session_id)`（不变）；group 分页拉取 = `(group_id, page_cursor_hash)`（新加）
- tab 状态 SHALL **保留** worktree id（`tab.projectId`，detail API 仍按 worktree id 定位）；新增 `tab.groupId: string` 字段供 sidebar 高亮"该 tab 属于哪个 group"
- detail API 调用 SHALL 仍传 worktree id（不变）
- Command Palette 全局搜索 SHALL 改调 `listGroupSessions(selectedGroupId, pageSize, null)` 拿合并候选；候选项 onclick 时按 `candidate.worktreeId` 创建 tab
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
