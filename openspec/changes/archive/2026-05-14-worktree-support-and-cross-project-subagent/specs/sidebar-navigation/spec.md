## ADDED Requirements

### Requirement: 默认渲染按仓库聚合的 Sidebar

Sidebar SHALL 默认调用 `list_repository_groups()` IPC 拉取按 git 仓库聚合的项目列表，把同一仓库的多个 worktree 折叠为单个可展开行。`RepositoryGroup` 渲染为顶层条目，含仓库名（`group.name`）与 worktree 数量徽章；单成员 group（只含一个 worktree）SHALL 直接平铺渲染为单行，跳过折叠/展开交互（对齐原版 SidebarHeader.tsx 的"无可分组时降级"行为）。

折叠/展开状态 SHALL 在 `sidebarStore.svelte.ts` 内的 `expandedGroupIds: Set<string>` 维护，仅活跃会话生效，**不**做跨会话持久化（与现有 Pin/Hide 状态同模式）。

#### Scenario: 多 worktree group 折叠
- **WHEN** Sidebar 拉到一个 group 含 2 个 worktree（main + 附加）
- **THEN** 默认 SHALL 渲染为一行可展开条目，展开 chevron 朝右
- **AND** 条目右侧 SHALL 显示 worktree 数量徽章（如 `2`）

#### Scenario: 单 worktree group 直接平铺
- **WHEN** Sidebar 拉到一个 group 只含 1 个 worktree（standalone project）
- **THEN** 默认 SHALL 直接渲染为单行，**不**显示 chevron / 数量徽章
- **AND** 点击该行 SHALL 直接选中该 worktree 进入 session 列表

#### Scenario: 展开 group 显示 worktree 子项
- **WHEN** 用户点击多 worktree group 的展开 chevron
- **THEN** SHALL 在该行下方展开 worktree 子列表，每项含 git branch 名（`git_branch`）与最近活动时间
- **AND** main worktree SHALL 排在第一位（已在后端 `WorktreeGrouper` 排序）

### Requirement: 活跃 worktree 选中状态

Sidebar SHALL 维护"当前选中的 worktree"为单一来源真值——按 design.md D7b 决策，不引入新 `activeWorktreeId` state，沿用既有 `App.selectedProjectId`：每个 `Worktree.id` 与底层 `Project.id` 一一对应（见 `cdt-core::Worktree::id`），worktree 子项点击时把 `worktree.id` 作为 project id 注入既有路径。SessionList SHALL 通过 `list_sessions(worktree.id, pagination)` 拉取该 worktree 自身的 sessions（不是 group 级合并）。

后端 `get_worktree_sessions(group_id, pagination)` IPC SHALL 保留作为可选的"group 级合并 sessions"入口，供未来"group 概览页"等场景按需调用；本 Requirement 不要求 sidebar 默认调它。

`selectedProjectId` 不跨会话持久化（仅本次会话状态），刷新后默认选中"最近活动 group 的 main worktree"。

#### Scenario: 切换 worktree 重新拉 sessions
- **WHEN** 用户在展开的 group 内点击非当前 worktree 的子项
- **THEN** `App.selectedProjectId` SHALL 更新为该 worktree 的 id（即对应底层 `Project.id`）
- **AND** SHALL 触发 `list_sessions(worktree.id, pagination)` 拉取该 worktree 自身的 sessions（按 mtime 倒序）
- **AND** SessionList SHALL 渲染该 worktree 的 sessions

#### Scenario: 默认选中最近活动 group 的 main worktree
- **WHEN** 应用启动，`list_repository_groups()` 返回多个 group
- **THEN** `App.selectedProjectId` 初始值 SHALL 为最近活动 group 内 `is_main_worktree=true` 的 worktree id
- **AND** 该 group SHALL 在用户首次打开 SidebarHeader dropdown 时自动展开（若是多 worktree group）

#### Scenario: get_worktree_sessions 仍暴露给未来 group 概览页
- **WHEN** 调用方需要拿到一个 group 内所有 worktree 合并后的 sessions（按 mtime 倒序，跨 worktree）
- **THEN** 后端 `get_worktree_sessions(group_id, pagination)` IPC SHALL 返回 `PaginatedResponse<SessionSummary>`，每条 SessionSummary 含 `worktreeId` / `worktreeName` 字段供调用方按 worktree 过滤
- **AND** 当前 sidebar 默认 SessionList 路径 SHALL NOT 强制走该 IPC（保持单 worktree 视图与既有 list_sessions 调用语义）

### Requirement: 移除 flat 视图 toggle

Sidebar UI SHALL 不暴露 flat / grouped 视图切换控件（对齐 design.md D4 决策——默认且唯一 grouped 视图）。原版 SidebarHeader 的 `viewMode` toggle 在本 port 内**不**实现。

Sidebar SHALL 在 `listRepositoryGroups()` IPC 失败 / 返回空数组时自动 fallback 到 `listProjects()` 平铺渲染（保证后端跑老版本或 grouper 异常时仍可用）。该 fallback SHALL 由 `repositoryGroups.length > 0` 派生条件控制，不引入额外 URL 参数 / config 字段——按 design.md D4b 决策，不引入 dev-only `?mode=flat` URL gate（vite dev / production Tauri 行为统一，简化状态机）。

#### Scenario: 后端 listRepositoryGroups 失败时 fallback 到 listProjects
- **WHEN** `listRepositoryGroups()` 抛错或返回空
- **THEN** Sidebar SHALL 回落到调 `listProjects()` 拿扁平 ProjectInfo 列表
- **AND** SidebarHeader dropdown SHALL 渲染为单层 flat 列表（无折叠 / chevron / worktree 子项）

#### Scenario: 单成员 group 平铺，无 chevron
- **WHEN** 渲染一个 `worktrees.length === 1` 的 RepositoryGroup
- **THEN** SHALL 直接渲染为单行 dropdown-item（无 `.dropdown-group-row`、无 chevron、无 worktree 数量徽章）
- **AND** 点击该行 SHALL 直接选中该 worktree

### Requirement: Worktree 子项展示元信息

每个 Worktree 子项 SHALL 在 Sidebar 内显示：worktree 名（`worktree.name`）、git branch（`worktree.git_branch`，缺失时省略）、最近活动时间（相对时间，对齐 SessionSummary 已有格式化）、session 数量徽章（`worktree.sessions.length`）。

#### Scenario: 子项含 git branch 标签
- **WHEN** worktree.git_branch 存在
- **THEN** 子项右侧 SHALL 显示 branch icon + branch 名（如 `feat/sidebar-click-replace`）

#### Scenario: 子项无 git branch 时省略 branch 标签
- **WHEN** worktree.git_branch 为 None
- **THEN** 子项 SHALL NOT 显示 branch 标签，其它字段保留
