## MODIFIED Requirements

### Requirement: Emit session metadata updates

系统 SHALL 在 `LocalDataApi` 上暴露 in-process 订阅机制 `subscribe_session_metadata()`，返回一个 `broadcast::Receiver<SessionMetadataUpdate>`。`SessionMetadataUpdate` SHALL 携带 `projectId` / `sessionId` / `title` / `messageCount` / `isOngoing` / `gitBranch`（序列化时为 camelCase）。Tauri host SHALL 把该订阅桥接到 webview，向前端 emit `session-metadata-update` 事件。

`list_sessions` 的骨架阶段 SHALL 对每条 `(session_id, jsonl_path)` 先调用 `try_lookup_cached_metadata`（lookup-only fast-path：查 `MetadataCache` + `FileSignature` 等价校验 + `is_session_stale(mtime)` 实时合成 `isOngoing`，**不**触发扫描）。命中条 SHALL 在骨架阶段直接 inline 填回 `title` / `messageCount` / `isOngoing` / `gitBranch` 真实值，且 SHALL NOT 入 `page_jobs`（即不 spawn 后台扫描、不通过 `broadcast::Sender<SessionMetadataUpdate>` 推送对应 update）；未命中场景包括 cache miss、`tokio::fs::metadata` stat 失败、`FileSignature` 不等（mtime / size / identity 任一不等）—— 任一未命中条 SHALL 入 `page_jobs` 走原后台扫描路径，扫完通过 broadcast 推送 update。

骨架阶段的 lookup 并发度 SHALL 通过 `Semaphore(METADATA_SCAN_CONCURRENCY=8)` 限流，与后台扫描使用同一上限常量。后台扫描自身的并发度 SHALL 同样被限流（固定上限 8），避免 50+ 文件同时打开；每次 `list_sessions(projectId, pagination)` 触发新扫描前 SHALL 取消同一 `projectId` 维度上一轮未完成的扫描，避免事件串扰。扫描范围 SHALL 限定为本次 `list_sessions` 返回页中的**未命中** sessions；实现 MUST NOT 因为请求第一页而后台扫描完整项目历史。

cache 全命中场景下 `page_jobs.is_empty()` 时 `list_sessions` SHALL 跳过 `tokio::spawn(scan_metadata_for_page(...))` 分支，**不**触碰 `active_scans` 注册表——既有的 abort + generation + insert race-free 抢占逻辑（详见本 spec `Session list pagination avoids duplicate full scans` 与历史 codex 二轮二审）由"cache miss 时进入 spawn 分支"路径自然继承。

#### Scenario: 订阅接收当前页未命中条的元数据更新

- **WHEN** 调用方先 `subscribe_session_metadata()` 取得 receiver
- **AND** 随后调用 `list_sessions("projectA", { pageSize: 3, cursor: null })`，响应页包含 3 个 session，**所有** session 在 `MetadataCache` 中均为 miss（如冷启动场景）
- **THEN** receiver SHALL 在扫描完成后**最多**收到 3 条 `SessionMetadataUpdate`，每条携带对应 sessionId 的真实 `title` / `messageCount` / `isOngoing` / `gitBranch`

#### Scenario: Tauri host emit session-metadata-update

- **WHEN** Tauri host 在 `setup` 调用 `subscribe_session_metadata()` 并在后台 task 内订阅
- **AND** 后端产出 `SessionMetadataUpdate { project_id: "p", session_id: "s", title: Some("T"), message_count: 12, is_ongoing: false, git_branch: Some("main") }`
- **THEN** webview SHALL 通过 `listen("session-metadata-update", ...)` 收到 payload `{ projectId: "p", sessionId: "s", title: "T", messageCount: 12, isOngoing: false, gitBranch: "main" }`（camelCase）

#### Scenario: 同 projectId 新扫描取消旧扫描

- **WHEN** `list_sessions("projectA", { pageSize: 20, cursor: null })` 正在扫描中（后台有未完成任务，至少一条 cache miss 进入 spawn 分支）
- **AND** 调用方再次调用 `list_sessions("projectA", { pageSize: 20, cursor: "next" })`，新页中有 cache miss 条触发新扫描
- **THEN** 旧扫描任务 SHALL 被 abort，未完成的 session 元数据 SHALL NOT 再被推送；新扫描 SHALL 只扫描新响应页中的未命中 sessions

#### Scenario: 后台扫描并发度限制

- **WHEN** 扫描任务在并发处理某页 50 个 cache-miss session 文件
- **THEN** 同一时刻打开的 JSONL 文件句柄数 SHALL 不超过 8（通过 `tokio::sync::Semaphore` 或等价机制限流）

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

#### Scenario: Cache 全命中时不触发 spawn 不触碰 active_scans

- **WHEN** `list_sessions` 骨架阶段对所有 session 都 cache 命中（page_jobs 为空）
- **THEN** 实现 SHALL NOT 调用 `tokio::spawn(scan_metadata_for_page(...))`
- **AND** SHALL NOT 改动 `active_scans` 注册表（既不 abort 旧 entry 也不 insert 新 entry）
- **AND** receiver SHALL 不收到任何对应该次调用的 `SessionMetadataUpdate`

#### Scenario: lookup stat 失败 fallback 到后台扫描

- **WHEN** `try_lookup_cached_metadata` 内 `tokio::fs::metadata(path).await` 返回 `Err`（罕见 IO 错误）
- **THEN** 函数 SHALL 返回 `None`
- **AND** 该 session SHALL 入 `page_jobs` 走后台扫描，由 `extract_session_metadata_cached` 内部的 uncached 路径处理（详见 `extract_session_metadata` 按 `FileSignature` 缓存 Requirement）
