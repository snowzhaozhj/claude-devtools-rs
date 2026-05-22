## MODIFIED Requirements

### Requirement: Emit session metadata updates

系统 SHALL 在 `LocalDataApi` 上暴露 in-process 订阅机制 `subscribe_session_metadata()`，返回一个 `broadcast::Receiver<SessionMetadataUpdate>`。`SessionMetadataUpdate` SHALL 携带 `projectId` / `sessionId` / `title` / `messageCount` / `isOngoing` / `gitBranch`（序列化时为 camelCase）。Tauri host SHALL 把该订阅桥接到 webview，向前端 emit `session-metadata-update` 事件。

`list_sessions` 的骨架阶段 SHALL 对每条 `(session_id, jsonl_path)` 先调用 `try_lookup_cached_metadata`（lookup-only fast-path：查 `MetadataCache` + `FileSignature` 等价校验 + `is_session_stale(mtime)` 实时合成 `isOngoing`，**不**触发扫描）。命中条 SHALL 在骨架阶段直接 inline 填回 `title` / `messageCount` / `isOngoing` / `gitBranch` 真实值，且 SHALL NOT 入 `page_jobs`（即不 spawn 后台扫描、不通过 `broadcast::Sender<SessionMetadataUpdate>` 推送对应 update）；未命中场景包括 cache miss、`tokio::fs::metadata` stat 失败、`FileSignature` 不等（mtime / size / identity 任一不等）—— 任一未命中条 SHALL 入 `page_jobs` 走原后台扫描路径，扫完通过 broadcast 推送 update。

骨架阶段的 lookup 并发度 SHALL 通过 `Semaphore(METADATA_SCAN_CONCURRENCY=8)` 限流，与后台扫描使用同一上限常量。后台扫描自身的并发度 SHALL 同样被限流（固定上限 8），避免 50+ 文件同时打开；每次 `list_sessions(projectId, pagination)` 触发新扫描前 SHALL 取消同一 `(projectId, cursor)` 维度上一轮未完成的扫描，避免**同分页**的事件串扰；不同 `cursor` 的扫描 SHALL 并存而互不 abort（典型场景：page 1 与 page 2 的并发扫描相互独立）。扫描范围 SHALL 限定为本次 `list_sessions` 返回页中的**未命中** sessions；实现 MUST NOT 因为请求第一页而后台扫描完整项目历史。

cache 全命中场景下 `page_jobs.is_empty()` 时 `list_sessions` SHALL 跳过 `tokio::spawn(scan_metadata_for_page(...))` 分支，**不**触碰 `active_scans` 注册表——既有的 abort + generation + insert race-free 抢占逻辑（详见本 spec `Session list pagination avoids duplicate full scans` 与历史 codex 二轮二审）由"cache miss 时进入 spawn 分支"路径自然继承。

`active_scans` 注册表的 key SHALL 为 `(projectId, cursor)` 组合编码字符串（实现以 `format!("{project_id}|{cursor_or_empty}")`，`|` 字符为 reserved 分隔符；当前 cursor 由 offset 数字字符串生成，不会冲突）。同 key 抢占 + per-key generation cleanup 的 race-free 语义不变。

**后台扫描按 `ContextId.backend_kind` 分流**（change `ssh-batch-readdir-with-metadata` 引入）：cache miss 后 `list_sessions` 调用 `tokio::spawn(scan_metadata_for_page_dispatch(...))` 而非直接 `scan_metadata_for_page`。dispatch 函数 SHALL 按 `context_id.backend_kind` 选择：

- **Local backend**：调既有 `scan_metadata_for_page`（per-session via fs trait，每条 session task 内部 `extract_session_metadata_cached` 走 `fs.stat` + cache miss 调 scanner）
- **SSH backend**：调新 helper `scan_metadata_for_page_batched`，工作流为：
  1. 一次 `fs.read_dir_with_metadata(project_dir)` 拿全 dir entry metadata（acquire 全局 Semaphore permit 限流 ≤ 8 并发）
  2. build `by_name: HashMap<PathBuf, FsMetadata>`
  3. 逐条 page_jobs：若 `by_name` 含对应 path → 调 `MetadataCache::lookup_with_known_signature(&ctx, path, &sig)`；命中 → broadcast 既有 cache 值的 `SessionMetadataUpdate`（`is_ongoing = entry.messages_ongoing`，与 `extract_session_metadata_cached` SSH 跳 stale check 分支语义一致）；mismatch 或 path missing → spawn 单 task 走既有 cache wrapper miss 路径（`extract_session_metadata_cached`）
  4. dir read 失败 SHALL fallback 到 `scan_metadata_for_page`（保证功能正确性，性能退化为既有 PR-D 形态），SHALL 通过 `tracing::warn!(target: "cdt_api::perf", ...)` 让运维侧可见

两条路径 SHALL 共享 `active_scans` 注册表（同形 `ScanEntry { generation, handle, context_id }`）、`Semaphore(METADATA_SCAN_CONCURRENCY=8)` 限流、`context_generation` 与 `root_generation` 双轴 race-free 校验、broadcast 形态（`SessionMetadataUpdate { project_id, session_id, title, message_count, is_ongoing, git_branch, group_id }`）。所有 `tx.send(SessionMetadataUpdate)` 调用前 SHALL 校验 `root_generation.load() == expected_root_generation` **与** `context_generation.load() == expected_context_generation` 同时成立，否则 silent drop（避免把 reconfigure 前的旧 root / 旧 ctx 状态推给前端）—— 对 batched 路径的命中分支同样 SHALL 满足（codex commit 二审 #1 修订：fs.read_dir_with_metadata await 期间 root 可能被 reconfigure）。

`scan_metadata_for_page_batched` 内部的 mismatch sub-task SHALL 通过 `JoinSet` 持有，顶层 batch task abort 时 sub-task 跟随 JoinSet drop 自动 abort（tokio 语义），**SHALL NOT** 重复注册到 `active_scans`。JoinSet `join_next` cleanup 循环 SHALL 显式处理 `JoinError`（至少 `tracing::error!`），避免 sub-task panic 时静默吞掉"某些 session 永远没 metadata"症状（codex commit 二审 #2 修订）。

**SSH backend 全命中 broadcast 例外**：SSH ctx 下即使 `try_lookup_cached_metadata` hot path 全命中（`page_jobs.is_empty()` 不成立——本规约下文 Scenario "Cache 全命中时不触发 spawn 不触碰 active_scans" 仅约束 Local backend，SSH backend `need_background_validation = is_remote || cached_meta.is_none()` 在 local.rs:896 实现），SSH 路径仍 SHALL spawn batched task 异步校验 cache freshness 并 broadcast 命中条 cache 现值——这是 SSH "cache 不信新鲜度"语义的必要补丁；Local backend 全命中仍 SHALL 不 spawn 不 broadcast 与既有 PR-A/B 契约一致。

#### Scenario: 订阅接收当前页未命中条的元数据更新

- **WHEN** 调用方先 `subscribe_session_metadata()` 取得 receiver
- **AND** 随后调用 `list_sessions("projectA", { pageSize: 3, cursor: null })`，响应页包含 3 个 session，**所有** session 在 `MetadataCache` 中均为 miss（如冷启动场景）
- **THEN** receiver SHALL 在扫描完成后**最多**收到 3 条 `SessionMetadataUpdate`，每条携带对应 sessionId 的真实 `title` / `messageCount` / `isOngoing` / `gitBranch`

#### Scenario: Tauri host emit session-metadata-update

- **WHEN** Tauri host 在 `setup` 调用 `subscribe_session_metadata()` 并在后台 task 内订阅
- **AND** 后端产出 `SessionMetadataUpdate { project_id: "p", session_id: "s", title: Some("T"), message_count: 12, is_ongoing: false, git_branch: Some("main") }`
- **THEN** webview SHALL 通过 `listen("session-metadata-update", ...)` 收到 payload `{ projectId: "p", sessionId: "s", title: "T", messageCount: 12, isOngoing: false, gitBranch: "main" }`（camelCase）

#### Scenario: 同 projectId 同 cursor 的新扫描取消旧扫描

- **WHEN** `list_sessions("projectA", { pageSize: 20, cursor: null })` 正在扫描中（后台有未完成任务，至少一条 cache miss 进入 spawn 分支）
- **AND** 调用方再次调用 `list_sessions("projectA", { pageSize: 20, cursor: null })`（**同 cursor**，典型场景：silent 刷新或重复触发同一页加载），新页中有 cache miss 条触发新扫描
- **THEN** 旧扫描任务 SHALL 被 abort，未完成的 session 元数据 SHALL NOT 再被推送；新扫描 SHALL 只扫描新响应页中的未命中 sessions

#### Scenario: 同 projectId 不同 cursor 的扫描并存互不 abort

- **WHEN** `list_sessions("projectA", { pageSize: 20, cursor: null })`（page 1）正在扫描中
- **AND** 调用方紧接着调用 `list_sessions("projectA", { pageSize: 20, cursor: "20" })`（page 2，典型场景：Sidebar 首次加载后 `queueMicrotask(() => maybeLoadMoreSessions(true))` 自动补满视口），新页中有 cache miss 条
- **THEN** page 1 扫描任务 SHALL **继续运行**，page 1 内未完成的 session 元数据 SHALL 通过 broadcast 正常推送；同时 page 2 SHALL 启动独立扫描任务推送其未命中 session 的 update

#### Scenario: 切 project 不主动 abort 旧 project 扫描

- **WHEN** `list_sessions("projectA", ...)` 后台扫描进行中，调用方紧接着调用 `list_sessions("projectB", ...)`
- **THEN** projectA 的扫描 SHALL **继续运行**至完成，旧 project 的 `SessionMetadataUpdate` 仍会被 broadcast；前端 listener 已按 `payload.projectId !== selectedProjectId` 过滤，UI 不受影响

#### Scenario: 后台扫描并发度限制

- **WHEN** 扫描任务在并发处理某页 50 个 cache-miss session 文件
- **THEN** 同一时刻打开的 JSONL 文件句柄数 SHALL 不超过 8（通过 `tokio::sync::Semaphore` 或等价机制限流）
- **AND** SSH backend 走 `scan_metadata_for_page_batched` 时，顶层 batch task 的 `fs.read_dir_with_metadata` 也 SHALL 占用同一 Semaphore permit，与其它 in-flight scan 共享 8 上限（避免新 batch 路径绕过限流）

#### Scenario: 骨架 lookup 并发度限制

- **WHEN** `list_sessions("projectA", { pageSize: 50, cursor: null })` 骨架阶段对 50 个 session 并发执行 `try_lookup_cached_metadata`
- **THEN** 同一时刻进行 `tokio::fs::metadata` stat 的 future 数 SHALL 不超过 8（通过与后台扫描共享的 `METADATA_SCAN_CONCURRENCY=8` 上限）

#### Scenario: 无 watcher 构造器下 subscribe 安全

- **WHEN** `LocalDataApi` 通过不带 watcher 的构造器实例化（集成测试路径）
- **AND** 调用方 `subscribe_session_metadata()`
- **THEN** 返回有效 `broadcast::Receiver`；`list_sessions` 仍能正常推送（broadcast 不依赖 watcher）

#### Scenario: Cache 命中时骨架直接带值且零 emit

- **WHEN** 调用方先 `subscribe_session_metadata()` 取得 receiver
- **AND** 已对 "projectA" 调用过一次 `list_sessions`，期间 `MetadataCache` 已写入该页所有 session 的元数据
- **AND** 在 session jsonl 文件 mtime/size 未变化的前提下，再次调用 `list_sessions("projectA", { pageSize: 3, cursor: null })`
- **THEN** 第二次 `list_sessions` 返回的 `SessionSummary[]` SHALL 在骨架阶段直接携带每条的真实 `title` / `messageCount` / `isOngoing` / `gitBranch`（非占位）
- **AND** receiver SHALL 在第二次调用后短时间内（如 300 ms）**不**收到任何新的 `SessionMetadataUpdate`

#### Scenario: Cache 部分命中时未命中条仍走后台扫描

- **WHEN** `list_sessions` 骨架阶段对 3 个 session 调用 `try_lookup_cached_metadata`，其中 2 个命中（`FileSignature` 等价）、1 个 miss（jsonl 文件被追加新消息，size 与 mtime 已变更，`FileSignature` 不等）
- **THEN** 返回的 `SessionSummary[]` 中 2 个命中条骨架阶段 SHALL 已带真实元数据，1 个 miss 条骨架阶段 SHALL 仍为占位（`title=null` / `messageCount=0` / `isOngoing=false`）
- **AND** 该 miss 条 SHALL 入 `page_jobs` 走后台扫描，扫完通过 broadcast 推送 1 条 `SessionMetadataUpdate`；receiver 收到的 update 数 SHALL 为 1（只覆盖 miss 条）

#### Scenario: Local backend cache 全命中时不触发 spawn 不触碰 active_scans

- **WHEN** Local active context 下 `list_sessions` 骨架阶段对所有 session 都 cache 命中（page_jobs 为空，`is_remote = false`）
- **THEN** 实现 SHALL NOT 调用 `tokio::spawn(scan_metadata_for_page_dispatch(...))`
- **AND** SHALL NOT 改动 `active_scans` 注册表（既不 abort 旧 entry 也不 insert 新 entry）
- **AND** receiver SHALL 不收到任何对应该次调用的 `SessionMetadataUpdate`

#### Scenario: SSH backend cache 全命中仍 spawn batched 校验

- **WHEN** SSH active context 下 `list_sessions` 骨架阶段对所有 session 都通过 `lookup_trust_cached` cache 命中（response inline 含真 title），但 `need_background_validation = is_remote || cached_meta.is_none()` 在 SSH 路径恒为 true（local.rs:896 PR-D 实现）
- **THEN** 实现 SHALL 仍把 page_jobs 入 spawn 调 `scan_metadata_for_page_dispatch` → SSH 走 `scan_metadata_for_page_batched`
- **AND** batched 路径全命中条 SHALL 通过 `MetadataCache::lookup_with_known_signature` 命中直 broadcast cache 现值给 receiver——这是 SSH "cache 不信新鲜度"语义的必要补丁（外部进程改远端文件需要后台校验 mismatch 触发 cache miss → SSE 推差量）
- **AND** receiver SHALL 在二次调用后短时间内收到 N 条 SessionMetadataUpdate（N = session 数）——形态与首次冷启动 broadcast 相同，前端按 sessionId 增量 patch UI 不受 race 影响

#### Scenario: lookup stat 失败 fallback 到后台扫描

- **WHEN** `try_lookup_cached_metadata` 内 `tokio::fs::metadata(path).await` 返回 `Err`（罕见 IO 错误）
- **THEN** 函数 SHALL 返回 `None`
- **AND** 该 session SHALL 入 `page_jobs` 走后台扫描，由 `extract_session_metadata_cached` 内部的 uncached 路径处理（详见 `extract_session_metadata` 按 `FileSignature` 缓存 Requirement）

#### Scenario: SSH ctx 后台校验走 batch read_dir_with_metadata 而非 per-session stat

- **WHEN** `list_sessions` 在 SSH active context 下入 `page_jobs` 非空 → spawn `scan_metadata_for_page_dispatch` task
- **THEN** dispatch SHALL 检查 `context_id.backend_kind == FsKind::Ssh` 走新 helper `scan_metadata_for_page_batched`
- **AND** batched helper SHALL 首先调一次 `fs.read_dir_with_metadata(project_dir)` 拿全 dir entry metadata（SFTP READDIR reply 1 RTT 含 entry attrs）
- **AND** 对每条 `(session_id, jsonl_path)` page_job：若 dir metadata 含对应 path → `MetadataCache::lookup_with_known_signature(&ctx, jsonl_path, &FileSignature::from_fs_metadata(meta))` 命中 → 直 broadcast `SessionMetadataUpdate { is_ongoing: entry.messages_ongoing, ... }`（**SHALL NOT** 调 `fs.stat` / `fs.open_read`）
- **AND** mismatch / 新增 / dir metadata 缺该 path → spawn sub-task 调既有 `extract_session_metadata_cached` 走 cache wrapper miss 路径（`fs.stat` + `fs.open_read`）

#### Scenario: SSH ctx batch helper dir read 失败时 fallback per-session

- **WHEN** `scan_metadata_for_page_batched` 调 `fs.read_dir_with_metadata(project_dir)` 返 `Err`
- **THEN** 函数 SHALL fallback 到既有 `scan_metadata_for_page`（per-session via fs trait）继续异步刷新——保证功能正确性
- **AND** SHALL 通过 `tracing::warn!(target: "cdt_api::perf", project_id = %project_id, ...)` 让运维侧可见
- **AND** fallback 路径下 page_jobs 每条 session 仍能被推 SessionMetadataUpdate（性能退化为既有 PR-D 形态：N 次串行 stat），无功能丢失

#### Scenario: Local ctx 后台扫描走既有 per-session 路径不变

- **WHEN** `list_sessions` 在 Local active context 下入 `page_jobs` 非空 → spawn `scan_metadata_for_page_dispatch` task
- **THEN** dispatch SHALL 检查 `context_id.backend_kind == FsKind::Local` 走既有 `scan_metadata_for_page`（per-session via fs trait），**不**走 batched 路径
- **AND** 行为契约与本 Requirement 既有 Scenario "订阅接收当前页未命中条的元数据更新" / "Cache 部分命中" 等完全一致——Local backend SHALL NOT 受本 change 影响
