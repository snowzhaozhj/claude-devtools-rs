## Why

当主 session 用 Claude Code 的 `EnterWorktree` 把 cwd 切到 `<repo>/.claude/worktrees/<slug>/`、或 subagent 在子 cwd 里跑工具时，Claude Code 把 subagent JSONL 写到 worktree cwd 编码出的另一个 `project_dir`，与主 session 所在的 `project_dir` 不同。当前 `crates/cdt-api/src/ipc/local.rs::scan_subagent_candidates` 只扫主 `project_dir`，找不到这些跨目录的 candidate，UI 里 Task tool 调用退化成 raw tool，缺 `SubagentCard`。同时，前端 sidebar 没有按 git repo 折叠多个 worktree——Rust 后端虽然已有 `WorktreeGrouper`（`crates/cdt-discover/src/worktree_grouper.rs`），但 IPC 层 `LocalDataApi::list_projects` 拍平返回、`get_worktree_sessions` 是空 stub，sidebar 上同一 repo 的多个 worktree（含 subagent 创建的）看起来像独立项目，造成"项目叠加"视觉。

两件事都源于同一前提：**主 session 与其衍生 subagent / 用户开的 worktree 分布在多个 `project_dir`**。统一在一个 change 里建立"跨 `project_dir` 的 repo 聚合视图"。

## What Changes

- **subagent 跨 `project_dir` 装载**（bug fix）：新增 `scan_subagent_candidates_cross_project(projects_dir, root_session_id)` 与 `find_subagent_jsonl_cross_project(projects_dir, root_session_id, sub_session_id)`，扫所有 `{projects_dir}/*/{root_session_id}/subagents/agent-*.jsonl`。新结构（`{project_dir}/{rootSessionId}/subagents/agent-{agentSessionId}.jsonl`）走跨目录，旧结构（flat `{project_dir}/agent-*.jsonl`）保持只扫主 `project_dir`。`get_session_detail` / `get_subagent_trace` / `get_image_asset` / `get_tool_output` / `locate_session_jsonl` 的内部调用同步替换。
- **`list_repository_groups` 与 `get_worktree_sessions` IPC**（feature）：`LocalDataApi` 注入 `WorktreeGrouper<LocalGitIdentityResolver>`，新增 `list_repository_groups() -> Vec<RepositoryGroup>` 与实现 `get_worktree_sessions(group_id, pagination)`；`DataApi` trait、Tauri `invoke_handler!`、`EXPECTED_TAURI_COMMANDS`、`KNOWN_TAURI_COMMANDS`、HTTP `/api/repository-groups` 同步注册；contract test 覆盖新 IPC camelCase 形态。
- **前端 grouped sidebar**（feature）：移植原版 `../claude-devtools/src/renderer/components/layout/SidebarHeader.tsx::groupWorktreesBySource`（lines 49-88、513-538）到 Svelte 5；`Sidebar.svelte` / `SidebarHeader.svelte` 默认展示 grouped 视图，同一 repo 多 worktree 折叠为可展开行；`ui/src/lib/api.ts` 加 `listRepositoryGroups()` / `getWorktreeSessions()` + `RepositoryGroup` / `Worktree` interface；`__fixtures__/multi-project-rich.ts` 与新 `__fixtures__/repository-groups.ts` 覆盖 mockIPC。
- **性能预算**：50 个 `project_dir` 跨扫描 < 50 ms（O(N) 一次 fs metadata stat）。tracing 探针 `cdt_api::perf` 记录 `projects_scanned` / `candidates_found`。

## Capabilities

### New Capabilities

- `sidebar-navigation`：covers Sidebar / SidebarHeader 的 grouped vs flat 模式切换、worktree 折叠展开、active worktree 选择状态、源分类（main / external worktree / standalone）等 UI 行为契约

### Modified Capabilities

- `tool-execution-linking`：补 Scenario "Subagent JSONL located in a different project directory" 到 `Resolve Task subagents with three-phase fallback matching` Requirement
- `ipc-data-api`：(a) 修订 `Expose project and session queries` 的 subagent 装载约定为"扫主 session 所在 `projects_dir` 下所有 project 目录的 `{rootSessionId}/subagents/agent-*.jsonl`"；(b) 修订 `Lazy load subagent trace` 的路径解析约定；(c) ADD `list_repository_groups` 与 `get_worktree_sessions` Requirements
- `project-discovery`：补 `Group projects by git worktree` 的 Scenario 完整度——`is_main_worktree` 排序约定、`RepositoryGroup` 字段集合（如尚未在 spec 中显式声明）

## Impact

- **后端代码**：`crates/cdt-api/src/ipc/local.rs`（scan / find / get_session_detail / get_subagent_trace / get_image_asset / get_tool_output / locate_session_jsonl / 新 list_repository_groups / get_worktree_sessions）、`crates/cdt-api/src/ipc/traits.rs`（DataApi trait 加两个方法）、`crates/cdt-api/src/http/routes.rs`（/api/repository-groups 新增）、`crates/cdt-api/tests/ipc_contract.rs`（EXPECTED_TAURI_COMMANDS 同步 + 新 contract test）、`crates/cdt-api/tests/cross_project_subagent.rs`（新集成测试）
- **Tauri 桥接**：`src-tauri/src/lib.rs::invoke_handler!` 注册 `list_repository_groups` + `get_worktree_sessions`
- **前端代码**：`ui/src/lib/api.ts` / `ui/src/lib/components/Sidebar.svelte` / `ui/src/lib/components/SidebarHeader.svelte` / `ui/src/lib/__fixtures__/multi-project-rich.ts` + 新 `repository-groups.ts` / `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS`
- **Spec**：4 个 capability 的 delta（1 个新建 + 3 个修订）
- **测试金字塔**：(a) Rust IPC contract test 覆盖两个新 command 字段；(b) cross_project_subagent 集成测试覆盖跨目录扫描；(c) vitest 单测覆盖 grouped sidebar store 状态机；(d) Playwright e2e 覆盖 grouped sidebar 展开/折叠交互
- **不影响**：raw IPC payload 字段（无新 omit 策略）、`tauri.conf.json::plugins.updater`、release.yml workflow、release-check 一致性
