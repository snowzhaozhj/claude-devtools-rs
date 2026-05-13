## 1. 后端跨 `project_dir` subagent 装载（`cdt-api`）

- [ ] 1.1 在 `crates/cdt-api/src/ipc/local.rs` 顶层加 `const CROSS_PROJECT_SUBAGENT_SCAN: bool = true;` 回滚开关
- [ ] 1.2 新增 `scan_subagent_candidates_cross_project(projects_dir: &Path, root_session_id: &str) -> Vec<SubagentCandidate>` 函数：遍历 `projects_dir` 下每个 project_dir，探测 `{dir}/{root_session_id}/subagents/`，存在则收集 `agent-*.jsonl`（跳过 `agent-acompact*`），调 `parse_subagent_candidate` 解析
- [ ] 1.3 新增 `find_subagent_jsonl_cross_project(projects_dir: &Path, root_session_id: &str, sub_session_id: &str) -> Option<PathBuf>` 函数：扫所有 `{projects_dir}/*/{root_session_id}/subagents/agent-{sub_session_id}.jsonl`，命中即返
- [ ] 1.4 旧结构 fallback：cross 版本未命中时，回退到原 `scan_subagent_candidates(&主_project_dir, ...)` 与 `find_subagent_jsonl(&主_project_dir, ...)` 各跑一遍合并结果
- [ ] 1.5 改 `get_session_detail`（`local.rs:687`）调用 `scan_subagent_candidates_cross_project(&projects_dir, session_id)`，受 `CROSS_PROJECT_SUBAGENT_SCAN` 开关 gate
- [ ] 1.6 改 `get_subagent_trace`（`local.rs:852-870`）内部用 `find_subagent_jsonl_cross_project`
- [ ] 1.7 改 `get_image_asset`（`local.rs:888+`）的 `locate_session_jsonl` 链路：每个 project_dir 内部调用改 cross 版本
- [ ] 1.8 改 `get_tool_output`（`local.rs:935+`）同 1.7
- [ ] 1.9 改 `locate_session_jsonl`（`local.rs:1407`）内部 `find_subagent_jsonl(&project_dir, ...)` 调用改 cross 版本
- [ ] 1.10 加 tracing 探针 `tracing::info!(target: "cdt_api::perf", projects_scanned, dirs_with_subagents, candidates_found, total_ms, ...)`
- [ ] 1.11 跑 `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] 1.12 跑 `cargo fmt --all`

## 2. 后端跨 `project_dir` subagent 装载集成测试（`cdt-api`）

- [ ] 2.1 新建 `crates/cdt-api/tests/cross_project_subagent.rs`，建 `tempfile::tempdir` 模拟 `<tmpdir>/projects/-ws-my-proj/` + `<tmpdir>/projects/-ws-my-proj-wt-feat-x/<root_uuid>/subagents/agent-<sub_uuid>.jsonl`（fixture 路径用纯字母数字 + `-`，避 Windows NTFS 禁用字符）
- [ ] 2.2 编写 fixture JSONL（root session 主 jsonl + subagent jsonl 含 `sessionId=root_uuid` + `agentId=sub_uuid` + 真实 message 形态）
- [ ] 2.3 用 `LocalDataApi::new_with_*` 注入 tmpdir 路径，调 `get_session_detail("-ws-my-proj", root_uuid)`，断言 `chunks` 内某 AIChunk 的 `subagents[0].sessionId == sub_uuid`
- [ ] 2.4 加 `CROSS_PROJECT_SUBAGENT_SCAN=false` 路径回归测试（同一 fixture，断言 subagent 不被装载，对应 Task 仍为未解析）
- [ ] 2.5 性能测试：建 50 个 project_dir（含 1 个命中），断言总耗时 < 50 ms
- [ ] 2.6 跑 `cargo test -p cdt-api --test cross_project_subagent`

## 3. 后端 `list_repository_groups` / `get_worktree_sessions` IPC（`cdt-api`）

- [ ] 3.1 `crates/cdt-api/src/ipc/traits.rs` 的 `DataApi` trait 加 `list_repository_groups() -> Result<Vec<RepositoryGroup>, ApiError>` 默认方法（fallback 调 `list_projects` 单成员 group 包装）
- [ ] 3.2 `DataApi::get_worktree_sessions(group_id, pagination) -> Result<PaginatedResponse<SessionSummary>, ApiError>` 改为非默认（已有 stub 在 `local.rs:1234`，提升为 trait 方法）
- [ ] 3.3 `LocalDataApi` 加 `worktree_grouper: WorktreeGrouper<LocalGitIdentityResolver>` 字段；`new()` 内部自动初始化；新增 `new_with_worktree_grouper<G: GitIdentityResolver>(...)` 构造器供测试注入 fake resolver
- [ ] 3.4 实现 `LocalDataApi::list_repository_groups`：调 `scanner.scan().await` + `worktree_grouper.group_by_repository(projects).await`，序列化为 `Vec<RepositoryGroup>` 返回
- [ ] 3.5 实现 `LocalDataApi::get_worktree_sessions(group_id, pagination)`：先调 `list_repository_groups` 定位 group，再并发拉每个 worktree 的 sessions（用 `list_sessions(project_id, ...)`），合并按 `lastModified` 倒序，应用分页，给每条加 `worktreeId` / `worktreeName` 字段。group 未命中返 `not_found` 错误
- [ ] 3.6 `crates/cdt-api/src/http/routes.rs` 加 `GET /api/repository-groups` 路由；已有 `/api/worktrees/{group_id}/sessions` 路由（`:83-84`）接通 `get_worktree_sessions` 实现
- [ ] 3.7 跑 `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] 3.8 跑 `cargo fmt --all`

## 4. Tauri command 与 IPC contract 同步（`src-tauri` / `cdt-api`）

- [ ] 4.1 `src-tauri/src/lib.rs` 加两个 Tauri command 包装：`list_repository_groups` + `get_worktree_sessions`
- [ ] 4.2 `src-tauri/src/lib.rs::invoke_handler![..]` 宏注册两个新 command
- [ ] 4.3 `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` 加 `"list_repository_groups"` + `"get_worktree_sessions"`
- [ ] 4.4 加 `list_repository_groups_returns_camelcase_array` contract test：构造 fake LocalDataApi 调 IPC 路径，断言 JSON 字段名是 `isMainWorktree` / `gitBranch` / `mostRecentSession` / `totalSessions`
- [ ] 4.5 加 `get_worktree_sessions_returns_paginated_response` contract test：断言响应形态是 `{ items, nextCursor, total }`
- [ ] 4.6 跑 `cargo test -p cdt-api --test ipc_contract`

## 5. 前端 API 与 fixture（`ui`）

- [ ] 5.1 `ui/src/lib/api.ts` 加 `RepositoryGroup` / `Worktree` / `RepositoryIdentity` interface（字段名 camelCase 与后端对齐）
- [ ] 5.2 `ui/src/lib/api.ts` 加 `listRepositoryGroups(): Promise<RepositoryGroup[]>` 与 `getWorktreeSessions(groupId, pagination): Promise<PaginatedResponse<SessionSummary>>` 函数
- [ ] 5.3 `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` 加两个新 command
- [ ] 5.4 `ui/src/lib/__fixtures__/multi-project-rich.ts` 扩 mockIPC handler 覆盖两个新 command；构造含 2 个 worktree 的 fixture group
- [ ] 5.5 新建 `ui/src/lib/__fixtures__/repository-groups.ts` 专用 fixture：单 worktree group + 多 worktree group + standalone project 各一个
- [ ] 5.6 跑 `npm run check --prefix ui`
- [ ] 5.7 跑 `npm run test:unit --prefix ui`（验证 fixture mock + API 形态）

## 6. 前端 Sidebar grouped 渲染（`ui`）

- [ ] 6.1 `ui/src/lib/components/Sidebar.svelte` 改 `loadProjects()` 调 `listRepositoryGroups()` 替换 `listProjects()`，state 改为 `repositoryGroups: $state<RepositoryGroup[]>`
- [ ] 6.2 `ui/src/lib/components/SidebarHeader.svelte` 移植原版 `groupWorktreesBySource` 不需要（后端已聚合好），改为直接 `{#each repositoryGroups as group (group.id)}` 渲染
- [ ] 6.3 加 `sidebarStore.svelte.ts::expandedGroupIds: Set<string>` 维护折叠/展开状态；`sidebarStore.activeWorktreeId` 替换原 `activeProjectId`（命名变更，逐处迁移）
- [ ] 6.4 渲染逻辑：单成员 group 平铺为单行；多成员 group 渲染为可展开行，含 chevron + worktree 数量徽章；展开后子列表按 main 优先 + mostRecent 倒序（已在后端排序）
- [ ] 6.5 Worktree 子项展示：worktree.name + worktree.gitBranch（branch icon + 名）+ 相对时间 + sessions 数量
- [ ] 6.6 点击 worktree 子项触发 `sidebarStore.activeWorktreeId = wt.id` + 调 `getWorktreeSessions(group.id, ...)` 注入 SessionList
- [ ] 6.7 初次加载默认选中"最近活动 group 的 main worktree"，并展开该 group
- [ ] 6.8 dev-only URL 参数 `?mode=flat` fallback：在 `if (import.meta.env.DEV && new URLSearchParams(location.search).get("mode") === "flat")` gate 下走旧 flat 渲染分支
- [ ] 6.9 跑 `npm run check --prefix ui`
- [ ] 6.10 跑 `RUN_BUNDLE_TESTS=1 npm run test:unit --prefix ui` 验证 production bundle DCE 仍 mock-free

## 7. 前端 e2e + 单测（`ui`）

- [ ] 7.1 新建 `ui/e2e/sidebar-grouped.spec.ts` Playwright 测试：fixture multi-project-rich 加载后断言多 worktree group 折叠/展开行为
- [ ] 7.2 e2e 测试：点击 worktree 子项触发 SessionList 切换
- [ ] 7.3 单测 `ui/src/lib/__tests__/sidebarStore.test.ts` 加 `expandedGroupIds` / `activeWorktreeId` 状态机测试
- [ ] 7.4 跑 `just test-e2e`

## 8. 验证与收尾

- [ ] 8.1 跑 `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] 8.2 跑 `cargo fmt --all`
- [ ] 8.3 跑 `cargo test --workspace`
- [ ] 8.4 跑 `npm run check --prefix ui` + `npm run test:unit --prefix ui` + `just test-e2e`
- [ ] 8.5 跑 `openspec validate worktree-support-and-cross-project-subagent --strict`
- [ ] 8.6 启动 `just dev`：打开真实 session `83886886-0eca-49f7-b1a1-1b878783856a`，确认 chunk 内的 Task tool 显示 SubagentCard 而非 raw tool；sidebar 中 `claude-devtools-rs` 显示为一组，下挂多个 worktree（主 + `sidebar-click-replace` 等）
- [ ] 8.7 design.md 完成后调 `Agent({ subagent_type: "codex:codex-rescue", prompt: ... })` 审 design.md 的 D1-D7 决策合理性
- [ ] 8.8 实现完成 push commit 后调同样的 codex subagent 二审实际代码：跨目录算法 + grouped UI 数据流
- [ ] 8.9 修完 codex 找到的 bug 跑第二轮 codex 验证
- [ ] 8.10 archive 这个 change：`openspec archive worktree-support-and-cross-project-subagent -y`，archive commit 作为 PR 最后一个 commit
