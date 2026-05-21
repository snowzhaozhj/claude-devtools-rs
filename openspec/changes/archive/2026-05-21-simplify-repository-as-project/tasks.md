## 1. discovery 层：is_repo_root + cwd_relative_to_repo_root（cdt-discover / cdt-core）

- [ ] 1.1 在 `crates/cdt-core/src/project.rs` 给 `Worktree` 增加 `is_repo_root: bool` + `cwd_relative_to_repo_root: Option<String>` 字段（`#[serde(skip_serializing_if = "Option::is_none")]` 仅对后者）
- [ ] 1.2 在 `crates/cdt-discover/src/worktree_grouper.rs::RepoLookup` 增加 `is_repo_root: bool`，`Default` 实现保守为 `false`
- [ ] 1.3 修 `LocalGitIdentityResolver::resolve_all_fs` / `locate_git_dirs` 计算 `is_repo_root`：仅当 `start` path canonical 与 `repo_root_path`（`common_dir` strip `/.git` suffix 后 canonical）相等时为 `true`；子目录 walk-up 命中同 `.git` SHALL 为 `false`
- [ ] 1.4 修 `WorktreeGrouper::group_by_repository`：拿到 `lookup.identity + project.path` 后纯字符串计算 `cwd_relative_to_repo_root = project.path.strip_prefix(repo_root)`（0 syscall）；fallback `infer_parent_repo_from_worktree_path` 分支下 `is_repo_root=false` 且 `cwd_relative_to_repo_root` 按相同公式算
- [ ] 1.5 修 Worktree 排序：`is_repo_root` 优先（repo 根排前）→ `is_main_worktree` 优先 → `most_recent_session` 倒序
- [ ] 1.6 单测覆盖 `specs/project-discovery/spec.md` 新 Scenario：`主仓子目录 cwd 不被误标为 repo root` + `linked worktree cwd 含 cwd_relative_to_repo_root` + 排序断言
- [ ] 1.7 老 grouper 测试（`two_worktrees_share_one_group` 等）所有 `Worktree { ... }` 字面量同步补 `is_repo_root` / `cwd_relative_to_repo_root` 字段；不破坏既有 Scenario
- [ ] 1.8 `cargo test -p cdt-discover` 全绿
- [ ] 1.9 `cargo test -p cdt-core` 全绿
- [ ] 1.10 perf 回归：`cargo test -p cdt-api --release --test perf_cold_scan -- --ignored --nocapture` apply 前 / 后各跑 3 次取 min，验证 ≤ 100ms（baseline 95ms × 1.05）

## 2. IPC join 层：SessionSummary 的 worktree/group 字段（cdt-api）

按 design D2 scheme c：`cdt-core::Session` **不**加 `cwd_relative_to_repo_root` 字段；该字段仅在 `cdt-core::Worktree` 上（task 1.x 已加），IPC `SessionSummary` 在序列化时通过 group→worktree join 填入。本节实现 join 缓存与字段填值。

- [ ] 2.1 在 `LocalDataApi` 增加 `worktree_meta_cache: Arc<RwLock<HashMap<String /*worktree_id*/, WorktreeMeta>>>` 字段，`WorktreeMeta { worktree_name: String, group_id: String, cwd_relative_to_repo_root: Option<String> }`
- [ ] 2.2 `list_repository_groups` 完成后 SHALL 刷新 `worktree_meta_cache`（遍历所有 group / worktree 重建映射）
- [ ] 2.3 `list_sessions` / `list_group_sessions` / `get_worktree_sessions` 在序列化 `SessionSummary` 时查 `worktree_meta_cache`，填入 `worktreeId` / `worktreeName` / `groupId` / `cwdRelativeToRepoRoot`（非 None 时）
- [ ] 2.4 缓存未填充时（理论上 UI 启动顺序保证 list_repository_groups 在前，不会触发）的 fallback：`worktreeId = projectId`、`groupId = projectId`、`cwdRelativeToRepoRoot = None`
- [ ] 2.5 单测覆盖 `specs/ipc-data-api/spec.md::SessionSummary 增加 worktree 元信息字段` 全部 Scenario（映射缓存刷新 / fallback / list_sessions / repo 根省略 / 子目录含字段）
- [ ] 2.6 `cargo test -p cdt-api` 相关测试全绿

## 3. ipc-data-api 层：共享 read semaphore 注入（cdt-discover / cdt-api）

- [ ] 3.1 `ProjectScanner::new_with_semaphore(projects_dir, fs, semaphore: Arc<tokio::sync::Semaphore>)` 新构造器；原 `new` 改 `#[cfg(test)]` + 内部 `Arc::new(Semaphore::new(SHARED_READ_CONCURRENCY))`
- [ ] 3.2 `LocalDataApi` 增加 `shared_read_semaphore: Arc<Semaphore>` 字段；构造时 `Arc::new(Semaphore::new(SHARED_READ_CONCURRENCY))`
- [ ] 3.3 全 workspace grep `ProjectScanner::new(` 非 cfg(test) 调用点（`crates/cdt-api/src/ipc/local.rs:764-768` 等），改为 `new_with_semaphore(...self.shared_read_semaphore.clone())`
- [ ] 3.4 单测覆盖 `specs/ipc-data-api/spec.md::ProjectScanner shared read semaphore injection` 的 3 个 Scenario（19 并发不击穿 / cfg(test) 可用 new / 生产强制 new_with_semaphore）
- [ ] 3.5 `cargo test --workspace` 全绿

## 4. ipc-data-api 层：list_group_sessions k-way merge IPC（cdt-api）

- [ ] 4.1 `crates/cdt-api/src/ipc/types.rs` 增加 `GroupSessionPage { sessions, next_cursor }` + `GroupCursor` + `WorktreeOffset` 类型（含 serde derive）
- [ ] 4.2 在 `crates/cdt-api/src/ipc/types.rs` 给 `SessionSummary` 增加 `worktree_id: String` / `worktree_name: String` / `cwd_relative_to_repo_root: Option<String>` 字段
- [ ] 4.3 `crates/cdt-api/src/ipc/local.rs` 实现 `LocalDataApi::list_group_sessions(group_id, page_size, cursor)`：
  - 定位 group → 拿到 N worktree id
  - 并发 `scan_project_dir`（共享 semaphore）拿每 worktree 骨架（已 mtime 倒序）
  - parse cursor → 二分定位每个 worktree 的指针起点：
    - `NotStarted` → 从该 worktree 第一条开始
    - `AfterMtime { mtime_ms: last_mtime, sid: last_sid }` → 找第一条满足 `(s.mtime_ms < last_mtime) || (s.mtime_ms == last_mtime && s.sid > last_sid)` 的条目（严格在 cursor 之后，不重复不漏同 mtime 不同 sid）
    - `Exhausted` → 跳过该 worktree（用于 worktree filter 场景）
  - `BinaryHeap<HeapEntry>` 全序按 `(mtime_ms desc, sid asc)`，max-heap 视角 = mtime 大 / 同 mtime 时 sid 小为"大"；取 page_size 条；每 pop 后 push 该 worktree 下一条；无下一条则该 worktree 切 Exhausted
  - 编码 next_cursor（base64 JSON），若所有 worktree Exhausted 则 next_cursor = None
- [ ] 4.4 在 `list_group_sessions` 内 fire-and-forget 触发 SSE detail 推送（仅当前页 sessions，key on `(group_id, session_id)`，复用现有 `active_scans` per-key cancel）
- [ ] 4.5 `crates/cdt-api/src/ipc/traits.rs::DataApi` trait 增加 `list_group_sessions` 默认实现 fallback（其它 impl 默认 `unimplemented` 或 fallback 到 worktree 串行）
- [ ] 4.6 `crates/cdt-api/src/ipc/local.rs` Tauri command wrapper 注册 `list_group_sessions`
- [ ] 4.7 `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` 加 `list_group_sessions`
- [ ] 4.8 `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` 加 `list_group_sessions`
- [ ] 4.9 `crates/cdt-api/src/http.rs` 加 `GET /api/repository-groups/{groupId}/sessions` route
- [ ] 4.10 单测覆盖 `specs/ipc-data-api/spec.md::Expose group session listing via k-way merge pagination` 全部 Scenario（首页 / 续页 / 流耗尽 / 同 mtime 稳序 / 续页定位边界 off-by-one / worktree filter via cursor Exhausted / 不全量收集 / pageSize=0 拒绝 / 损坏 cursor fallback）
- [ ] 4.11 单测覆盖 `Tauri command for list_group_sessions` 2 个 Scenario
- [ ] 4.12 `SessionSummary` 新字段 IPC contract round-trip 测：覆盖 list_sessions / list_group_sessions / get_worktree_sessions 三个 IPC 都含 `worktreeId` / `worktreeName` / `groupId` / `cwdRelativeToRepoRoot`（非 None 时）字段
- [ ] 4.13 修 `get_worktree_sessions` 实现禁用 `usize::MAX` 全量扫描；共享 list_group_sessions 内部逻辑（或委托给同一 helper）；添加 `实现不允许全量扫描` Scenario 单测
- [ ] 4.14 `cargo test -p cdt-api` 全绿
- [ ] 4.15 `cargo clippy --workspace --all-targets -- -D warnings` 全绿
- [ ] 4.16 `cargo fmt --all`

## 5. 配置迁移（**已取消**，apply 阶段反转 D7b）

apply grep 实测 `crates/cdt-config` 与 `ui/src/lib` 都无 `selected_project_id` 持久化字段——`selectedProjectId` 是 UI session-scoped in-memory state，启动 fallback 到"最近活动 group"是现有行为，无需迁移。spec `Migrate persisted selected_project_id on load` Requirement 已移除。详 `design.md::D7b`。

- [x] 5.1 ~~`ConfigManager::load` 增加 migration~~ — 无字段可迁移
- [x] 5.2 ~~迁移幂等~~ — 不适用
- [x] 5.3 ~~单测覆盖 Scenario~~ — Requirement 已删
- [x] 5.4 ~~cargo test -p cdt-config 全绿~~ — 无新改动

## 6. UI 改造（ui/src）

按 design D7 的"sidebar/SSE 用 group id，detail 链路保留 worktree id"分层；session 行 click 时拿 `session.worktreeId` 创建 tab，tab 内 detail IPC 不改。

- [ ] 6.1 `ui/src/lib/api.ts` 增加 `listGroupSessions(groupId, pageSize, cursor)` 客户端 + 新类型 `GroupSessionPage` / `SessionSummary` 的 4 个 worktree 字段
- [ ] 6.2 `ui/src/lib/groupCursor.ts` 新工具模块：`buildFilterCursor(groupWorktrees, selectedWorktreeId)` 构造 server-side filter cursor（非选中 worktree 标 `Exhausted`，base64 JSON 编码）
- [ ] 6.3 `ui/src/components/ProjectSwitcher.svelte`：删除多 worktree group accordion 分支 + `dropdown-group-row` / `dropdown-group-chevron` / `dropdown-group-badge` CSS；多 worktree group 与单 worktree group 走同一 line 117-135 渲染分支，onclick 改为 `selectGroup(group)` 传 group id；删除 `$effect` 内 `expandedGroupIds` 相关逻辑
- [ ] 6.4 `ui/src/lib/sidebarStore.svelte.ts`：删除 `expandedGroupIds` state / `isGroupExpanded` / `toggleGroupExpanded` / `setGroupExpanded`；rename `selectedProjectId` → `selectedGroupId`，持 group id（全局 grep 改名 + 全 import 同步）
- [ ] 6.5 `ui/src/lib/projectDataStore.svelte.ts::fetchProjectData`：默认 `selectedGroupId` 推导取 `group.id` 而非 `group.worktrees[0].id`
- [ ] 6.6 `ui/src/components/Sidebar.svelte::loadSessions` 改为调 `listGroupSessions(selectedGroupId, pageSize, cursor)`；loadMore 用 `nextCursor` 续页；filter 切到具体 worktree 时 cursor 用 `buildFilterCursor` 构造
- [ ] 6.7 `ui/src/components/Sidebar.svelte`：删除 line 836-839 的 `{#if session.cwd}<span class="session-cwd">...</span>{/if}` 整块 + 对应 CSS + `cwdTailLabel` helper（若 helper 无其它消费方）
- [ ] 6.8 `ui/src/components/Sidebar.svelte`：在 session 行 meta 末尾（git branch chip 之后）渲染 cwd hint chip：`{#if session.cwdRelativeToRepoRoot}<span class="session-cwd-hint">…/{lastTwoSegs(session.cwdRelativeToRepoRoot)}</span>{/if}`
- [ ] 6.9 `ui/src/components/Sidebar.svelte`：sidebar 顶部新增 worktree filter 下拉组件（仅当前 group `worktrees.length > 1` 时可见），options 按 spec 排序，filter state session-scoped，切 group 重置为"全部"
- [ ] 6.10 worktree filter 切换：清空 sidebar session 列表 + 重新调 `listGroupSessions(groupId, pageSize, cursor)`，"全部"时 cursor=null，具体 worktree 时 cursor=`buildFilterCursor(...)`；server 端 cursor `Exhausted` 标记天然过滤
- [ ] 6.11 自动补页：若 server-side filter 返回某页 `sessions.length < pageSize` 且 nextCursor 非 null，sidebar 自动续 loadMore 直到填满一屏或 cursor 全 Exhausted
- [ ] 6.12 SSE event filter：`session-metadata-update` event payload 新增 `groupId` 字段（task 7.x 实现后端 emit）；前端按 `payload.groupId === selectedGroupId` 匹配；同时保留 `payload.projectId` 用于 detail 路径
- [ ] 6.13 `ui/src/lib/sessionListStore.svelte.ts`（如存在该 store）：cache key 改为 `(groupId, filterWorktreeId | null)` 组合，切 filter 不串台；缓存命中规则同步更新
- [ ] 6.14 `ui/src/lib/tabStore.svelte.ts`：`tab.projectId` **保持** worktree id 语义（detail API 入参不变）；**新增** `tab.groupId: string` 字段供 sidebar 高亮"该 tab 属于哪个 group"；点击 session 行创建 tab 时同时写入两个字段
- [ ] 6.15 `ui/src/routes/SessionDetail.svelte` 调 `getSessionDetail(tab.projectId, tab.sessionId)` **不变**（仍传 worktree id）
- [ ] 6.16 `ui/src/components/CommandPalette.svelte`：原 `listSessions(selectedProjectId, ...)` 改为 `listGroupSessions(selectedGroupId, pageSize, null)` 拿合并候选；候选 onclick 时按 `candidate.worktreeId` 创建 tab，tab 内 detail 路径走 worktree id
- [ ] 6.17 全 ui grep `selectedProjectId` 确认所有引用已切到 `selectedGroupId`（除了 detail / tab / per-project state 路径明确保留 worktree id 的位置）
- [ ] 6.18 Vitest 单测：worktree filter store 状态机 + `selectedGroupId` 切换 + SSE patch 按 groupId 匹配 + tab 创建写入 projectId/groupId 双字段 + sessionListStore cache key 含 filter 维度 + buildFilterCursor 编码正确
- [ ] 6.19 Playwright e2e：选 multi-worktree group → session 列表合并展示 → 切 worktree filter → 仅显示该 worktree session（自动补页验证）→ 点 session 行打开 SessionDetail → SessionDetail 正常加载 + tab 显示
- [ ] 6.20 `pnpm --dir ui run check` 全绿
- [ ] 6.21 `pnpm --dir ui run test:unit` 全绿
- [ ] 6.22 `pnpm --dir ui run test:e2e` 全绿（按需视环境 / `just test-e2e`）

## 7. SSE event 字段扩展（cdt-api / ui）

- [ ] 7.1 `crates/cdt-api/src/ipc/types.rs::SessionMetadataUpdate` 增加 `group_id: String` 字段（camelCase serialize `groupId`）
- [ ] 7.2 emit 路径（`session_metadata.rs` 等）查找 session 对应 group_id 后写入 event（grouper 输出的 RepositoryGroup 提供 worktree → group 映射）
- [ ] 7.3 IPC contract test 加 `SessionMetadataUpdate.groupId` round-trip
- [ ] 7.4 `cargo test -p cdt-api` 全绿

## 8. 文档与 followups

- [ ] 8.1 更新 `openspec/followups.md`（如有相关 TS divergence 改动）
- [ ] 8.2 `openspec validate simplify-repository-as-project --strict` 全绿
- [ ] 8.3 `just preflight` 一把梭（fmt + lint + test + spec validate）全绿

## N. 发布
- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
