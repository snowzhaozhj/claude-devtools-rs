# push-events Specification (delta)

## MODIFIED Requirements

### Requirement: Stream detected errors to subscribers

系统 SHALL 在数据 API 层上暴露一个 in-process 订阅机制，让宿主 runtime（例如 Tauri 应用）能够接收自动通知 pipeline 产出的新检测错误，无需轮询持久化通知存储。

#### Scenario: Tauri runtime subscribes and forwards to renderer
- **WHEN** Tauri runtime 在应用 setup 时调用 `subscribe_detected_errors()`
- **AND** 通知 pipeline 产出一条新的 `DetectedError`
- **THEN** 订阅者持有的接收端 SHALL yield 该 `DetectedError`，宿主可据此向前端 emit 一个事件（例如 `notification-added`）

#### Scenario: Subscription without a watcher attached
- **WHEN** 数据 API 层通过不带 watcher 的构造器实例化（集成测试或仅 HTTP 宿主路径）
- **AND** 调用方调用 `subscribe_detected_errors()`
- **THEN** 调用 SHALL 返回一个永不 yield 的有效接收端（静默 no-op），而非错误

#### Scenario: Multiple subscribers receive the same error
- **WHEN** 两个独立订阅者各自调用 `subscribe_detected_errors()`
- **AND** pipeline 产出一条 `DetectedError`
- **THEN** 两个订阅者 SHALL 各自**恰好**收到一次同一条 `DetectedError`

### Requirement: Emit session metadata updates

系统 SHALL 在数据 API 层上暴露 in-process 订阅机制 `subscribe_session_metadata()`，返回一个可接收 `SessionMetadataUpdate` 的接收端。`SessionMetadataUpdate` SHALL 携带 `projectId` / `sessionId` / `title` / `messageCount` / `isOngoing` / `gitBranch`（序列化时为 camelCase）。Tauri host SHALL 把该订阅桥接到 webview，向前端 emit `session-metadata-update` 事件。

`list_sessions` 的骨架阶段 SHALL 对每条 `(session_id, jsonl_path)` 先调用 `try_lookup_cached_metadata`（lookup-only fast-path：查 MetadataCache + `FileSignature` 等价校验 + stale 实时合成 `isOngoing`，**不**触发扫描）。命中条 SHALL 在骨架阶段直接 inline 填回 `title` / `messageCount` / `isOngoing` / `gitBranch` 真实值，且 SHALL NOT 入 `page_jobs`（即不 spawn 后台扫描、不推送对应 update）；未命中场景包括 cache miss、stat 失败、`FileSignature` 不等（mtime / size / identity 任一不等）—— 任一未命中条 SHALL 入 `page_jobs` 走原后台扫描路径，扫完通过广播推送 update。

骨架阶段的 lookup 并发度 SHALL 通过限流机制（上限 8）控制，与后台扫描使用同一上限常量。后台扫描自身的并发度 SHALL 同样被限流（固定上限 8），避免 50+ 文件同时打开；每次 `list_sessions(projectId, pagination)` 触发新扫描前 SHALL 取消同一 `(projectId, cursor)` 维度上一轮未完成的扫描，避免**同分页**的事件串扰；不同 `cursor` 的扫描 SHALL 并存而互不 abort。扫描范围 SHALL 限定为本次 `list_sessions` 返回页中的**未命中** sessions；实现 MUST NOT 因为请求第一页而后台扫描完整项目历史。

cache 全命中场景下 `page_jobs` 为空时 `list_sessions` SHALL 跳过 spawn 后台扫描分支，**不**触碰 active_scans 注册表。

active_scans 注册表的 key SHALL 为 `(projectId, cursor)` 组合编码字符串。同 key 抢占 + per-key generation cleanup 的 race-free 语义不变。

**后台扫描按 backend kind 分流**：cache miss 后 dispatch 函数 SHALL 按 context 的 backend kind 选择：

- **Local backend**：调既有 per-session 扫描路径
- **SSH backend**：调 batched helper，工作流为：
  1. 一次 read_dir_with_metadata 操作拿全 dir entry metadata（限流 ≤ 8 并发）
  2. build path → metadata 索引
  3. 逐条 page_jobs：索引命中 → lookup_with_known_signature cache 命中 → 广播现值；mismatch → spawn sub-task 走 cache wrapper miss 路径
  4. dir read 失败 SHALL fallback 到 per-session 路径

两条路径 SHALL 共享 active_scans 注册表、限流机制（上限 8）、context_generation 与 root_generation 双轴 race-free 校验、广播形态。所有广播调用前 SHALL 校验 root_generation **与** context_generation 同时匹配，否则 silent drop。

batched helper 内部的 mismatch sub-task SHALL 通过 JoinSet 持有，顶层 abort 时 sub-task 跟随 drop 自动 abort。JoinSet cleanup 循环 SHALL 显式处理 join 错误。

**SSH backend 全命中广播例外**：SSH ctx 下即使骨架阶段全命中（SSH 路径 need_background_validation 恒为 true），SSH 路径仍 SHALL spawn batched task 异步校验并广播 cache 现值；Local backend 全命中仍 SHALL 不 spawn 不广播。

#### Scenario: 订阅接收当前页未命中条的元数据更新

- **WHEN** 调用方先 `subscribe_session_metadata()` 取得 receiver
- **AND** 随后调用 `list_sessions("projectA", { pageSize: 3, cursor: null })`，所有 session 均 cache miss
- **THEN** receiver SHALL 在扫描完成后**最多**收到 3 条 `SessionMetadataUpdate`

#### Scenario: Tauri host emit session-metadata-update

- **WHEN** Tauri host 在 setup 订阅并产出 `SessionMetadataUpdate`
- **THEN** webview SHALL 通过 `listen("session-metadata-update", ...)` 收到 camelCase payload

#### Scenario: 同 projectId 同 cursor 的新扫描取消旧扫描

- **WHEN** 同 cursor 的 `list_sessions` 正在扫描中，调用方再次调用（同 cursor）
- **THEN** 旧扫描 SHALL 被 abort；新扫描只扫新页未命中 sessions

#### Scenario: 同 projectId 不同 cursor 的扫描并存互不 abort

- **WHEN** page 1 正在扫描，调用方调 page 2
- **THEN** page 1 扫描 SHALL 继续运行；page 2 启动独立扫描

#### Scenario: 后台扫描并发度限制

- **WHEN** 扫描任务在并发处理某页 50 个 cache-miss session 文件
- **THEN** 同一时刻打开的 JSONL 文件句柄数 SHALL 不超过 8

#### Scenario: 骨架 lookup 并发度限制

- **WHEN** 骨架阶段对 50 个 session 并发执行 `try_lookup_cached_metadata`
- **THEN** 同一时刻进行 stat 操作的 future 数 SHALL 不超过 8

#### Scenario: Local backend cache 命中时骨架直接带值且零 emit

- **WHEN** Local context 下已有 cache 命中，再次调用 `list_sessions`（文件未变）
- **THEN** 骨架阶段 SHALL 直接携带真实元数据
- **AND** receiver SHALL 在短时间内（如 300 ms）**不**收到任何新的 `SessionMetadataUpdate`

#### Scenario: Local backend cache 全命中时不触发 spawn 不触碰 active_scans

- **WHEN** Local context 下所有 session 都 cache 命中
- **THEN** SHALL NOT spawn 后台扫描，SHALL NOT 改动 active_scans

#### Scenario: SSH backend cache 全命中仍 spawn batched 校验

- **WHEN** SSH context 下所有 session 都 trust cached 命中，但 SSH need_background_validation 恒为 true
- **THEN** 仍 SHALL spawn batched 校验并广播 cache 现值

#### Scenario: lookup stat 失败 fallback 到后台扫描

- **WHEN** `try_lookup_cached_metadata` 内 stat 返回错误
- **THEN** 函数 SHALL 返回 None，该 session 入后台扫描

#### Scenario: SSH ctx 后台校验走 batch read_dir_with_metadata

- **WHEN** SSH context 下 page_jobs 非空
- **THEN** dispatch SHALL 走 batched helper；首先调一次 read_dir_with_metadata

#### Scenario: SSH ctx batch helper dir read 失败时 fallback

- **WHEN** batched helper 调 read_dir_with_metadata 返回错误
- **THEN** SHALL fallback 到 per-session 路径，日志记录让运维可见

#### Scenario: Local ctx 后台扫描走既有 per-session 路径不变

- **WHEN** Local context 下 page_jobs 非空
- **THEN** dispatch SHALL 走 per-session 路径，不走 batched
