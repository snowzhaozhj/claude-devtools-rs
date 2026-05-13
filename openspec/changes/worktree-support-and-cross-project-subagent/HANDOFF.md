# Handoff — 2026-05-14 中场交接

## 当前状态

- **分支**：`worktree-cross-project-subagent-and-worktree-view`（已 push 到 origin）
- **worktree 路径**：`.claude/worktrees/cross-project-subagent-and-worktree-view/`
- **commits**（本 change 内）：
  - `e4b5044` chore(openspec): propose（proposal + design + 4 spec delta + tasks）
  - `9b51c7c` fix(cdt-api): subagent 装载支持跨 project_dir（**Change 1 完成**）

## 已做（不要重做）

### Change 1: subagent 跨 `project_dir` 装载（完成）

`crates/cdt-api/src/ipc/local.rs`：

- 顶层加 `const CROSS_PROJECT_SUBAGENT_SCAN: bool = true` 回滚开关（紧邻 `COMPACT_DERIVED_ENABLED`）
- 新增 `scan_subagent_candidates_cross_project(projects_dir, main_project_dir, root_session_id)` —— 扫所有 `{pd}/{root}/subagents/agent-*.jsonl`，HashSet 去重；旧结构 flat `{main_pd}/agent-*.jsonl` 兜底
- 新增 `find_subagent_jsonl_cross_project(projects_dir, root_session_id, sub_session_id)` —— 跨目录定位单个 subagent jsonl
- 改 `get_session_detail`（line 691）、`get_subagent_trace`（line 850+）、`locate_session_jsonl`（line 1405+）调用 cross 版本
- `get_image_asset` / `get_tool_output` 经 `locate_session_jsonl` 链路自动获益
- tracing 探针 `cdt_api::perf` 记录 `projects_scanned` / `dirs_with_subagents` / `candidates_found` / `total_ms`

测试：6 个 lib unit test（`scan_cross_project_*` / `find_subagent_jsonl_cross_project_*` / `locate_session_jsonl_*`）。`cargo test -p cdt-api --lib` 66 passed。`cargo clippy --workspace --all-targets -- -D warnings` 全过。

### design.md codex 二审已修订（按 codex 找到的 6 个漏洞）

`design.md` 内 D1b/D2b/D3b/D4b/D6b/D7b 修订段（**不**删原 D1-D7，保留决策审计）：

- **D1b**：性能预算降级为"加 release benchmark 跟踪"，不强保证 5 ms。`cdt_api::perf` 探针落地后实测；P95 > 100 ms 再考虑反向索引
- **D2b**：旧结构假设降级为**显式风险**。本 change **不**覆盖"旧结构 jsonl 在非主 pd"场景
- **D3b**：⚠️ **改了 Change 2 设计**。`LocalDataApi` **不**新增 `worktree_grouper` 字段。`list_repository_groups` 内部每次 lazy 构造 `WorktreeGrouper::new(LocalGitIdentityResolver::new())`
- **D4b**：⚠️ **改了 Change 2 设计**。URL `?mode=flat` fallback **仅 vite dev server 下有效**。production Tauri 窗口加载本地 `dist/index.html`，URL params 不传。不引入 config / localStorage 开关，仅永久 grouped
- **D6b**：fixture 路径用 `-ws-my-proj-wt-feat-x` 保留；Windows encode 边界靠 `path_decoder` 内单测覆盖
- **D7b**：⚠️ **改了 Change 2 设计**。`App.selectedProjectId` **不**重命名。worktree 子项点击时把 `worktree.id` 注入现有 `App.selectedProjectId` 路径（`worktree.id == Project.id` 既定）。`sidebarStore.svelte.ts` 仅新增 `expandedGroupIds: Set<string>`

## 待做（Change 2，按 tasks.md section 3-8）

### Section 3：后端 IPC（`cdt-api`）

- [ ] 3.1 `crates/cdt-api/src/ipc/traits.rs:25` `DataApi::list_repository_groups() -> Result<Vec<RepositoryGroup>, ApiError>` 加默认方法（fallback 调 `list_projects` 单成员 group 包装）
- [ ] 3.2 `DataApi::get_worktree_sessions(group_id, pagination) -> Result<PaginatedResponse<SessionSummary>, ApiError>`：**改签名**！现在是 `get_worktree_sessions(&self, group_id: &str) -> Result<serde_json::Value, ApiError>`（traits.rs:228），需要加 pagination 参数 + 把返回改为强类型 `PaginatedResponse<SessionSummary>`
- [ ] 3.3 ~~`LocalDataApi` 字段缓存 WorktreeGrouper~~ → **按 D3b**：每次 `list_repository_groups` 内部构造
- [ ] 3.4 `LocalDataApi::list_repository_groups` 实现：`scanner.scan().await` → `WorktreeGrouper::new(LocalGitIdentityResolver::new()).group_by_repository(projects).await`
- [ ] 3.5 `LocalDataApi::get_worktree_sessions(group_id, pagination)`：定位 group → 并发拉每个 worktree 的 sessions → 合并按 `lastModified` 倒序 → 分页 → 每条加 `worktreeId` / `worktreeName`。group 未命中返 `ApiError::not_found`
- [ ] 3.6 `crates/cdt-api/src/http/routes.rs:83-84` 加 `GET /api/repository-groups` 路由
- [ ] 3.7 clippy
- [ ] 3.8 fmt

**关键**：`SessionSummary`（types.rs:48）需要加 `worktreeId` / `worktreeName` 两个 `Option<String>` 字段（`#[serde(default, skip_serializing_if = "Option::is_none")]`），仅 `get_worktree_sessions` 路径填，不破坏现有 IPC contract。

### Section 4：Tauri command 同步

- [ ] 4.1 `src-tauri/src/lib.rs` 加 `list_repository_groups` + `get_worktree_sessions` Tauri command 包装
- [ ] 4.2 `invoke_handler!`（lib.rs:662）注册两个新 command
- [ ] 4.3 `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` 加两条
- [ ] 4.4-4.6 contract test：`list_repository_groups_returns_camelcase_array` + `get_worktree_sessions_returns_paginated_response`

### Section 5：前端 API / fixture

- [ ] 5.1-5.3 `ui/src/lib/api.ts` 加 interface + 函数；`tauriMock.ts::KNOWN_TAURI_COMMANDS` 同步
- [ ] 5.4-5.5 `__fixtures__/multi-project-rich.ts` 扩 + 新建 `__fixtures__/repository-groups.ts`
- [ ] 5.6-5.7 npm check + vitest

### Section 6：前端 Sidebar grouped 渲染（**最大块**）

- [ ] 6.1 `Sidebar.svelte::loadProjects()` 改调 `listRepositoryGroups()`，state `repositoryGroups: $state<RepositoryGroup[]>`
- [ ] 6.2 `SidebarHeader.svelte` 移植原版 `../claude-devtools/src/renderer/components/layout/SidebarHeader.tsx::49-88` `groupWorktreesBySource`（后端已聚合，前端**不需要**重做分组算法）+ lines 513-538 dropdown 渲染
- [ ] 6.3 `sidebarStore.svelte.ts` 仅加 `expandedGroupIds: Set<string>` —— **按 D7b**：`App.selectedProjectId` 不改名，worktree 子项点击时设置 `selectedProjectId = worktree.id`
- [ ] 6.4-6.7 渲染逻辑：单成员 group 平铺 / 多成员折叠 / 默认选中"最近活动 group 的 main worktree" / 展开该 group
- [ ] 6.8 ⚠️ **按 D4b**：dev-only URL `?mode=flat` 仅 `vite serve` 下有效，production Tauri 窗口跳过
- [ ] 6.9-6.10 npm check + bundle test

### Section 7：e2e + 单测

- [ ] 7.1-7.4 Playwright `ui/e2e/sidebar-grouped.spec.ts` + sidebarStore vitest

### Section 8：收尾

- [ ] 8.1-8.6 preflight + 真实 `just dev` 验证
- [ ] 8.7 ~~design.md codex 审~~ → **已完成**（D1b-D7b 修订完）
- [ ] 8.8 push 后调 `Agent({ subagent_type: "codex:codex-rescue" })` 二审实际代码
- [ ] 8.9 修完 codex 找到的 bug 跑第二轮验证
- [ ] 8.10 archive：`openspec archive worktree-support-and-cross-project-subagent -y` 作为 PR 最后一个 commit

## 下个会话怎么继续

1. **checkout 分支**：`git fetch && git checkout worktree-cross-project-subagent-and-worktree-view`
2. **进 worktree**：`cd .claude/worktrees/cross-project-subagent-and-worktree-view/`（CLAUDE.md hook 拦 main 分支 Edit，必须在 worktree 内工作）
3. **告诉 Claude**：
   ```
   继续 change worktree-support-and-cross-project-subagent，Change 1 已完成（commit 9b51c7c）。
   读 openspec/changes/worktree-support-and-cross-project-subagent/HANDOFF.md 拿上下文，
   按 tasks.md section 3-8 推进 Change 2（后端 list_repository_groups / get_worktree_sessions + 前端 grouped sidebar）。
   注意 D3b/D4b/D7b 三处 design 修订。
   ```
4. **第一步建议**：先看 `crates/cdt-discover/src/worktree_grouper.rs::WorktreeGrouper::group_by_repository`（line 103）确认返回 `Vec<RepositoryGroup>` 形态；然后从 traits.rs 加 trait 方法开始
5. **测试基线**：当前 `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` 全过；改动后用同样命令验证
6. **codex 二审**：Change 2 push 后调 `Agent({ subagent_type: "codex:codex-rescue" })` —— 上次需要 Bash 授权才能调，准备好授权

## 已踩过的坑（避免重新撞）

- **CLAUDE.md hook**：main 分支不能 Edit/Write 源码（白名单仅 CLAUDE.md / README.md / .claude/ / .github/ / docs/ / openspec/changes/<slug>/...）→ 必须 `git checkout -b feat/xxx` 或在 worktree 内工作
- **doc_markdown clippy**：注释里 `project_dir` / `session_id` 这类标识符要反引号 `` `project_dir` ``
- **lib test 名字冲突**：`scan_subagent_candidates` 已有同名 helper 在 `mod tests` line 2438，新加的要避名（用了 `write_xproj_subagent_jsonl`）
- **`new` 签名兼容**：`LocalDataApi::new` 被大量 tests 依赖，**不**改签名，扩 `new_with_*` 走 D3b 路径反而更轻量
- **Windows NTFS**：fixture 目录名禁 `: < > " / \ | ? *`，用纯字母数字 + `-`
- **codex 二审需 Bash 授权**：`codex-companion.mjs` 走 Bash 调用，第一次会要授权

## 关键文件清单

- `openspec/changes/worktree-support-and-cross-project-subagent/`
  - `proposal.md` / `design.md`（D1b-D7b 修订完）/ `tasks.md`（section 1-2 ✅ 已勾）
  - `specs/tool-execution-linking/spec.md` —— MODIFIED 跨目录 Scenario
  - `specs/ipc-data-api/spec.md` —— MODIFIED + ADDED `list_repository_groups` / `get_worktree_sessions` / Tauri command Requirements
  - `specs/project-discovery/spec.md` —— MODIFIED `Group projects by git worktree` 补字段约定
  - `specs/sidebar-navigation/spec.md` —— ADDED 全部 Requirements（4 个）
- `crates/cdt-api/src/ipc/local.rs` —— Change 1 改动 + 6 个 unit test
- 待改：`crates/cdt-api/src/ipc/traits.rs` / `crates/cdt-api/src/ipc/types.rs` / `crates/cdt-api/src/ipc/local.rs::list_repository_groups + get_worktree_sessions` / `crates/cdt-api/src/http/routes.rs` / `crates/cdt-api/tests/ipc_contract.rs` / `src-tauri/src/lib.rs` / `ui/src/lib/api.ts` / `ui/src/lib/tauriMock.ts` / `ui/src/lib/__fixtures__/*` / `ui/src/lib/components/Sidebar.svelte` / `ui/src/lib/components/SidebarHeader.svelte` / `ui/src/lib/sidebarStore.svelte.ts` / `ui/e2e/sidebar-grouped.spec.ts`

## 工作量估算（剩余）

- 后端 IPC（section 3-4）：~200 行 + contract test，2-3 小时
- 前端（section 5-7）：~400 行 + e2e + vitest，半天到一天
- codex 二审 + 修 bug + archive PR（section 8）：1-2 小时

总计：1-1.5 天
