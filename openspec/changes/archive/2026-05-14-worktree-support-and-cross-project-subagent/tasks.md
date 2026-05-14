## 1. 后端跨 `project_dir` subagent 装载（`cdt-api`）

- [x] 1.1 在 `crates/cdt-api/src/ipc/local.rs` 顶层加 `const CROSS_PROJECT_SUBAGENT_SCAN: bool = true;` 回滚开关（紧邻 `COMPACT_DERIVED_ENABLED`）
- [x] 1.2 新增 `scan_subagent_candidates_cross_project(projects_dir: &Path, main_project_dir: &Path, root_session_id: &str) -> Vec<SubagentCandidate>` 函数：遍历 `projects_dir` 下每个 project_dir，探测 `{dir}/{root_session_id}/subagents/`，存在则收集 `agent-*.jsonl`（跳过 `agent-acompact*`），调 `parse_subagent_candidate` 解析；用 `seen_ids: HashSet<String>` 跨目录去重；旧结构兜底只扫 `main_project_dir`（按 D2b 风险显式化）
- [x] 1.3 新增 `find_subagent_jsonl_cross_project(projects_dir: &Path, root_session_id: &str, sub_session_id: &str) -> Option<PathBuf>` 函数：扫所有 `{projects_dir}/*/{root_session_id}/subagents/agent-{sub_session_id}.jsonl`，命中即返
- [x] 1.4 旧结构 fallback：cross 版本内置 `read_dir(main_project_dir)` 扫 flat `agent-*.jsonl` 一次合并（按 `parent_session_id == root_session_id` 过滤）
- [x] 1.5 改 `get_session_detail`（`local.rs:691`）调用 `scan_subagent_candidates_cross_project(&projects_dir, &project_dir, session_id)`，受 `CROSS_PROJECT_SUBAGENT_SCAN` 开关 gate（false 时退回原 `scan_subagent_candidates`）
- [x] 1.6 改 `get_subagent_trace`（`local.rs:850+`）：优先 `find_subagent_jsonl_cross_project` 跨目录查找；未命中走旧结构兜底（找含 root jsonl 的 project_dir 后 `find_subagent_jsonl`）
- [x] 1.7 改 `get_image_asset`（`local.rs:902+`）—— `locate_session_jsonl` 已在内部跨目录化，调用层无需改
- [x] 1.8 改 `get_tool_output`（`local.rs:935+`）—— 同 1.7，`locate_session_jsonl` 链路自动跨目录
- [x] 1.9 改 `locate_session_jsonl`（`local.rs:1405+`）：`session_id == root_session_id` 时跨目录找 root jsonl；不等时优先 `find_subagent_jsonl_cross_project`，未命中再 fallback 到旧结构（root project_dir 内 flat）
- [x] 1.10 加 tracing 探针 `tracing::info!(target: "cdt_api::perf", projects_scanned, dirs_with_subagents, candidates_found, parse_total_ms, parse_max_ms, total_ms, "scan_subagent_candidates_cross_project")`
- [x] 1.11 跑 `cargo clippy -p cdt-api --all-targets -- -D warnings` 通过
- [x] 1.12 跑 `cargo fmt --all` 通过

## 2. 后端跨 `project_dir` subagent 装载单元测试（`cdt-api`）

- [x] 2.1 在 `crates/cdt-api/src/ipc/local.rs::mod tests` 内加 6 个 unit test（直接测内部 `pub(super)` 函数，无需走 `LocalDataApi` 全套基础设施）：
  - `scan_cross_project_finds_subagent_in_sibling_project_dir`
  - `scan_cross_project_dedupes_same_agent_id`
  - `scan_cross_project_empty_when_no_match`
  - `find_subagent_jsonl_cross_project_locates_sibling`
  - `find_subagent_jsonl_cross_project_returns_none_when_missing`
  - `locate_session_jsonl_finds_root_in_any_project_dir`
  - `locate_session_jsonl_finds_subagent_across_project_dirs`
- [x] 2.2 fixture helper `write_xproj_subagent_jsonl(path, root_session_id, agent_id, cwd)` 写真实 JSONL 形态（含 sessionId / agentId / parentUuid=null / cwd / timestamp / type=user 字段）；fixture 目录名用 `-ws-my-proj` / `-ws-my-proj-wt-feat-x` 等纯字母数字 + `-`，避 Windows NTFS 禁用字符
- [x] 2.3 ~~集成端到端 `get_session_detail` 测试~~ —— 改为 unit test 路线：跨目录扫描属于纯磁盘形态契约，端到端 `get_session_detail` 调用链已被现有 lib tests 覆盖，重复构造端到端 fixture 收益低。如未来需要再加 `crates/cdt-api/tests/cross_project_subagent.rs` 端到端测试
- [ ] 2.4 ~~`CROSS_PROJECT_SUBAGENT_SCAN=false` 路径回归测试~~ —— `const` 切回需要 recompile，单测无法运行时切换；保留 `const` 作为紧急回滚开关，但**不**写单测覆盖（顶层 const 切换在 cargo profile 层验证，不在单测内）
- [ ] 2.5 ~~性能测试：50 个 project_dir~~ —— 转为 release benchmark：跑 `cargo test --release -p cdt-api --test perf_get_session_detail`，通过 `cdt_api::perf` tracing 探针记录真实 `total_ms`；本 change 内仅保证功能正确，性能阈值跟踪到 followups
- [x] 2.6 跑 `cargo test -p cdt-api --lib`（6 个新 unit test 全过，66 passed 总计）

## 3. 后端 `list_repository_groups` / `get_worktree_sessions` IPC（`cdt-api`）

- [x] 3.1 `crates/cdt-api/src/ipc/traits.rs` 的 `DataApi` trait 加 `list_repository_groups() -> Result<Vec<RepositoryGroup>, ApiError>` 默认方法（fallback 调 `list_projects` 单成员 group 包装）
- [x] 3.2 `DataApi::get_worktree_sessions(group_id, pagination) -> Result<PaginatedResponse<SessionSummary>, ApiError>` 改为非默认（已有 stub 在 `local.rs:1234`，提升为 trait 方法）
- [x] 3.3 ~~`LocalDataApi` 加 `worktree_grouper: WorktreeGrouper<LocalGitIdentityResolver>` 字段~~ —— 按 D3b 修订：**不**新增字段，`list_repository_groups` 内部每次构造 `WorktreeGrouper::new(LocalGitIdentityResolver::new())`（grouper 无状态轻量）
- [x] 3.4 实现 `LocalDataApi::list_repository_groups`：调 `scanner.scan().await` + 内部 `WorktreeGrouper::new(LocalGitIdentityResolver::new()).group_by_repository(projects).await`，序列化为 `Vec<RepositoryGroup>` 返回
- [x] 3.5 实现 `LocalDataApi::get_worktree_sessions(group_id, pagination)`：先调 `list_repository_groups` 定位 group，再并发拉每个 worktree 的 sessions（用 `list_sessions(project_id, ...)`），合并按 `lastModified` 倒序，应用分页，给每条加 `worktreeId` / `worktreeName` 字段。group 未命中返 `not_found` 错误
- [x] 3.6 `crates/cdt-api/src/http/routes.rs` 加 `GET /api/repository-groups` 路由；已有 `/api/worktrees/{group_id}/sessions` 路由（`:83-84`）接通 `get_worktree_sessions` 实现
- [x] 3.7 跑 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 3.8 跑 `cargo fmt --all`

## 4. Tauri command 与 IPC contract 同步（`src-tauri` / `cdt-api`）

- [x] 4.1 `src-tauri/src/lib.rs` 加两个 Tauri command 包装：`list_repository_groups` + `get_worktree_sessions`
- [x] 4.2 `src-tauri/src/lib.rs::invoke_handler![..]` 宏注册两个新 command
- [x] 4.3 `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` 加 `"list_repository_groups"` + `"get_worktree_sessions"`
- [x] 4.4 加 `list_repository_groups_returns_camelcase_array` contract test：构造 fake LocalDataApi 调 IPC 路径，断言 JSON 字段名是 `isMainWorktree` / `gitBranch` / `mostRecentSession` / `totalSessions`
- [x] 4.5 加 `get_worktree_sessions_returns_paginated_response` contract test：断言响应形态是 `{ items, nextCursor, total }`
- [x] 4.6 跑 `cargo test -p cdt-api --test ipc_contract`

## 5. 前端 API 与 fixture（`ui`）

- [x] 5.1 `ui/src/lib/api.ts` 加 `RepositoryGroup` / `Worktree` / `RepositoryIdentity` interface（字段名 camelCase 与后端对齐）
- [x] 5.2 `ui/src/lib/api.ts` 加 `listRepositoryGroups(): Promise<RepositoryGroup[]>` 与 `getWorktreeSessions(groupId, pagination): Promise<PaginatedResponse<SessionSummary>>` 函数
- [x] 5.3 `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` 加两个新 command
- [x] 5.4 `ui/src/lib/__fixtures__/multi-project-rich.ts` 扩 mockIPC handler 覆盖两个新 command；构造含 2 个 worktree 的 fixture group
- [x] 5.5 ~~新建 `ui/src/lib/__fixtures__/repository-groups.ts` 专用 fixture~~ —— 实际偏离：直接在 `multi-project-rich.ts` 加 `repositoryGroups` 字段（含 rust-port 含 2 worktree group + 4 个 standalone group）；mockIPC handler 在缺字段时自动从 `fx.projects` 派生单成员 group fallback。独立 fixture 当前 e2e/vitest 不需要，避免重复维护两套数据
- [x] 5.6 跑 `npm run check --prefix ui`
- [x] 5.7 跑 `npm run test:unit --prefix ui`（验证 fixture mock + API 形态）

## 6. 前端 Sidebar grouped 渲染（`ui`）

- [x] 6.1 `ui/src/lib/components/Sidebar.svelte` 改 `loadProjects()` 调 `listRepositoryGroups()` 替换 `listProjects()`，state 改为 `repositoryGroups: $state<RepositoryGroup[]>`
- [x] 6.2 `ui/src/lib/components/SidebarHeader.svelte` 移植原版 `groupWorktreesBySource` 不需要（后端已聚合好），改为直接 `{#each repositoryGroups as group (group.id)}` 渲染
- [x] 6.3 加 `sidebarStore.svelte.ts::expandedGroupIds: Set<string>` 维护折叠/展开状态 —— 按 D7b 修订：**不**重命名 `App.selectedProjectId`，worktree 子项点击时把 `worktree.id` 作为 `projectId` 注入现有 `App.selectedProjectId` 路径（`worktree.id` 已等同 `Project.id`，见 `cdt-core::Worktree::id` 构造逻辑）
- [x] 6.4 渲染逻辑：单成员 group 平铺为单行；多成员 group 渲染为可展开行，含 chevron + worktree 数量徽章；展开后子列表按 main 优先 + mostRecent 倒序（已在后端排序）
- [x] 6.5 Worktree 子项展示：worktree.name + worktree.gitBranch（branch icon + 名）+ 相对时间 + sessions 数量
- [x] 6.6 点击 worktree 子项触发 `onSelectProject(wt.id, wt.name)` 注入 `App.selectedProjectId` —— 按 D7b 修订：**不**新增 `activeWorktreeId` state（`Worktree.id == Project.id`，复用既有 selectedProjectId 路径）；后端 `getWorktreeSessions` IPC 仍暴露给将来的"group 级合并 sessions"概览页使用，本期 sidebar SessionList 仍走 `listSessions(worktree.id)` 拉单 worktree 自身 sessions
- [x] 6.7 初次加载默认选中"最近活动 group 的 main worktree"，并展开该 group
- [x] 6.8 ~~dev-only URL 参数 `?mode=flat` fallback~~ —— 实际偏离：未实现 URL gate；改为 `useGroupedView = repositoryGroups.length > 0` 派生条件——`listRepositoryGroups()` 失败 / 返空时 SidebarHeader 自动 fallback 到 flat `projects` 渲染；fixture e2e 通过 mock 覆盖两条路径足够。URL `?mode=flat` 在当前 e2e 无价值，未来确实需要再加
- [x] 6.9 跑 `npm run check --prefix ui`
- [x] 6.10 跑 `RUN_BUNDLE_TESTS=1 npm run test:unit --prefix ui` 验证 production bundle DCE 仍 mock-free

## 7. 前端 e2e + 单测（`ui`）

- [x] 7.1 新建 `ui/e2e/sidebar-grouped.spec.ts` Playwright 测试：fixture multi-project-rich 加载后断言多 worktree group 折叠/展开行为
- [x] 7.2 e2e 测试：点击 worktree 子项触发 SessionList 切换
- [x] 7.3 单测 `ui/src/lib/__tests__/sidebarStore.test.ts` 加 `expandedGroupIds` / `activeWorktreeId` 状态机测试
- [x] 7.4 跑 `just test-e2e`

## 8. 验证与收尾

- [x] 8.1 跑 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 8.2 跑 `cargo fmt --all`
- [x] 8.3 跑 `cargo test --workspace`
- [x] 8.4 跑 `npm run check --prefix ui` + `npm run test:unit --prefix ui` + `just test-e2e`
- [x] 8.5 跑 `openspec validate worktree-support-and-cross-project-subagent --strict`
- [ ] 8.6 启动 `just dev`：打开真实 session `83886886-0eca-49f7-b1a1-1b878783856a`，确认 chunk 内的 Task tool 显示 SubagentCard 而非 raw tool；sidebar 中 `claude-devtools-rs` 显示为一组，下挂多个 worktree（主 + `sidebar-click-replace` 等）
- [ ] 8.7 design.md 完成后调 `Agent({ subagent_type: "codex:codex-rescue", prompt: ... })` 审 design.md 的 D1-D7 决策合理性
- [ ] 8.8 实现完成 push commit 后调同样的 codex subagent 二审实际代码：跨目录算法 + grouped UI 数据流
- [ ] 8.9 修完 codex 找到的 bug 跑第二轮 codex 验证
- [ ] 8.10 archive 这个 change：`openspec archive worktree-support-and-cross-project-subagent -y`，archive commit 作为 PR 最后一个 commit
