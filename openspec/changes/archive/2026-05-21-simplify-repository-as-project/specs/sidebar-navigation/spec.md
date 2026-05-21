## ADDED Requirements

### Requirement: Worktree filter dropdown for multi-worktree group

Sidebar SHALL 在顶部（与会话搜索框同一行）渲染 worktree filter 下拉，**仅当**当前选中的 RepositoryGroup `worktrees.length > 1` 时可见；单 worktree group 或退化的 flat fallback 模式下 SHALL 隐藏该控件。

filter options SHALL 按以下顺序排列：
1. "全部"（默认 selected）
2. group 内 `isRepoRoot=true` 的 worktree（repo 根）
3. 其它 worktree 按 `is_main_worktree` 优先 + `most_recent_session` 倒序

每个 option SHALL 显示：worktree 名（`is_repo_root` 时 fallback 为 group.name；否则 `worktree.name`）+ git branch chip（带 `GIT_BRANCH_SVG` icon，`git_branch` 为 None 时省略）+ cwd 相对路径 hint（`cwd_relative_to_repo_root` 非 None 时，否则省略）+ session count 徽章。

filter 切换 SHALL 重置当前 group 的 session 列表分页状态（清空已加载页 + cursor 重置），重新调 `list_group_sessions`。

filter 实现 SHALL 走 **server-side filter via cursor `Exhausted`**：前端构造初始 cursor 让所有非选中 worktree `WorktreeOffset = Exhausted`，k-way merge 在 server 端自然只产出选中 worktree 的 sessions（详 ipc-data-api spec `Expose group session listing via k-way merge pagination` § "worktree filter 通过 cursor 表达"）。"全部"时 SHALL 传 `cursor = null`，server 按全 worktree 拉。SHALL NOT 走纯前端展示过滤——以避免低占比 worktree 一页 50 条只命中 2-3 条让用户看到空列表卡住的退化（codex post-propose 二审 #5 驳回前端 filter 方案）。

filter 切到具体 worktree 后续页 cursor 由 server 在 `list_group_sessions` 响应中自然返回（保持 `Exhausted` 标记），前端 loadMore 直接续传该 cursor 即可。

filter state SHALL session-scoped（仅本次会话状态），切 group 时重置为"全部"，不跨会话持久化。

**自动补页保护**：若 server-side filter 后某页 sessions 数仍 < `pageSize`（理论上仅在该 worktree 接近耗尽时发生），sidebar SHALL 自动 loadMore 直到填满一屏或 cursor 全部 `Exhausted`，避免视觉空白。

#### Scenario: 多 worktree group 默认显示 filter
- **WHEN** 用户切到 worktrees.length === 2 的 group
- **THEN** sidebar 顶部 SHALL 显示 worktree filter 下拉，默认 selected "全部"

#### Scenario: 单 worktree group 隐藏 filter
- **WHEN** 用户切到 worktrees.length === 1 的 group（standalone project）
- **THEN** sidebar 顶部 SHALL NOT 渲染 worktree filter 下拉

#### Scenario: 切 filter 构造 server-side filter cursor
- **WHEN** 用户在多 worktree group（含 worktree `wt-A` / `wt-B` / `wt-C`）切 filter 从 "全部" → `wt-B`
- **THEN** session 列表 SHALL 立即清空
- **AND** 前端 SHALL 构造 cursor `{ "wt-A": Exhausted, "wt-B": NotStarted, "wt-C": Exhausted }` (base64 JSON) 调 `list_group_sessions(groupId, pageSize, cursor)`
- **AND** server 返回 sessions SHALL 仅含 `wt-B` 的 sessions

#### Scenario: 切回"全部"清空 cursor
- **WHEN** 用户在已选 `wt-B` 状态切回"全部"
- **THEN** 前端 SHALL 调 `list_group_sessions(groupId, pageSize, null)`（cursor 重置 null）
- **AND** server 返回 sessions SHALL 含全 group 的合并条目

#### Scenario: 切 group 重置 filter
- **WHEN** 用户从 group A 切到 group B
- **THEN** filter state SHALL 重置为 "全部"（即便 A 上次选了某具体 worktree）

#### Scenario: filter option 含分支 + cwd hint
- **WHEN** group 含 worktree 名 `feat-x`，git_branch `feat/x`，cwd_relative_to_repo_root `.claude/worktrees/feat-x`
- **THEN** 对应 filter option SHALL 显示 `feat-x` + git icon `feat/x` chip + `.claude/worktrees/feat-x` hint

#### Scenario: 自动补页防止首屏视觉空白
- **WHEN** server-side filter 返回某页 sessions 数 < pageSize 但 cursor 还有非 Exhausted worktree
- **THEN** sidebar SHALL 自动续调 loadMore 直到填满一屏或 cursor 全部 Exhausted

### Requirement: Session row branch + cwd chip 替代行尾 cwd 全路径

Sidebar 会话项渲染 SHALL 在每条 session 行右侧 chip group 显示 worktree 归属信息：
1. **分支 chip**：当 `session.gitBranch` 非空时渲染 git icon + branch 名（沿用既有 `ProjectSwitcher.svelte::.dropdown-item-branch` 样式）
2. **cwd hint chip**：当 `session.cwdRelativeToRepoRoot` 非空且非空字符串时渲染 `…/<lastTwoSegs>`（路径取最后两段：`crates/cdt-discover` → `crates/cdt-discover`、`.claude/worktrees/feat-x` → `worktrees/feat-x`）

SHALL NOT 渲染 PR #183 引入的 `Sidebar.svelte` 行尾 cwd 全路径 label（`<span class="session-cwd">{cwdTailLabel(session.cwd)}</span>`）。该 label 整块代码 SHALL 从 `Sidebar.svelte` 移除。

`SessionDetail.svelte` 顶部 cwd badge SHALL 保持不变（详情页需要完整 cwd path）。

#### Scenario: session 行渲染分支 chip
- **WHEN** session.gitBranch = "feat/x"
- **THEN** session 行右侧 SHALL 渲染 git icon + "feat/x" chip

#### Scenario: 子目录 cwd session 行渲染 cwd hint chip
- **WHEN** session.cwdRelativeToRepoRoot = "crates"
- **THEN** session 行右侧 SHALL 渲染 "crates" chip（短路径直接全显）

#### Scenario: 深层子目录截取最后两段
- **WHEN** session.cwdRelativeToRepoRoot = ".claude/worktrees/feat-x"
- **THEN** session 行右侧 cwd hint chip SHALL 显示 "worktrees/feat-x"

#### Scenario: repo 根 session 不渲染 cwd hint
- **WHEN** session.cwdRelativeToRepoRoot 为 None 或 空字符串
- **THEN** session 行 SHALL NOT 渲染 cwd hint chip（仅渲染分支 chip）

#### Scenario: PR #183 行尾 cwd 全路径 label 已移除
- **WHEN** Sidebar.svelte grep `<span class="session-cwd">` 或 `cwdTailLabel`
- **THEN** SHALL 无任何匹配（label 渲染代码已删）

### Requirement: selectedGroupId 与 worktree id 分层维护

Sidebar 当前选中的项目入口 SHALL 用新的 `selectedGroupId` 字段持 `RepositoryGroup.id`，用于顶层导航 / 列表分页 / SSE event 过滤 / 用户配置持久化。**但** session 详情链路（`get_session_detail` / `get_tool_output` / `get_image_asset` / `get_subagent_trace`）入参的 project id MUST 继续持 worktree id（即底层 `Project.id`，encoded project dir 名），避免 detail API 无法定位具体 session 文件（codex post-propose 二审 #3 驳回一刀切方案）。

两套 id 通过 `SessionSummary.worktreeId` + `SessionSummary.groupId` + `SessionSummary.sessionId` 三元组桥接：UI 列表点击 session 时拿 `session.worktreeId` 注入 tab，tab 内 detail 路径用 worktree id 走老 IPC，sidebar 顶层 `selectedGroupId` 不变。

收敛点 checklist（apply 时按表 verify）：

| 位置 | 现状 | 改动 | 备注 |
|---|---|---|---|
| `ui/src/lib/sidebarStore.svelte.ts::selectedProjectId` | worktree id | rename → `selectedGroupId`，持 group id | 顶层导航 |
| `ui/src/lib/projectDataStore.svelte.ts::fetchProjectData` 推导默认 | 取 `group.worktrees[0].id` | 取 `group.id` | 顶层导航 |
| `ui/src/components/Sidebar.svelte::loadSessions` 入参 | worktree id → `listSessions(projectId, ...)` | group id → `listGroupSessions(groupId, pageSize, cursor)` | 列表分页 |
| `ui/src/lib/sessionListStore.svelte.ts` cache key | project id | group id + worktree filter id 组合 key（filter 维度纳入 cache key 否则切 filter 串台） | session 列表缓存 |
| SSE `session-metadata-update` event payload | 含 `projectId` (worktree id) | **新增** `groupId` 字段；前端 filter 按 `payload.groupId === selectedGroupId` 匹配；保留 `projectId` 字段供 detail 路径用 | event 过滤 |
| `active_scans` per-key cancel | key = project_id | 分两类 key：detail 拉取 = `(project_id /*worktree id*/, session_id)`（不变）；group 分页拉取 = `(group_id, page_cursor_hash)`（新加） | 后台任务取消 |
| `ui/src/lib/tabStore.svelte.ts::tab.projectId` | worktree id | **不变**（保留 worktree id，detail API 仍按 worktree id 定位）；新增 `tab.groupId: string` 字段供 sidebar 高亮"该 tab 属于哪个 group" | tab 状态 |
| `ui/src/routes/SessionDetail.svelte::getSessionDetail(projectId, sid)` | 用 `tab.projectId` | **不变**（仍传 worktree id） | detail API |
| `ui/src/components/CommandPalette.svelte::listSessions(selectedProjectId, ...)` | worktree id | 改为调 `listGroupSessions(selectedGroupId, pageSize, null)` 拿合并 sessions 做全局 fuzzy 候选；候选项 onclick 时按 `candidate.worktreeId` 创建 tab | 全局搜索 |
| 用户配置 `selected_project_id` 持久化 | worktree id | 改为 `selected_group_id`；启动时若读到老 worktree id，按 grouper 反查 group id 后改写一次（迁移） | persistence |
| 项目 memory / prefs（如有 per-project state） | worktree id | **不变**（维持 per-worktree，与 detail API 一致） | per-project state |

单 worktree group 时 group id 与 worktree id 字符串相同（grouper 在 standalone project 场景下 `group.id = project.id`），单 worktree 项目用户无感知 ID 变化。

#### Scenario: SSE patch 按 groupId filter
- **WHEN** 用户当前 `selectedGroupId = "group-X"`，SSE 推送 `session-metadata-update` event 含 `groupId: "group-Y"` + `projectId: "wt-Z"`
- **THEN** 前端 SHALL 丢弃该 event（不 patch 到当前列表）

#### Scenario: SSE patch 同 group 命中
- **WHEN** 用户当前 `selectedGroupId = "group-X"`，event 含 `groupId: "group-X"` + `projectId: "wt-X1"` + sessionId
- **THEN** 前端 SHALL 在列表中找到对应 session 并 patch metadata

#### Scenario: 打开 session detail 用 worktree id
- **WHEN** 用户点击 sidebar 某 session 行（`session.worktreeId = "wt-X1"`、`session.sessionId = "sid"`）
- **THEN** 前端 SHALL 创建 / 切换 tab，写入 `tab.projectId = "wt-X1"` + `tab.sessionId = "sid"` + `tab.groupId = "group-X"`
- **AND** `SessionDetail` SHALL 调 `getSessionDetail("wt-X1", "sid")` 走老 detail IPC 路径

#### Scenario: CommandPalette 全局搜索走 group 维度
- **WHEN** 用户在 CommandPalette 输入查询词，当前 `selectedGroupId = "group-X"`
- **THEN** CommandPalette SHALL 调 `listGroupSessions("group-X", pageSize: 200, cursor: null)` 拿合并候选（覆盖所有 worktree 的 session）
- **AND** 候选项 onclick 时 SHALL 按 `candidate.worktreeId` 创建 tab，tab 内 detail 路径走 worktree id

#### Scenario: sessionListStore cache key 含 worktree filter
- **WHEN** 用户在 group-X 内切 filter 从 "全部" → "wt-X1" → "全部"
- **THEN** sessionListStore 缓存 SHALL 用 `(groupId, filterWorktreeId | null)` 作为 cache key 区分三个状态的缓存
- **AND** 切回"全部"时 SHALL 命中第一次"全部"的缓存，不重发 IPC

#### Scenario: 单 worktree group group id 等于 worktree id
- **WHEN** standalone project 转化的 RepositoryGroup
- **THEN** `group.id` SHALL 等于 `group.worktrees[0].id`（即 encoded project dir 名）
- **AND** 单 worktree 项目用户配置 `selected_project_id` 在迁移前后字符串相同，无需写迁移

## MODIFIED Requirements

### Requirement: 项目选择

`UnifiedTitleBar` 的 `zone-left-center` SHALL 提供项目选择下拉作为主导航控件，**项目入口语义 SHALL 为 RepositoryGroup（同一个 git repo 一个条目）**——不再按 worktree 维度暴露多条平铺。选择项目后 SHALL 自动加载该项目（group）的合并 session 列表到 Sidebar。项目选择控件 MUST NOT 渲染在 `SidebarHeader.svelte` 或 Sidebar 内部，MUST NOT 随 sidebar 折叠状态消失。

多 worktree group SHALL 在下拉内显示为**单行**（不再 accordion），点击即选中整个 group；单 worktree group 同样渲染为单行，行为一致。worktree 维度的快速切换由 sidebar 顶部的 worktree filter 下拉提供（见 `Worktree filter dropdown for multi-worktree group` Requirement），不在 ProjectSwitcher 内暴露。

#### Scenario: 初始加载
- **WHEN** 应用启动且有可用项目
- **THEN** 系统 SHALL 自动选中第一个项目（按 `mostRecentSession` 倒序的首个 RepositoryGroup）并加载其合并 session 列表
- **AND** chrome 内项目下拉 SHALL 显示当前选中 group 的名称（`group.name`）

#### Scenario: 切换项目
- **WHEN** 用户从 chrome 内项目下拉选择器切换到另一个 group
- **THEN** session 列表 SHALL 更新为新 group 的合并 sessions，之前的列表 SHALL 被替换
- **AND** chrome 内项目下拉 SHALL 显示新选中 group 的名称

#### Scenario: 无项目
- **WHEN** 无可用项目
- **THEN** Sidebar SHALL 显示空状态提示
- **AND** chrome 内项目下拉 SHALL 显示禁用态占位文本（如「无项目」）

#### Scenario: sidebar 折叠不影响项目选择
- **WHEN** 用户点击 chrome 内 sidebar 折叠按钮把 sidebar 收起
- **THEN** chrome 内项目下拉 SHALL 仍可见且可操作
- **AND** 用户 SHALL 可在 sidebar 折叠态切换项目，新项目的会话列表会在重新展开 sidebar 时立即可见

#### Scenario: 多 worktree group 单行展示无 accordion
- **WHEN** ProjectSwitcher 渲染一个 worktrees.length === 19 的 group
- **THEN** 下拉内 SHALL 显示**一行**（不展开 19 条子项），点击即选中该 group
- **AND** 行内 SHALL 显示 `group.name` + `group.totalSessions` 计数
- **AND** SHALL NOT 渲染 `dropdown-group-row` chevron / worktree count badge / 展开后的 worktree 子列表

### Requirement: 默认渲染按仓库聚合的 Sidebar

Sidebar SHALL 默认调用 `list_repository_groups()` IPC 拉取按 git 仓库聚合的项目列表，把同一仓库的多个 worktree 合并为单个 RepositoryGroup。ProjectSwitcher（chrome 内）SHALL 把每个 `RepositoryGroup` 渲染为单行 dropdown item（无论多 worktree 还是单 worktree group），点击即选中整个 group。

单 worktree group（只含一个 worktree）SHALL 与多 worktree group 走同一渲染分支（无特殊处理），点击行为一致：选中后 sidebar 调 `list_group_sessions(groupId, pageSize, null)` 拉取该 group 的合并 sessions（单 worktree 时合并退化为该 worktree 自身 sessions）。

`expandedGroupIds: Set<string>` state SHALL 移除——ProjectSwitcher 不再有 accordion 展开/折叠交互。worktree 维度的细化由 sidebar 顶部 worktree filter 下拉提供。

#### Scenario: 多 worktree group 单行渲染
- **WHEN** Sidebar 拉到一个 group 含 19 个 worktree
- **THEN** ProjectSwitcher 下拉内 SHALL 渲染**一行** dropdown item（`group.name` + `group.totalSessions`），无 chevron、无 worktree count badge、无展开子列表
- **AND** 点击该行 SHALL 把 `selectedProjectId` 设为 `group.id`，触发 `list_group_sessions` 拉取合并 sessions

#### Scenario: 单 worktree group 同分支渲染
- **WHEN** Sidebar 拉到一个 group 只含 1 个 worktree（standalone project）
- **THEN** ProjectSwitcher 下拉内 SHALL 渲染一行 dropdown item（与多 worktree group 同分支）
- **AND** 点击该行 SHALL 把 `selectedProjectId` 设为 `group.id`（标量等于 `worktrees[0].id`）

#### Scenario: 不再渲染 accordion
- **WHEN** ProjectSwitcher.svelte grep `dropdown-group-row` / `dropdown-group-chevron` / `dropdown-group-badge` / `expandedGroupIds`
- **THEN** SHALL 无任何匹配（accordion 代码已删）

### Requirement: 活跃 worktree 选中状态

Sidebar SHALL 维护"当前选中的 RepositoryGroup"为单一来源真值——`App.selectedProjectId` 字段持 `group.id`（不再持 worktree.id）。SessionList SHALL 通过 `list_group_sessions(group.id, pageSize, cursor)` 拉取该 group 内所有 worktree 合并、按 mtime 全局倒序的 sessions。

worktree 维度的过滤由 sidebar 顶部 worktree filter 下拉控制（见 `Worktree filter dropdown for multi-worktree group` Requirement）；filter state 与 `selectedProjectId` 分离持有（filter state 在 group 级 scope）。

`selectedProjectId` 跨会话持久化由 `Migrate persisted selected_project_id on load` Requirement 负责迁移老 worktree id 格式。

`get_worktree_sessions` IPC 保留为兼容入口供未来"group 概览页 + 多维过滤"等场景按需调用；本 Requirement 不要求 sidebar 默认调它。

#### Scenario: 切换 group 调 list_group_sessions
- **WHEN** 用户在 ProjectSwitcher 切到 group-X
- **THEN** `App.selectedProjectId` SHALL 更新为 `"group-X"`
- **AND** SHALL 触发 `list_group_sessions(groupId: "group-X", pageSize: 50, cursor: null)` 拉取合并 sessions
- **AND** SessionList SHALL 渲染合并后的 sessions

#### Scenario: 默认选中最近活动 group
- **WHEN** 应用启动，`list_repository_groups()` 返回多个 group（按 mostRecentSession 倒序）
- **THEN** `App.selectedProjectId` 初始值 SHALL 为最近活动 group 的 `group.id`
- **AND** worktree filter SHALL 初始为 "全部"

#### Scenario: list_group_sessions 触发 SSE detail 推送
- **WHEN** `list_group_sessions` 返回首页 50 条骨架
- **THEN** 后台 SHALL 对这 50 条 session 触发 `session-metadata-update` SSE 推送
- **AND** 前端 SHALL 按 `(groupId, sessionId)` 匹配 patch metadata

### Requirement: 会话项展示

每个会话项 SHALL 显示标题和元数据（消息计数、相对时间、git 分支、worktree cwd hint）。标题 SHALL 优先使用后端提供的 title 字段，无 title 时 fallback 到**完整 sessionId**——CSS 的 `text-overflow: ellipsis` 自然截断超出宽度的部分；同时 SHALL 在该元素上设置 HTML `title` 属性（`title || sessionId` 完整值），让用户 hover 时浏览器原生 tooltip 显示完整字符串。**禁止**在 JS 侧再手动 `slice(0, 8) + "…"`——双重截断让用户看到的是"前 8 字符 + …"既不能复制粘贴定位 session、也丢失了 CSS 自然 ellipsis 提供的 hover 全展能力。

消息计数（`SessionSummary.messageCount`）SHALL 等于该 session 文件中**真实 user-chunk 消息**与配对 assistant 消息的总数——后端 `cdt-api/src/ipc/session_metadata.rs::extract_session_metadata` MUST 用对齐原版 `claude-devtools/src/main/types/messages.ts::isParsedUserChunkMessage` 的过滤函数判定 user 消息：`category != User` 或 `is_meta = true` 或 `MessageContent::Blocks` 不含任何 `Text` / `Image` block（即纯 `tool_result`-only 行）SHALL NOT 计入。配对计数规则保持原状：每个 user-chunk 后，紧接的第一个非 synthetic 非 sidechain 的 assistant 消息计 1（与 `awaitingAIGroup` 状态机一致）。

git 分支（`SessionSummary.gitBranch`）SHALL 在每条会话项第二行 meta 末尾以 `· <GitBranch icon> {branch}` chip 形式渲染；`gitBranch` 为 `null` 时 SHALL NOT 渲染该 chip（不留分隔符 `·`、不留空位）。该 chip MUST 跟随 `session-metadata-update` 事件 patch 的 `gitBranch` 即时更新。

worktree cwd hint（`SessionSummary.cwdRelativeToRepoRoot`）SHALL 在 `gitBranch` chip 之后以单独 chip 渲染：`…/<lastTwoSegs>`（最后两段路径）。`cwdRelativeToRepoRoot` 为 `null` / 空字符串时 SHALL NOT 渲染该 chip。

会话项 SHALL NOT 渲染 PR #183 引入的 `<span class="session-cwd">{cwdTailLabel(session.cwd)}</span>` 行尾全路径 label——该展示路径已被 `Session row branch + cwd chip` Requirement 替代。

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

#### Scenario: 元数据含 cwd hint chip
- **WHEN** 会话项渲染，`cwdRelativeToRepoRoot = "crates"`
- **THEN** SHALL 在 gitBranch chip 之后追加 cwd hint chip 显示 "crates"

#### Scenario: 深层子目录 cwd hint 取最后两段
- **WHEN** 会话项渲染，`cwdRelativeToRepoRoot = ".claude/worktrees/feat-x"`
- **THEN** cwd hint chip SHALL 显示 "worktrees/feat-x"

#### Scenario: repo 根 session 不渲染 cwd hint chip
- **WHEN** `cwdRelativeToRepoRoot` 为 null / 空字符串
- **THEN** SHALL NOT 渲染 cwd hint chip

#### Scenario: 行尾全路径 label 已移除
- **WHEN** Sidebar.svelte grep `cwdTailLabel` 或 `session-cwd`
- **THEN** SHALL 无任何匹配

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

### Requirement: 完整加载分页会话历史

Sidebar 默认会话列表 SHALL 使用 `list_group_sessions(groupId, pageSize, cursor)` 的分页结果渐进展示 sessions，而不是为了首屏或普通浏览同步加载完整会话历史。若 `list_group_sessions` 响应包含非空 `nextCursor`，Sidebar SHALL 在用户滚动接近列表末尾或显式请求更多时继续分页；Command Palette 需要覆盖完整历史搜索时，MUST 使用 `session-search` 或显式承担逐页加载成本的专用路径，不能要求 Sidebar 首屏预先加载完整历史。

实现 SHALL NOT 使用"扩大 `pageSize` 并从头重拉直到 `nextCursor = null`"作为 Sidebar 首屏策略。实现 SHALL 保证每次分页返回页的 `session-metadata-update` 扫描不会因为后续页加载而错误覆盖或丢失已加载页的 metadata patch（按 `(groupId, sessionId)` key 匹配 patch，跨页 patch 互不干扰）。

#### Scenario: Sidebar 首屏不加载默认第一页之后的旧会话

- **WHEN** 当前 group 有 51 条合并 sessions，且 `list_group_sessions(groupId, { pageSize: 20, cursor: null })` 返回第一页并带 `nextCursor`
- **THEN** Sidebar SHALL 立即显示第一页 sessions
- **AND** Sidebar SHALL NOT 为了首屏显示第 51 条旧会话而同步加载完整 51 条

#### Scenario: Sidebar 滚动后加载默认第一页之后的旧会话

- **WHEN** 当前 group 有 51 条合并 sessions，且用户持续滚动到需要更多 sessions
- **THEN** Sidebar SHALL 使用 `nextCursor` 继续请求后续页
- **AND** 第 51 条旧会话 SHALL 在其所在页加载后出现在会话列表中

#### Scenario: 切 worktree filter 重置分页 cursor
- **WHEN** 用户在多 worktree group 内切换 filter
- **THEN** Sidebar SHALL 清空已加载页 + 重置 cursor 为 null + 重调 `list_group_sessions`

## REMOVED Requirements

### Requirement: Worktree 子项展示元信息

**Reason**：方案 B 把 worktree 维度从 ProjectSwitcher 顶层入口抹掉，accordion 内子项不再存在；worktree 选择能力下沉到 sidebar 顶部 worktree filter 下拉。

**Migration**：原 accordion 子项的 branch + 时间 + count 三元素，迁移到 worktree filter 下拉 option 内的 branch chip + cwd hint chip + session count 徽章（见 `Worktree filter dropdown for multi-worktree group`）。
