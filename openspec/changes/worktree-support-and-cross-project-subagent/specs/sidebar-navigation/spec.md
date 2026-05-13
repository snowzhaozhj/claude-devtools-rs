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

Sidebar SHALL 维护"当前选中的 worktree"为单一来源真值（`sidebarStore.activeWorktreeId`），并据此决定渲染哪个 worktree 的 sessions 列表。切换 worktree SHALL 触发 `getWorktreeSessions(group_id, pagination)` IPC（不是 `list_sessions(project_id)`——后者按 project_id 拉，跨 worktree 不合并），把该 worktree 的 sessions 注入 SessionList。

`activeWorktreeId` 不持久化（仅本次会话），刷新后默认选中"最近活动 group 的 main worktree"。

#### Scenario: 切换 worktree 重新拉 sessions
- **WHEN** 用户在展开的 group 内点击非当前 worktree 的子项
- **THEN** `sidebarStore.activeWorktreeId` SHALL 更新为新 worktree id
- **AND** SHALL 触发 `getWorktreeSessions(group_id, pagination)` 拉取整 group 合并的 sessions（按 mtime 倒序）
- **AND** SessionList SHALL 高亮该 worktree 归属的 sessions（通过 SessionSummary.worktreeId 字段过滤展示）

#### Scenario: 默认选中最近活动 group 的 main worktree
- **WHEN** 应用启动，`list_repository_groups()` 返回多个 group
- **THEN** `sidebarStore.activeWorktreeId` 初始值 SHALL 为最近活动 group 内 `is_main_worktree=true` 的 worktree id
- **AND** 该 group SHALL 默认展开（若是多 worktree group）

### Requirement: 移除 flat 视图 toggle

Sidebar UI SHALL 不暴露 flat / grouped 视图切换控件（对齐 design.md D4 决策——默认且唯一 grouped 视图）。原版 SidebarHeader 的 `viewMode` toggle 在本 port 内**不**实现。

URL 参数 `?fixture=<name>&mode=flat` SHALL 仅在 dev 模式（`import.meta.env.DEV=true`）下生效，用于 e2e 测试 fixture 走 flat 渲染；production bundle 通过 vite DCE 完全消除该路径（验证靠 `tauriMock.bundle.test.ts`）。

#### Scenario: production bundle 不含 viewMode toggle
- **WHEN** `NODE_ENV=production npm run build --prefix ui` 生成 bundle
- **THEN** `ui/dist/` 内 SHALL NOT 含 `viewMode` 状态机相关代码
- **AND** Sidebar 渲染入口 SHALL 直接走 grouped 分支

#### Scenario: dev 模式 URL 参数 fallback 到 flat
- **WHEN** 开发者访问 `http://localhost:5173/?mock=1&fixture=multi-project-rich&mode=flat`
- **THEN** Sidebar SHALL 渲染为 flat 列表（按 `Project.id` 平铺，不分组）
- **AND** 该 fallback **仅**在 `import.meta.env.DEV=true` 时启用

### Requirement: Worktree 子项展示元信息

每个 Worktree 子项 SHALL 在 Sidebar 内显示：worktree 名（`worktree.name`）、git branch（`worktree.git_branch`，缺失时省略）、最近活动时间（相对时间，对齐 SessionSummary 已有格式化）、session 数量徽章（`worktree.sessions.length`）。

#### Scenario: 子项含 git branch 标签
- **WHEN** worktree.git_branch 存在
- **THEN** 子项右侧 SHALL 显示 branch icon + branch 名（如 `feat/sidebar-click-replace`）

#### Scenario: 子项无 git branch 时省略 branch 标签
- **WHEN** worktree.git_branch 为 None
- **THEN** 子项 SHALL NOT 显示 branch 标签，其它字段保留
