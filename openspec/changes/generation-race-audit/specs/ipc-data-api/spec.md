# ipc-data-api spec delta

## MODIFIED Requirements

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
