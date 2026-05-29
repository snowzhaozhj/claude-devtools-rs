## Context

CommandPalette（Cmd+K 快捷面板）在 PR #385 中完成了 session 列表的 group 化（改用 `listGroupSessions`），但两处遗漏使多 worktree 用户体验退化：

1. 搜索 `searchSessions(selectedGroupId, q)` 后端用 `projects_dir.join(project_id)` 定位目录——group id 不是任何单一 worktree 的 encoded path（除非恰好是单 worktree group），目录不存在返回空。
2. 项目列表 `listProjects()` 返回 worktree 级 `ProjectInfo`，选中后写入 `selectedGroupId` 但传入的是 worktree id 而非 group id。

`App.svelte:318` 把 `selectedGroupId` 作为 `selectedProjectId` prop 传给 CommandPalette——调用方语义已是 group id，但 CommandPalette 内部两处消费仍假设 worktree id。

## Goals / Non-Goals

**Goals:**
- 多 worktree group 用户在 CommandPalette 中搜索能命中所有 worktree 的 sessions
- 项目列表选择写入 `selectedGroupId` 的值语义正确（id = group.id）
- 单 worktree 用户零行为变更

**Non-Goals:**
- 不加搜索结果的 worktree 来源标注（后续迭代）
- 不改 search result 的展示 UI（已有 sessionTitle / totalMatches 足够）
- 不加搜索结果分页（当前 max_results=50 足够）

## Decisions

### D1：后端 `search_group_sessions` 复用 `list_repository_groups_inner()` 解析 group

**选择**：新增 `DataApi::search_group_sessions(group_id, query)` 方法，实现中调 `self.list_repository_groups_inner()` 一次拿 `(groups, fs, projects_dir, ctx, generation)` 五元组——与 `build_group_session_page` 同源（D2 change `generation-race-audit`），避免 fs/group 跨 await race。

**理由**：
- `list_repository_groups_inner()` 已是"拿当前 active context 下的 group + fs + projects_dir"的单一 snapshot helper
- 搜索需要 fs + projects_dir + group.worktrees — 与 `build_group_session_page` 完全同源
- 不新建 helper — 复用降低代码表面积

**替代方案（已否决）**：
- "直接用 `active_fs_and_policy()` + 单独 `list_repository_groups()`"——两次 await 之间可能被 ssh switch/reconfigure 跨过（`generation-race-audit` D2 已修的问题）

### D2：`SessionSearcher::search_across_projects` 全局 mtime 排序 + 逐文件搜索

**选择**：新增 `search_across_projects(project_ids: &[&str], query, max_results, config)` 方法：收集所有 worktree project_dir 下的 session 文件，全局按 mtime desc 排序后逐文件搜索（复用 `search_session_file`）。SSH stage-limit 和 time_budget 按总量生效。

**理由**：
- 全局 mtime 排序保证用户看到的结果永远是"最近活跃的 session 优先"——符合直觉
- 复用 `search_session_file` 和缓存（SearchTextCache 按 file path key），不引入新缓存层
- SSH stage-limit 自然扩展到跨 worktree 合集（`processed` 计数跨 worktree 递增）

**替代方案（已否决）**：
- "逐 worktree 调 `search_sessions` 再 merge results"——每次调用独立计数 stage-limit，SSH 用户可能在第一个 worktree 就提前返回而不搜后续 worktree
- "并发搜索各 worktree"——CPU-bound 路径（全文搜索），并发不比顺序快（都是内存操作），反而增加复杂度

### D3：前端项目列表直接复用 `loadProjectData().projects`

**选择**：CommandPalette 的项目列表从 `listProjects()` 改为 `loadProjectData().projects`——`projectDataStore` 的 `summarizeRepositoryGroups()` 已生成 group 化的 `ProjectInfo[]`（id = group.id、path = anchor worktree.path、displayName = group.name）。

**理由**：
- `loadProjectData()` 在 App.svelte onMount 时已调过一次，首帧 data 已缓存——CommandPalette 打开时零额外 IPC
- `projects` 数组字段 shape 与现有 `ProjectInfo` interface 完全一致——模板代码零改动
- `onSelectProject(project.id, project.displayName)` 写入 `selectedGroupId` 时 id 语义正确

**替代方案（已否决）**：
- "CommandPalette 内部自己调 `listRepositoryGroups()` 后手动 map"——多一次 IPC + 重复 summarize 逻辑
- "传 `repositoryGroups` prop 给 CommandPalette 让它自己 render group items"——需改 Props 接口 + 改模板，过大改动

### D4：缺失 worktree project_dir 的降级策略

**选择**：`search_across_projects` 中 `list_session_files(&project_dir)` 若 `read_dir` 返回 ENOENT（worktree 对应目录不存在）或任何 IO 错误，warn + skip 继续搜索其余 worktree。不整体 500。

**理由**：
- 与 `build_group_session_page` 一致的降级语义（scan 失败的 worktree 跳过，不阻塞剩余）
- 常见场景：worktree 刚添加尚未有 claude session

## Risks / Trade-offs

- **性能**：N 个 worktree × M 个 session 文件全局 mtime 排序后逐文件搜索。最坏情况（20 worktrees × 100 sessions）= 2000 文件排序（O(n log n)，< 1ms）+ 逐文件搜索受 max_results=50 和 SSH stage-limit 限制，不会全扫。单 worktree 退化为 1 × M = 现有路径。
- **返回类型兼容**：`SearchSessionsResult.results[].project_id` 原来指向 worktree encoded path；改为 group 搜索后每条结果的 `project_id` 仍是实际 worktree id（因为 `search_session_file` 传入的是 `wt.id`）——前端 `openTab(session.sessionId, session.projectId, ...)` 仍能正确定位 session 文件。
