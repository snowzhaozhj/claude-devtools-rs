## 1. 后端 search_across_projects（D2）

- [ ] 1.1 `cdt-discover/src/session_search.rs`：新增 `SessionSearcher::search_across_projects(project_ids, query, max_results, config)` —— 收集多个 worktree project_dir 的 session 文件，全局 mtime desc 排序后逐文件搜索，缺失目录 warn+skip（D4）
- [ ] 1.2 `cdt-discover/tests/session_search.rs`：新增单测覆盖多 worktree 合并搜索 + 缺失目录降级

## 2. 后端 search_group_sessions IPC command（D1）

- [ ] 2.1 `cdt-api/src/ipc/traits.rs`：DataApi trait 新增 `search_group_sessions(group_id, query)` 签名（返回 `SearchSessionsResult`）
- [ ] 2.2 `cdt-api/src/ipc/local.rs`：LocalDataApi 实现——`list_repository_groups_inner()` 取 group + worktrees + fs + projects_dir，构造 `SessionSearcher`，调 `search_across_projects`
- [ ] 2.3 `src-tauri/src/lib.rs`：新增 `#[tauri::command] search_group_sessions` + 在 `generate_handler!` 注册
- [ ] 2.4 `cdt-api/src/http/routes.rs`：新增 `POST /api/repository-groups/{group_id}/search` route + handler

## 3. 契约测试

- [ ] 3.1 `cdt-api/tests/contract_data.rs`：EXPECTED_TAURI_COMMANDS 加 `"search_group_sessions"`
- [ ] 3.2 `cdt-api/tests/ipc_contract.rs`：count 52→53 + 新增 `search_group_sessions` serialization contract test

## 4. 前端 CommandPalette group 化（D3）

- [ ] 4.1 `ui/src/lib/api.ts`：新增 `searchGroupSessions(groupId, query)` wrapper
- [ ] 4.2 `ui/src/components/CommandPalette.svelte`：项目列表 `listProjects()` → `loadProjectData().projects`；搜索 `searchSessions` → `searchGroupSessions`
- [ ] 4.3 `ui/src/lib/tauriMock.ts`：KNOWN_TAURI_COMMANDS 加 `'search_group_sessions'` + buildHandler case

## 5. 验证

- [ ] 5.1 `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --all`
- [ ] 5.2 `cargo test -p cdt-api -p cdt-discover` 全绿
- [ ] 5.3 `pnpm --dir ui run check` 通过

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
