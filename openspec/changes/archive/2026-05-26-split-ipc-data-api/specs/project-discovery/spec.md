# project-discovery Specification (delta)

## ADDED Requirements

### Requirement: Resolve project id from session id alone

`DataApi` trait SHALL 暴露 `find_session_project(session_id: &str) -> Result<Option<String>, ApiError>`，让仅持有 `session_id` 的调用方反查所属 `project_id`。HTTP `GET /api/sessions/:id` 与 trait 内 `get_sessions_by_ids` MUST 走该方法配合 `get_session_detail(project_id, session_id)` 的复合路径，**不**得直接调 `get_session_detail("", session_id)`。

trait 默认实现 SHALL 遍历 `list_projects()` 取每个 `project_id`，依次调 `list_sessions_sync(project_id, { page_size: usize::MAX, cursor: None })`，命中第一个含 `session_id` 的项目立即返回 `Ok(Some(project_id))`；遍历完无命中返 `Ok(None)`。**主会话**（`<projects_dir>/<encoded>/<session_id>.jsonl`）必然能被默认实现命中；subagent jsonl 是否被命中 SHALL 视具体实现的覆盖能力而定（默认实现不强制覆盖）。

`LocalDataApi` SHALL 覆盖默认实现，直接 `read_dir(scanner.projects_dir())` 扫每个 project 子目录，按以下顺序匹配（命中即返回 `Ok(Some(<encoded_project_id>))`）：

1. **主会话快路径**：`<project_dir>/<session_id>.jsonl` 存在。
2. **legacy subagent**：`<project_dir>/agent-<session_id>.jsonl` 存在。
3. **新结构 subagent**：`<project_dir>/<parent>/subagents/agent-<session_id>.jsonl` 存在（任一 parent）。

实现 SHALL 复用既有 `find_subagent_jsonl` helper，与 `LocalDataApi::get_session_detail` 的查找口径完全一致——避免出现"`find_session_project` 命中但 `get_session_detail` 又取不到"的不一致状态。

#### Scenario: 默认实现命中主会话
- **WHEN** 调用方对一个 mock `DataApi` 调 `find_session_project("sid-A")`，`sid-A` 是项目 `proj-1` 下的主会话
- **AND** mock 实现走 trait 默认 `list_projects` + `list_sessions_sync` 路径
- **THEN** 返回 SHALL 为 `Ok(Some("proj-1"))`

#### Scenario: 默认实现找不到时返 None
- **WHEN** 调用方对 mock `DataApi` 调 `find_session_project("sid-ghost")`，所有 project 的 `list_sessions_sync` 都不含该 id
- **THEN** 返回 SHALL 为 `Ok(None)`

#### Scenario: LocalDataApi 直扫 FS 命中主会话
- **WHEN** tmpdir 下构造 `LocalDataApi`，写入 `<projects_dir>/<encoded-A>/sid-1.jsonl`
- **AND** 调用方调 `find_session_project("sid-1")`
- **THEN** 返回 SHALL 为 `Ok(Some("<encoded-A>"))`

#### Scenario: LocalDataApi 命中 subagent jsonl
- **WHEN** tmpdir 下构造 `LocalDataApi`，写入 `<projects_dir>/<encoded-B>/parent/subagents/agent-sid-2.jsonl`
- **AND** 调用方调 `find_session_project("sid-2")`
- **THEN** 返回 SHALL 为 `Ok(Some("<encoded-B>"))`

#### Scenario: LocalDataApi 多 project 命中第一个
- **WHEN** tmpdir 下两个 project 目录都不含目标 sid，第三个含 `sid-3.jsonl`
- **AND** 调用方调 `find_session_project("sid-3")`
- **THEN** 返回 SHALL 为 `Ok(Some("<encoded-的第三个>"))`，不报错且只命中一次

#### Scenario: LocalDataApi 找不到时返 None 不报错
- **WHEN** tmpdir 下所有 project 目录都不含目标 sid
- **AND** 调用方调 `find_session_project("sid-ghost")`
- **THEN** 返回 SHALL 为 `Ok(None)`（**不**得返回 `Err`、**不**得 panic）

#### Scenario: 与 get_session_detail 口径一致
- **WHEN** `find_session_project(sid)` 返回 `Ok(Some(pid))`
- **THEN** 紧接着调 `get_session_detail(pid, sid)` SHALL 成功返回 `SessionDetail`（不**得**返回 `not_found`）；反之，`Ok(None)` 时 `get_session_detail` 任意 `project_id` 调用 SHALL 都返回 `not_found`

### Requirement: Expose git branch on session summary and metadata updates

`SessionSummary` 与 `SessionMetadataUpdate` SHALL 在已有字段集（`sessionId` / `projectId` / `timestamp` / `title` / `messageCount` / `isOngoing`）之外**额外**携带 `git_branch: Option<String>` 字段（IPC 序列化时为 camelCase `gitBranch`）。骨架返回（`list_sessions` 同步阶段）SHALL 为 `None`，真实值由后端异步元数据扫描在 `LocalDataApi::list_sessions` 后台 JoinSet 任务内填充并通过 `session-metadata-update` 事件 push 到前端。

后端取值规则：解析 session JSONL 时 SHALL 遍历 `cdt_parse::ParsedMessage.message.git_branch`，记录**最后一条** `Some(...)` 作为最终值（与原版 `claude-devtools/src/renderer/utils/sessionExporter.ts` 取值方式一致——反映会话最后所在的 git 分支）。session 中所有行的 `git_branch` 都为 `None`（非 git 仓库）时 SHALL 保持 `None`。

`cdt-api/tests/ipc_contract.rs` SHALL 加断言验证 `SessionSummary` 与 `SessionMetadataUpdate` 序列化结果含 `gitBranch` camelCase 字段，与 `messageCount` 等同位。

#### Scenario: list_sessions skeleton has gitBranch null

- **WHEN** caller 调用 `list_sessions("p")`
- **THEN** 同步返回的每个 `SessionSummary` SHALL 含字段 `gitBranch`（值为 `null`，因尚未异步扫描）

#### Scenario: session-metadata-update payload contains gitBranch

- **WHEN** 后端后台扫描某个 session 完毕，最后一行 `git_branch` 为 `Some("feat/foo")`
- **AND** 该 session 通过 `session-metadata-update` 推送
- **THEN** event payload SHALL 含 `gitBranch: "feat/foo"`（camelCase）

#### Scenario: session without any git_branch line

- **WHEN** 后端扫描 session 所有行 `git_branch` 均为 `None`（非 git 项目）
- **AND** 该 session 通过 `session-metadata-update` 推送
- **THEN** event payload `gitBranch` SHALL 为 `null`

#### Scenario: backend takes last non-empty git_branch

- **WHEN** session 内消息行 `git_branch` 序列依次为 `Some("main")` / `None` / `Some("feat/x")` / `Some("feat/y")` / `None`
- **THEN** 该 session 元数据推送的 `gitBranch` SHALL 为 `"feat/y"`（最后一条非空）

#### Scenario: contract test asserts camelCase serialization

- **WHEN** `cargo test -p cdt-api --test ipc_contract` 执行
- **THEN** 断言 `SessionSummary { git_branch: Some("main"), ... }` 序列化为 JSON 后 SHALL 含字段名 `"gitBranch"`，且 `SessionMetadataUpdate` 同样

### Requirement: Expose repository group queries

系统 SHALL 暴露 `list_repository_groups()` IPC：把 `ProjectScanner::scan()` 结果通过 `WorktreeGrouper::group_by_repository` 聚合为 `Vec<RepositoryGroup>`，每个 group 含 `id` / `identity` / `name` / `worktrees[]` / `mostRecentSession` / `totalSessions` 字段。Worktree 排序 SHALL 按 `is_main_worktree` 优先、再按 `most_recent_session` 倒序（已在 `WorktreeGrouper` 内部实现）。Group 排序 SHALL 按 `mostRecentSession` 倒序。

序列化 SHALL 使用 camelCase（`isMainWorktree`、`gitBranch`、`mostRecentSession`、`totalSessions`、`createdAt`）。

#### Scenario: 列出多 worktree 仓库分组
- **WHEN** 同一 git 仓库下存在主 worktree 与一个用户开的附加 worktree，且两者都有 sessions
- **THEN** `list_repository_groups()` SHALL 返回一个 group，`worktrees` 数组含两项，`worktrees[0].isMainWorktree=true`、`worktrees[1].isMainWorktree=false`

#### Scenario: 独立项目作为单成员分组
- **WHEN** 一个 project 路径无 git 元数据（不属任何 worktree）
- **THEN** `list_repository_groups()` SHALL 返回一个 group，`worktrees` 数组含该项目一项，`identity` 为 `null`

#### Scenario: 序列化 camelCase
- **WHEN** `list_repository_groups()` 返回结果被序列化为 JSON
- **THEN** 字段名 SHALL 为 `isMainWorktree` / `gitBranch` / `mostRecentSession` / `totalSessions` / `createdAt`（不是 snake_case）

### Requirement: Expose worktree sessions query

系统 SHALL 实现 `get_worktree_sessions(group_id, pagination)` IPC：定位 `group_id` 对应 `RepositoryGroup`，把该 group 下所有 worktree 的 sessions 合并为单一列表，按 `timestamp` 倒序后再应用 `PaginatedRequest`（`pageSize` + `cursor`）。返回 `PaginatedResponse<SessionSummary>`，每个条目 SHALL 额外携带 `worktreeId` / `worktreeName` 字段以便 UI 标注归属。

`pageSize == 0` 时 SHALL 立即拒绝（`ApiError::validation`），`pageSize` 不再被静默 clamp 为 1，避免隐藏调用方错误参数。

未命中 `group_id` 时 SHALL 拒绝（`ApiError::not_found`）。

错误形态遵循既有项目约定：trait / HTTP 层产 `ApiError { code, message }` 结构化错误；Tauri command wrapper 沿用 `Result<_, String>` —— 把 `ApiError` 通过 `to_string()` 序列化为含错误前缀的人类可读字符串（与 `list_sessions` / `get_session_detail` 等既有 command 一致），结构化 `code` 字段仅在 HTTP `axum::IntoResponse` 路径暴露。

Tauri command 入参 SHALL 与既有 `list_sessions` 风格一致——顶层 `groupId: string` + `pageSize?: number` + `cursor?: string`，**不**嵌套 `pagination` 对象（保持 IPC 调用形态在所有 paginated command 间一致）。HTTP 路径走 `GET /api/worktrees/{groupId}/sessions?pageSize=...&cursor=...` query string。

#### Scenario: 合并多 worktree sessions 按时间排序
- **WHEN** caller 调用 `invoke("get_worktree_sessions", { groupId: "repo-1", pageSize: 10 })`，repo-1 含两个 worktree 各 5 个 session
- **THEN** 响应 `items` SHALL 含 10 项，按 `timestamp` 倒序排列
- **AND** 每项 SHALL 含 `worktreeId` / `worktreeName` 字段

#### Scenario: 分页继续
- **WHEN** caller 接上一页 `nextCursor` 再调 `invoke("get_worktree_sessions", { groupId, pageSize, cursor: nextCursor })`
- **THEN** 响应 SHALL 返回剩余 sessions，不重复返回上一页内容

#### Scenario: pageSize 为 0 时拒绝
- **WHEN** caller 调用 `invoke("get_worktree_sessions", { groupId: "g1", pageSize: 0 })`
- **THEN** trait 层 SHALL 立刻返 `ApiError::validation(...)`，message 含 `pageSize must be > 0`
- **AND** Tauri command wrapper 把 ApiError 字符串化后让 `invoke` Promise reject 含该 message；HTTP 层走 `IntoResponse` 返 400 + `{code: "validation_error", message}` 结构化 JSON
- **AND** SHALL NOT 静默 clamp 为 1 也 SHALL NOT 返回部分结果

#### Scenario: group_id 不存在
- **WHEN** caller 调用 `invoke("get_worktree_sessions", { groupId: "nonexistent-group", pageSize: 10 })`
- **THEN** trait 层 SHALL 返 `ApiError::not_found(...)`，message 含 group id 标识符
- **AND** Tauri command wrapper 把 ApiError 字符串化后让 `invoke` Promise reject；HTTP 层走 `IntoResponse` 返 404 + `{code: "not_found", message}` 结构化 JSON

### Requirement: Tauri commands for repository groups and worktree sessions

系统 SHALL 通过 Tauri `invoke_handler!` 注册 `list_repository_groups` 与 `get_worktree_sessions` 两个 IPC command，参数与返回类型 SHALL 与上述 IPC trait 方法一致。两个 command 名 SHALL 同步出现在 `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` 与 `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` 两处常量列表中。

#### Scenario: invoke list_repository_groups 返回 camelCase 数组
- **WHEN** 前端调用 `invoke("list_repository_groups")`
- **THEN** 响应 SHALL 为 JSON 数组，每项含 `id` / `identity` / `name` / `worktrees` / `mostRecentSession` / `totalSessions` 字段（camelCase）

#### Scenario: invoke get_worktree_sessions 返回 PaginatedResponse
- **WHEN** 前端调用 `invoke("get_worktree_sessions", { groupId: "g1", pageSize: 20, cursor: null })`（顶层 `pageSize` / `cursor` 与既有 `list_sessions` 一致，不嵌套 `pagination`）
- **THEN** 响应 SHALL 为 `{ items: SessionSummary[], nextCursor: string | null, total: number }` 形态

### Requirement: Expose group session listing via k-way merge pagination

系统 SHALL 实现 `list_group_sessions(group_id, page_size, cursor)` IPC：定位 `group_id` 对应 `RepositoryGroup`，对 group 内 N 个 worktree 各自的 sessions（已在 `WorktreeGrouper` / `ProjectScanner` 层按 `mtime` 倒序）做 **k-way merge 流式分页**，返回 `GroupSessionPage { sessions: Vec<SessionSummary>, next_cursor: Option<String> }`。

实现 MUST 满足：
- **Server 无状态**：cursor 自描述每个 worktree 当前指针位置（`BTreeMap<worktree_id, WorktreeOffset>`，`WorktreeOffset` 枚举为 `NotStarted` / `AfterMtime { mtime_ms, sid }` / `Exhausted`），序列化为 base64(JSON)，重启服务后仍可继续分页
- **全序定义**：全局排序方向为 `(mtime_ms desc, sid asc)`——`mtime_ms` 大的排前，同 `mtime_ms` 时 `sid` 字典序小的排前
- **k-way merge**：内部用 `BinaryHeap<HeapEntry { mtime_ms, sid, worktree_id, idx }>`，`Ord` 实现按全序"排前者优先 pop"（max-heap 视角：`mtime_ms` 大 / 同 mtime 时 `sid` 小为"大"），取 `page_size` 条；每次 pop 后把对应 worktree 的下一条 push 回堆
- **续页定位**：cursor `AfterMtime { mtime_ms: last_mtime, sid: last_sid }` 表示"已消费到 `(last_mtime, last_sid)` 这条"；续页时对每个 worktree 二分定位 SHALL 找到第一条**严格在 `(last_mtime, last_sid)` 之后**的 session，即满足 `(s.mtime_ms < last_mtime) || (s.mtime_ms == last_mtime && s.sid > last_sid)` 的最早条目；MUST NOT 重复返回 `(last_mtime, last_sid)` 自身，MUST NOT 漏掉同 mtime 但 sid 更大的条目
- **不全量收集**：MUST NOT 在产出当前页前把 group 所有 sessions 全部 collect 到 `Vec`（避免 RSS 击穿）；MUST NOT 对每个 worktree 调 `list_sessions_sync(page_size = usize::MAX)` 复用全量路径
- **共享并发限流**：内部并发跑 `ProjectScanner::scan_project_dir` SHALL 使用 `LocalDataApi` 持有的共享 `Arc<Semaphore>`（见 `ProjectScanner shared read semaphore injection`），不得为每个 worktree 新建独立 semaphore
- **页面 SSE detail 触发**：返回页骨架后，SHALL fire-and-forget 触发 `session-metadata-update` 后台拉取，**仅**对当前页 sessions（key on `(project_id /*worktree id*/, session_id)`，复用现有 detail 拉取 active_scans 键空间），借 `active_scans` per-key cancel 在切页 / 切 group / 切 worktree filter 时取消旧拉取
- **worktree filter 通过 cursor 表达**：前端切 worktree filter 为某 worktree `wt-X` 时 SHALL 构造初始 cursor，让所有非 X 的 worktree `WorktreeOffset = Exhausted`，k-way merge 自然只产出 X 的 sessions（server 不感知 filter，纯 cursor 语义复用）
- **(groups, fs, ctx, captured_generation) 同源快照**：`build_group_session_page` 实现 SHALL 通过单一内部 helper（`list_repository_groups_inner`）一次原子调用拿 `(groups, fs, projects_dir, ctx, captured_generation)` 五元组，MUST NOT 各自独立 `await` `list_repository_groups()` 与 `active_fs_and_context_strict()` 两次抽样。理由：两次独立 await 之间可被 `switch_context` / `ssh_connect` / `ssh_disconnect` / `reconfigure_claude_root` 跨过 → 拿到 (OLD ctx 的 groups, NEW ctx 的 fs/ctx) 拼接 → 用 OLD worktree_id 在 NEW fs 上 scan 返空页（用户可观察的 "切换后立刻看 group 是空的"）。inner 内部 scan + grouper 自身仍可被 ssh switch 跨过，但 caller 拿到的五元组保持 self-consistent（要么全 OLD 要么全 NEW），下游 scan 基于同一 fs/ctx，不会出现混合态。
- **后台 metadata scan task spawn 前二次校验**：`build_group_session_page` 在 page 骨架组装完成后、spawn `scan_metadata_for_page` 后台 task **之前** SHALL 短暂获取 `ssh_watcher_ops: Mutex<()>` 锁，并在锁内做 (current_ctx == captured_ctx) **AND** (current_generation == captured_generation) 双重校验：
  - **匹配** → 在锁内完成所有 `tokio::spawn(scan_metadata_for_page(...))` + active_scans.insert，然后释放锁；spawn 过的 task 自身仍按既有约束在 broadcast 前校验 `expected_context_generation` 不变
  - **任一 mismatch** → SHALL 返 `GroupSessionPage` 骨架但 SHALL NOT spawn 任何 metadata scan task；SHALL 在 `tracing` 写 `debug` 留痕
  - 理由：bump-first 顺序使得 inner 拿到的 captured_generation 可能等于 ssh_mgr.switch_context 完成后的 current_generation（同值都为 bumped 后值），此时单 generation 校验会让 task spawn 后 broadcast 校验 `current == expected` 误判为"context 没变"，向新 ctx UI 发旧 ctx update。spawn 前在锁内识别 ctx 变化结构性闭合该 sub-window；spawn 在锁内进行确保 spawn 期间没有 mutate 跑（switch / connect 等也持同锁）。

错误形态：
- `page_size == 0` SHALL 立刻返 `ApiError::validation`，message 含 `pageSize must be > 0`
- `group_id` 不存在 SHALL 返 `ApiError::not_found`
- cursor 反序列化失败 SHALL 视为首页请求（fallback 为 `cursor = null`），并在 tracing 写 `warn` 留痕

序列化 SHALL 使用 camelCase（`pageSize` / `nextCursor` / `worktreeId` / `worktreeName` / `cwdRelativeToRepoRoot`）。

#### Scenario: 首页请求返回 page_size 条按全局 mtime 倒序

- **WHEN** caller 调用 `invoke("list_group_sessions", { groupId: "g1", pageSize: 50, cursor: null })`，g1 含 2 个 worktree 各 30 个 session（mtime 交错）
- **THEN** 响应 `sessions` SHALL 含 50 条，按 `timestamp` 严格倒序
- **AND** 响应 `nextCursor` SHALL 非空，每个 worktree 的 offset 反映已消费到的最后一条

#### Scenario: 续页请求按 cursor 续位

- **WHEN** caller 接上一页 `nextCursor` 再调 `invoke("list_group_sessions", { groupId, pageSize: 50, cursor })`
- **THEN** 响应 SHALL 返回剩余 sessions，不重复返回上一页内容；保持全局 mtime 倒序

#### Scenario: 所有 worktree 流耗尽时 next_cursor 为 null

- **WHEN** caller 续到最后一页，所有 worktree offset SHALL 为 `Exhausted`
- **THEN** 响应 `nextCursor` SHALL 为 `null`

#### Scenario: 同 mtime session 按 sid 字典序稳定排序

- **WHEN** 两个 worktree 各含一条 `mtime_ms = 1000` 但 `sid` 不同的 session（`sidA` < `sidB`）
- **THEN** 全局排序 SHALL 把 `sidA` 排在 `sidB` 之前
- **AND** cursor 记录的 `AfterMtime { mtime_ms: 1000, sid: "sidA" }` SHALL 在续页时跳过 sidA 自身但保留 sidB

#### Scenario: 续页定位边界

- **WHEN** worktree W1 的 sessions 按全序为 `[(2000,"a"), (1000,"b"), (1000,"d"), (500,"c")]`，cursor `AfterMtime { mtime_ms: 1000, sid: "b" }`
- **THEN** 续页 SHALL 跳过 `(2000,"a")` 与 `(1000,"b")`，从 `(1000,"d")` 开始返回
- **AND** SHALL NOT 重复返回 `(1000,"b")`（cursor 自身已消费）
- **AND** SHALL NOT 漏掉 `(1000,"d")`（同 mtime 但 sid > "b"）

#### Scenario: worktree filter via cursor Exhausted

- **WHEN** caller 构造 cursor `{ "wt-X": NotStarted, "wt-other-1": Exhausted, "wt-other-2": Exhausted }` 调 `list_group_sessions`
- **THEN** 响应 sessions SHALL 仅含 `wt-X` 的 sessions（按 mtime 倒序）
- **AND** 续页 cursor 中 `wt-other-1` / `wt-other-2` SHALL 仍为 `Exhausted`

#### Scenario: 不全量收集

- **WHEN** group 含 10 个 worktree 各 100 个 session（共 1000 条），caller 请求 `pageSize: 20`
- **THEN** 实现内部 MUST NOT 把 1000 条 session 全部加载到内存再排序分页
- **AND** 单次请求 RSS 增量 SHALL 在 200 KB 量级（骨架字段 × 1000 条）

#### Scenario: pageSize 为 0 时拒绝

- **WHEN** caller 调用 `invoke("list_group_sessions", { groupId: "g1", pageSize: 0 })`
- **THEN** SHALL 立即返 `ApiError::validation`，message 含 `pageSize must be > 0`

#### Scenario: 损坏 cursor fallback 为首页

- **WHEN** caller 调用 `invoke("list_group_sessions", { groupId: "g1", pageSize: 50, cursor: "invalid-base64" })`
- **THEN** 实现 SHALL fallback 为首页请求（等价 `cursor = null`），返回首页内容
- **AND** SHALL 在 tracing 写 `warn` 留痕

#### Scenario: build_group_session_page 用单一 snapshot 不出现 (groups OLD, fs NEW) 拼接

- **WHEN** active context = `Ssh<host_a>` 且 g1 在 host_a 下有 worktrees `[wt-a-1, wt-a-2]`
- **AND** 调用方 task A 触发 `switch_context("local")` 期间
- **AND** 调用方 task B 并发调 `list_group_sessions("g1", 50, None)`
- **THEN** task B 实现内部 SHALL 仅调用一次 `list_repository_groups_inner` 拿五元组（含 captured_generation），**不得**独立再调 `active_fs_and_context_strict`
- **AND** 拿到的 (groups, fs, ctx) SHALL 来自同一原子抽样（要么全 host_a 要么全 Local）
- **AND** 后续 `scanner.list_sessions(wt_id, ...)` 用五元组里的 fs 扫五元组里 groups 内的 worktree_id —— 不会出现"用 host_a 的 wt-a-1 ID 在 Local fs 上 scan 返空"的混合态错乱

#### Scenario: build_group_session_page 在 ctx mismatch 时返页面骨架但跳 metadata scan spawn

- **WHEN** active context = `Ssh<host_a>` 且 g1 在 host_a 下有 worktrees + sessions
- **AND** 调用方 task B 调 `list_group_sessions("g1", 50, None)`，inner 拿到 (host_a 的 groups, fs, ctx, captured_generation = N+1)，page 骨架 sessions 已组装完
- **AND** 调用方 task A 在 task B 拿 `ssh_watcher_ops` 锁之前完成 `switch_context("local")`（ssh_mgr.active 切到 Local，generation 已 bump 到 N+1，post-mutate 不再 bump）
- **THEN** task B 在锁内识别 `current_ctx = Local` ≠ `captured_ctx = Ssh<host_a>` → mismatch
- **AND** task B SHALL 返回 `GroupSessionPage { sessions: page_sessions, next_cursor }` 给 caller（page 骨架内容是 host_a 的真实数据 self-consistent）
- **AND** task B SHALL NOT spawn 任何 `scan_metadata_for_page` task；session_metadata SSE channel SHALL NOT 收到本次调用产出的 update
- **AND** task A 完成切换后用户在 Local 主动调 `list_group_sessions` 时 SHALL 走全新一轮（拿 Local 的 groups + fs + spawn Local 的 scan task）

### Requirement: Tauri command for list_group_sessions

系统 SHALL 通过 Tauri `invoke_handler!` 注册 `list_group_sessions` IPC command，入参顶层 `groupId: string` + `pageSize?: number` + `cursor?: string`（**不**嵌套 `pagination` 对象，与既有 `list_sessions` 保持一致），返回 `GroupSessionPage`。command 名 SHALL 同步出现在 `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` 与 `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` 两处常量列表中。

HTTP 路径 SHALL 走 `GET /api/repository-groups/{groupId}/sessions?pageSize=...&cursor=...` query string。

#### Scenario: invoke list_group_sessions 返回 GroupSessionPage
- **WHEN** 前端调用 `invoke("list_group_sessions", { groupId: "g1", pageSize: 20, cursor: null })`
- **THEN** 响应 SHALL 为 `{ sessions: SessionSummary[], nextCursor: string | null }` 形态（camelCase）

#### Scenario: command 注册在 invoke_handler 与 mock 列表
- **WHEN** ipc_contract 测试遍历 `EXPECTED_TAURI_COMMANDS`
- **THEN** `list_group_sessions` SHALL 在列表内
- **AND** `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` SHALL 含 `list_group_sessions`

### Requirement: SessionSummary 增加 worktree 元信息字段

系统 SHALL 在 `SessionSummary`（IPC 序列化形态）中增加 `worktreeId: String` / `worktreeName: String` / `groupId: String` / `cwdRelativeToRepoRoot: Option<String>` 四个字段：
- `worktreeId` = 该 session 所属 worktree 的 id（等同底层 `Project.id`，encoded project dir 名）
- `worktreeName` = 该 session 所属 worktree 的展示名
- `groupId` = 该 session 所属 `RepositoryGroup.id`（让前端按 group 维度过滤 SSE event / cache key）
- `cwdRelativeToRepoRoot` = 该 session 所属 `Worktree.cwd_relative_to_repo_root`（`None` 时通过 `#[serde(skip_serializing_if = "Option::is_none")]` 省略）

这四个字段 SHALL 同时出现在 `list_sessions` / `list_group_sessions` / `get_worktree_sessions` 三个 IPC 返回的 `SessionSummary` 中，保证 UI 在任一调用路径下都能拿到 worktree / group 归属信息。

**填值来源（scheme c join）**：IPC handler 在序列化 `SessionSummary` 时，从 `LocalDataApi` 持有的轻量 `worktree_id → (worktree_name, group_id, cwd_relative_to_repo_root)` 映射缓存（`worktree_meta_cache`，`HashMap<String, WorktreeMeta>` flat key）查表填入。`cdt-core::Session` SHALL NOT 持有这些字段，避免 scanner 阶段重走 repo 解析。

**映射缓存刷新约束**：

- 映射缓存 MUST 在 `list_repository_groups` 调用过程中按"captured-snapshot safe refresh"模式更新。`list_repository_groups` 实现 SHALL 通过内部 `list_repository_groups_inner()` 拿到 `(groups, fs, projects_dir, captured_ctx, captured_generation)` 同源快照——`captured_generation` SHALL 在 `active_fs_and_policy()` 完成之**后**立即 load `context_generation`，与 (fs, ctx) 同 snapshot；inner 内后续不修改 generation。
- `list_repository_groups` 在调 `refresh_worktree_meta_cache(&groups)` 之前 SHALL 短暂获取 `ssh_watcher_ops: Mutex<()>` 锁，并在锁内做**双重校验**：
  - 比较 `current_ctx`（锁内通过 `ssh_mgr.active_context_id().await` + `ssh_mgr.provider_and_context_id(...).await` 重建 ContextId；Local active 时 fall through 到 `ContextId::local(self.projects_dir.lock().await.clone())`）与 `captured_ctx` 全等
  - 比较 `current_generation = context_generation.load(SeqCst)` 与 `captured_generation` 全等
  - **两条同时匹配** → 在锁保护下 clear-and-rebuild `worktree_meta_cache`
  - **任一 mismatch** → SHALL skip refresh（safe degrade，旧 mapping 保留至下次 IPC 自然刷新）；SHALL 在 `tracing` 写 `debug` 留痕（`captured`/`current` ContextId + 两个 generation 值）；SHALL 仍把 `groups` 返回给 caller（caller 自身消费 groups 不依赖 cache 状态）
- 后续 IPC（含 `list_sessions` / `list_group_sessions` / `get_worktree_sessions`）SHALL 复用同一映射；缓存失效 SHALL 在 grouper 重跑（filesystem 变化触发 refresh）时整体替换。
- 设计动机：`switch_context` / `ssh_connect` / `ssh_disconnect` / `reconfigure_claude_root` / `shutdown_ssh_all` 五个 context 切换入口 **bump-first** 顺序（先 `context_generation.fetch_add(1, SeqCst)` 再 await `ssh_mgr.switch_context/connect/disconnect` 等 mutate）使得 `context_generation` 在 ssh_mgr / projects_dir 状态 mutate 之前就领先；任何并发 `list_repository_groups` 在 inner 内的 generation pre/post snapshot 都可能落在 ① bump 之后 ② mutate 完成之前 的 window 内 —— pre 与 post 同值（仍是 bumped 后值）误判 "context 未切"，把旧 ctx 的 groups 写入 flat-key cache 污染新 ctx 后续查询。**单 ctx-equality 校验**也无法识别"同 host 快速 disconnect+reconnect 期间 ContextId 等价但 generation bumped 两次"边角；**单 generation-equality**无法识别"reconfigure_claude_root 改 Local projects_dir 但 ssh_mgr.active 不变"边角。**(ctx + generation) 双重校验**结构性闭合两类边角：refresh 路径锁内与 5 处 mutate 入口互斥，锁内读到的状态是稳定真相值。

序列化 SHALL 使用 camelCase。

#### Scenario: 映射缓存随 list_repository_groups 刷新

- **WHEN** caller 调 `invoke("list_repository_groups")` 后再调 `invoke("list_group_sessions", { groupId })`
- **THEN** 后者返回的每条 `SessionSummary` SHALL 含 `worktreeId` / `worktreeName` / `groupId` / `cwdRelativeToRepoRoot`（非 None 时）字段
- **AND** 这些字段 SHALL 与 `list_repository_groups` 返回的 group 内对应 worktree 信息一致

#### Scenario: 缓存未填充时 SessionSummary 缺 worktree 字段

- **WHEN** caller 在首次 `list_repository_groups` 之前调用 `list_sessions(projectId, ...)`（理论上不发生，UI 启动顺序保证 list_repository_groups 在前）
- **THEN** 返回的 SessionSummary `worktreeId` SHALL 等于 `projectId`（fallback：worktree id 就是 project id），`groupId` SHALL 等于 `projectId`（fallback：单 worktree group），`cwdRelativeToRepoRoot` SHALL 为 None

#### Scenario: list_sessions 返回 SessionSummary 含 worktree 字段

- **WHEN** caller 调用 `invoke("list_sessions", { projectId, pageSize: 10 })`
- **THEN** 响应 `items[i]` SHALL 含 `worktreeId` / `worktreeName` / `groupId` 字段（对应该 session 所在 Project / Worktree / Group）

#### Scenario: repo 根 session 省略 cwdRelativeToRepoRoot

- **WHEN** session 所属 worktree `is_repo_root = true`
- **THEN** SessionSummary 序列化 SHALL 省略 `cwdRelativeToRepoRoot` 键
- **AND** SHALL 仍含 `worktreeId` / `worktreeName` / `groupId` 字段

#### Scenario: 子目录 session 含 cwdRelativeToRepoRoot

- **WHEN** session 所属 worktree `is_repo_root = false` 且 `cwd_relative_to_repo_root = Some("crates")`
- **THEN** SessionSummary 序列化 SHALL 含 `"cwdRelativeToRepoRoot": "crates"`

#### Scenario: switch_context 期间并发 list_repository_groups 不污染 worktree_meta_cache

- **WHEN** active context = `Ssh<host_a>` 且 `worktree_meta_cache` 已有 host_a 的 worktree mapping
- **AND** 调用方 task A 触发 `switch_context("local")`，进入 `ssh_mgr.switch_context(None).await` 期间（context_generation 已 bump 到 N+1 但 ssh_mgr 状态尚未切完）
- **AND** 调用方 task B 并发调 `list_repository_groups()`，task B 的 `list_repository_groups_inner()` 拿到 captured_ctx = `Ssh<host_a>` + captured_generation = `N+1`
- **THEN** task A 完成后 worktree_meta_cache 的内容 SHALL 仍是切换前 host_a 的 mapping（被 skip 不清空）
- **AND** task B 调 refresh 路径 SHALL 在 `ssh_watcher_ops` 锁内识别 `current_ctx = Local` ≠ `captured_ctx = Ssh<host_a>` → skip refresh
- **AND** SHALL NOT 出现 "host_a 的 mapping 在 Local active 时被 clear-and-rebuild 入 cache" 的错乱状态
- **AND** task B 仍 SHALL 返回它扫到的 host_a groups 给 caller（不报错；caller 消费这一次返回值不依赖 cache 状态）

#### Scenario: 同 host 快速 disconnect+reconnect 期间 generation bump 触发 skip refresh

- **WHEN** active context = `Ssh<host_a>`，`worktree_meta_cache` 已有 host_a mapping
- **AND** 调用方 task B 进入 `list_repository_groups_inner()`，拿到 captured_ctx = `Ssh<host_a>` + captured_generation = `N`
- **AND** 调用方 task A 在 task B inner 完成之后、wrapper 拿锁之前完成 `ssh_disconnect("host_a")`（generation N→N+1）+ `ssh_connect("host_a")` 同 host 重连（generation N+1→N+2），active 重回 `Ssh<host_a>`，新的 SshSessionResources 已就位
- **THEN** task B wrapper 拿锁后 `current_ctx == captured_ctx == Ssh<host_a>`（同 host ContextId 相等）但 `current_generation == N+2` ≠ `captured_generation == N` → mismatch → skip refresh
- **AND** SHALL NOT 把 task B inner 用旧 host_a session 拿到的 groups 写入 `worktree_meta_cache`（避免覆盖新 session 应有的最新 mapping）

#### Scenario: reconfigure_claude_root 改 Local projects_dir 期间 list_repository_groups 不污染 cache

- **WHEN** active context = `Local`，`projects_dir = /old/dir`，`worktree_meta_cache` 已有 /old/dir 的 mapping
- **AND** 调用方 task B 进入 `list_repository_groups_inner()`，拿到 captured_ctx = `Local { projects_dir: /old/dir }` + captured_generation = `N+1`（reconfigure 已 bump-first 到 N+1）
- **AND** 调用方 task A 在 task B inner 完成之后、wrapper 拿锁之前完成 `reconfigure_claude_root(Some("/new/root"))`，projects_dir 切换到 `/new/dir`
- **THEN** task B wrapper 拿锁后 `current_ctx = Local { projects_dir: /new/dir }` ≠ `captured_ctx = Local { projects_dir: /old/dir }` → ctx mismatch → skip refresh
- **AND** SHALL NOT 把 /old/dir 扫到的 groups 写入 cache 污染 /new/dir 后续查询

