## Why

PR #385 修复了 CommandPalette 的 session 列表（改用 `listGroupSessions`），但 codex 二审发现该组件仍有两处未完成 group 化：

1. **搜索仍传 worktree-level id 给 `searchSessions`**：`selectedProjectId` prop 实际传入的是 `selectedGroupId`（App.svelte:318），但 `searchSessions(projectId, q)` 后端用 `projects_dir.join(project_id)` 查目录——多 worktree group 下该目录不存在，返回空结果。
2. **项目列表来源未 group 化**：`listProjects()` 返回 worktree 级 `ProjectInfo`（id = encoded worktree path），用户选择后把 worktree id 写入 `selectedGroupId`——语义不匹配。

单 worktree 用户不受影响（group id = worktree id）；多 worktree 用户在 CommandPalette 中搜索返回空结果，项目切换可能语义错乱。

## What Changes

- **后端新增 `search_group_sessions(group_id, query)`**：遍历 group 内所有 worktree，合并搜索结果（全局按 mtime desc 排序），复用 `SessionSearcher::search_sessions` per-worktree。
- **前端 CommandPalette 项目列表**：`listProjects()` → `loadProjectData().projects`（已做 group 化 summary，id = group.id）。
- **前端 CommandPalette 搜索**：`searchSessions(projectId, q)` → `searchGroupSessions(groupId, q)`。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `ipc-data-api`：新增 `search_group_sessions` Tauri command + HTTP route
- `session-search`：新增 "Search across all worktrees of a repository group" Requirement

## Impact

- **代码**：
  - `crates/cdt-api/src/ipc/traits.rs` — DataApi trait 新增方法
  - `crates/cdt-api/src/ipc/local.rs` — LocalDataApi 实现（复用 `list_repository_groups_inner()`）
  - `crates/cdt-discover/src/session_search.rs` — 新增 `search_across_projects` 方法
  - `crates/cdt-api/src/http/routes.rs` — 新增 HTTP route + handler
  - `src-tauri/src/lib.rs` — 注册 Tauri command
  - `ui/src/lib/api.ts` — 新增 `searchGroupSessions` wrapper
  - `ui/src/components/CommandPalette.svelte` — 改用 group 化 API
  - `ui/src/lib/tauriMock.ts` — 新增 mock case
- **性能**：多 worktree group 搜索相当于逐 worktree 串行调 `search_sessions`（每个 worktree 内按 mtime 倒序），总 I/O 量 = 所有 worktree sessions 文件之和。SSH stage-limit 与 time_budget 保持生效。单 worktree 用户退化为原有 `search_sessions` 路径——零额外开销。
- **风险**：低。逻辑是已有 `search_sessions` 的简单 N 路合并；返回类型不变（`SearchSessionsResult`）；前端消费方不变。
