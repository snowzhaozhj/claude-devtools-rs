## Context

PR #177（archived as `unify-session-list-loading-strategy`）让 `list_sessions` IPC 走"骨架 + 后台扫描 + broadcast push patch"模式：IPC return 立即拿到 `SessionSummary` 列表（仅 `sessionId` / `timestamp` 真值，其余字段占位），后台 `tokio::spawn(scan_metadata_for_page)` 用 `JoinSet + Semaphore(8)` 并发解析每条 jsonl 文件的 `title` / `messageCount` / `isOngoing` / `gitBranch`，每解析完一条通过 `broadcast::Sender<SessionMetadataUpdate>` 发出，前端 listener 按 `sessionId` in-place patch。

代价是用户首屏看到列表的**两阶段渲染**：sidebar 先显示 `sessionId` 长 UUID 占位（fallback 文本）→ 数十至数百毫秒后 patch 到达突变为真实 `title`（中文 / 英文）。即使加了 `.metadata-pending` shimmer + `opacity: 0.55` + `transition: opacity 0.15s` fade-in，文字内容从 UUID 突变到标题的视觉跳变在切项目和首次打开时仍然刺眼，用户明确反馈"列表跳动"。

调研已确认（[Explore subagent 报告]）：
- 现有 `LocalDataApi::list_sessions_sync`（`crates/cdt-api/src/ipc/local.rs:1112`）已实现"骨架 + `join_all` 同步等所有 metadata + 填充后返回"语义，但语义是**全量**同步等（不分页），538 session corpus 实测 ~3.4s 不可接受。
- broadcast push 基础设施在翻页 / file-change silent refresh / SSE 自愈 仍依赖，**不**能整体废弃。
- HTTP `GET /api/projects/{id}/sessions` 与 IPC 共用同一 `DataApi::list_sessions` trait 方法。

## Goals / Non-Goals

**Goals:**

- 用户切项目 / 首次打开看到的**首页可见 N 条**（`page_size` 默认 50，前端实际 `SESSION_PAGE_SIZE = 20`）SHALL 是 metadata 真值，**消除"sessionId UUID → 真实 title 突变"的两阶段视觉跳变**。
- 翻页（`cursor != None`） / file-change silent refresh / SSE 自愈仍可走骨架+push，**不**回退已有节奏。
- 首页 IPC return 延迟变化在可控预算内（< 300ms wall, < 0.5 user/real ratio, RSS 不退化），加 perf bench gate 防回归。
- IPC / HTTP 双路径行为对称——HTTP 浏览器端 sidebar 同样体验。

**Non-Goals:**

- 不改 `SessionSummary` JSON / serde 字段名 / 形状。**仅**首页响应中字段**值含义**变更（不再是占位）。
- 不动 `broadcast::Sender<SessionMetadataUpdate>` 基础设施 / Tauri emit 桥 / HTTP SSE 桥（这些翻页 + silent refresh 仍依赖）。
- 不改前端 `Sidebar.svelte` 的 `metadata-pending` CSS / shimmer / pendingMetadataUpdates buffer——首页不触发 → 自然降级，翻页路径仍然兜底。
- 不预扫所有 project 的 metadata（启动期 eager preload 是另一个独立优化空间，本 change 不做）。

## Decisions

### D1：行为分叉点放在 `list_sessions` 内部，按 `cursor` 判定路径

**选**：在 `LocalDataApi::list_sessions` 内部判 `pagination.cursor.is_none()`：
- `None`（首页）：调 `list_sessions_skeleton` 拿骨架后，对**当页所有 items** `futures::future::join_all` 同步等 `extract_session_metadata_cached(...)`，把真值字段填回后再返回；**不** `tokio::spawn(scan_metadata_for_page)`、**不** broadcast emit。
- `Some(_)`（翻页）：保持原"骨架 → spawn scan → broadcast push"路径不变。

**不选**：
- `(A) 加新 trait 方法` `list_sessions_eager_first_page(...)`，Tauri command 层按 `cursor` 分发。代价：trait 公开 API 多一个方法，HTTP handler 也要分发，调用点多两处。**收益不足以抵消额外契约面积。**
- `(B) 改 trait 方法签名`加 `eager: bool` 参数。代价：trait 默认方法需要兼容旧实现，两个 implementer（LocalDataApi + 测试 mock）都要改签名扩散到所有调用点。

D1 选项的好处：trait 公开 API 不变，所有调用点（Tauri command / HTTP handler / 测试 mock）零改动；行为差异完全收敛在 `LocalDataApi` 实现内部，by `cursor` 分发。

### D2：HTTP `GET /api/projects/{id}/sessions` 共享同一 `DataApi::list_sessions`，不动 handler

D1 把分支放 trait 实现内 → HTTP handler 自然继承"首页 eager / 翻页骨架"双路径。HTTP 浏览器端 sidebar 与 Tauri 桌面端 sidebar 行为完全对称。

**好处**：单一 source of truth；HTTP test 现有用例只需更新 expectation（首页 items 含真值），不需要新增分发逻辑测试。

**风险**：HTTP CLI 客户端（非浏览器）原本可能依赖"骨架 → SSE 推送"两阶段流（流式接收 metadata）。本 change **保持**翻页路径上的骨架+SSE 不变，仅首页变 eager；CLI 客户端如需流式可强制 cursor 起步从非 None（罕见用例）。**不在本 change 修复**。

### D3：首页 eager 路径**不** spawn `scan_metadata_for_page`、**不** broadcast emit

理由：首页 items 已含真值在 IPC response 里，前端无需 patch。重复 emit 会让前端 listener 收到对已渲染真值条目的重复"patch"——虽然语义幂等（同值 in-place set），但浪费 broadcast capacity（256）+ Tauri/SSE 序列化开销 + 前端 reactivity 触发。

**例外**：现有 `scan_metadata_for_page`（local.rs:990+）会根据 `(active scans by project + cursor)` key 去重 spawn，避免同 cursor 并发重复扫。本 change 在首页路径直接**不** spawn → 该去重表对首页 cursor=None 不再 insert 条目，对翻页路径无影响。

**测试覆盖**：新增 scenario "首页 cursor=None 路径 SHALL NOT 触发 broadcast emit"（订阅 `subscribe_session_metadata` 在 `list_sessions(cursor=None)` 完成后断 channel 内零消息）。

### D3b：silent refresh 也走 eager（取代原 broadcast 依赖）— **codex design 二审 issue 1**

`ui/src/components/Sidebar.svelte::loadSessions(projectId, silent=true)` 调用 `listSessions(projectId, SESSION_PAGE_SIZE)`，等同于 `cursor=None` 首页请求。原 design 的 D3 + 现有前端 `mergeSilentMetadata` 行为（旧 metadata 优先，依赖后续 broadcast 覆盖）会导致 silent refresh 后已有 session 的 `title` / `messageCount` 被旧值压住——因为 eager 路径不再 emit broadcast。

**新决策**：silent refresh 走 eager 路径无差别处理——response 已含真值。前端 `sessionMerge.ts::mergeSilentMetadata` SHALL 改为"response 含真值则 response 优先；response 是占位才保留旧值"语义。具体：把现有 `mergeSilentMetadata` 替换为已存在的 `mergeRecoveryResponse`（PR #177 codex 二审 round 5 引入，行为已是"response 真值覆盖 prev stale"）；`applySilentRefresh` 改调 `mergeRecoveryResponse(prev, firstPage)`。

**好处**：silent refresh 也无跳变（与 file-change 触发 silent refresh 后 `title` / `messageCount` 实时同步）；统一 eager 行为模型，不再有"silent 路径依赖 broadcast"的隐性契约。

**代价**：silent refresh wall 从 < 100ms（骨架立返）变 ~150-300ms（首页同步等）。file-change 高频触发场景下感知略增——但 file-change 已有 100ms debounce + 前端 `dedupeRefresh` in-flight 合并，实际并发量可控。

### D4：首页 eager 路径复用现有 `metadata_scan_semaphore`（permits=8）

**选**：复用同一 `Arc<Semaphore>`。

**理由**：
- 单 user 顺序操作时间线：用户切 project → IPC 调用 list_sessions(cursor=None) → 首页 eager 等 → IPC return → 用户开始浏览。**这一刻没有别的 list_sessions 后台 scan 在跑**（前一个 project 的 scan 已完成或被切换 cancel）。
- 翻页路径会 spawn `scan_metadata_for_page` 后台扫，但翻页发生在用户已浏览首页**之后**——首页 eager 已经返回，permits 都空闲。
- 多个 project 并发 list_sessions（实际不会，sidebar 单选项目）才会真竞争 → 现实路径不存在。

**不选**：独立 `eager_page_semaphore`。代价：双重资源池增加复杂度 + 总 permit 数翻倍可能撑爆 fd cache / IO 调度。收益场景（多 project 并发首页）不存在。

### D4b：D4 的前提错误——切 project 时 abort 旧 project 翻页扫描 — **codex design 二审 issue 3**

D4 原说"切项目时没有别的 list_sessions 后台 scan 在跑"。**这是错的**：现有 spec `Emit session metadata updates` Scenario "切 project 不主动 abort 旧 project 扫描" 明确旧 projectA 的翻页扫描在切到 projectB 后**继续运行**——A 还在占着 8 permits 的几个，B 切过来首页 eager 等待会和 A 后台扫描竞争同一 semaphore，permits 全占用时 B 首页等待会被拖慢。

**新决策**：保留 spec "切 project 不主动 abort 旧 project 扫描"为既有约定不动；改在**实现层**给 eager 首页路径**优先级**：复用同一 semaphore 但 eager 路径用 `try_acquire` 立即获取（不可用时降级为正常 acquire），**且** projectB 首页 eager 启动前 SHALL `abort` projectA `active_scans` 中所有 entry——eager 路径开始时主动让出 permits。

**实现要点**（最终见 D4b 修订段，本段表述有误已被覆盖；保留作为决策审计）：
- 真相是 `LocalDataApi::list_sessions(currentProjectId, cursor=None)` 进入 eager 路径前 SHALL 遍历 `active_scans` 中**每个** entry，按 `key.split('|').next()` 解析其 projectId，对 `projectId != currentProjectId` 的所有 entry 调 `abort()`，让所有旧 project 后台 scan 提早释放 permits。**注意**：这与现有 spec "切 project 不主动 abort 旧 project 扫描"看似冲突——重新审视该 Scenario 语义：原意是**翻页路径切翻页路径**不互相 abort（不同 project 的并发翻页扫描独立）；本决策只在 **eager 首页**触发时显式 abort 非当前 project 的旧扫描，符合"切 project 后新 project 优先"的用户意图。Scenario "切 project 不主动 abort 旧 project 扫描" SHALL MODIFIED 为"翻页路径 → 翻页路径切换不互相 abort；首页 eager → 任意非当前 project 翻页扫描会被 abort 让出 permits"。
- 性能 perf bench `perf_eager_first_page` SHALL 含 "多个旧 project 翻页扫描在跑 + 新 project 首页 eager 启动" 的复合场景断 wall 仍在预算内（abort 后 permits 立即可用）。

### D5：首页 page_size 上限——**不**额外限制，由 `pagination.page_size` 自然控制

`pagination.page_size` 默认 50（Tauri command 层 `lib.rs:33`），前端实际 `SESSION_PAGE_SIZE = 20`。一页 20 条同步等 metadata：538 session corpus 单条 ~30-80ms（顺序 read jsonl + 行级 JSON parse），8 并发下 `ceil(20/8) = 3` 批 ≈ 90-240ms wall。50 条 ≈ 200-500ms wall。

**Risks** 见下文 R1：极端大 jsonl（数千 turn）+ 慢磁盘可能让某条 metadata 拖到秒级，整页 wall 拖累。R1 的 mitigation 是 per-session metadata extraction 已有 `extract_session_metadata_cached` LRU cache（命中后 cache hit O(1)），冷启动只穿透一次。

**不选**：硬限 page_size ≤ 20。代价：HTTP 客户端如指定大 page_size 会被截，违反 contract。**让客户端自己控**——前端默认 20 已经合理，超过 20 是显式选择应自负其责。

### D5b：page_size 超过软上限 N 时仅 eager-await 前 N 条，剩余回退骨架 — **codex design 二审 issue 5（中等）**

D5 接受 page_size=50 wall 可能超 300ms 预算的风险——但 perf gate 实测可能拒绝。

**新决策**：定义 `EAGER_FIRST_PAGE_LIMIT: usize = 20`（const，与前端 `SESSION_PAGE_SIZE` 同步）。`list_sessions(cursor=None)` 内部行为：
- `page_size <= EAGER_FIRST_PAGE_LIMIT`：所有 items 同步 await metadata（D1 行为）
- `page_size > EAGER_FIRST_PAGE_LIMIT`：前 `EAGER_FIRST_PAGE_LIMIT` 条同步 await 真值；剩余 `page_size - EAGER_FIRST_PAGE_LIMIT` 条**保留骨架** + spawn `scan_metadata_for_page` 后台扫描 + broadcast emit（与翻页路径同模型）

**好处**：HTTP CLI 客户端如指定 page_size=50 仍能在 ~250ms 内拿到首屏可见 20 条真值，剩余 30 条作为骨架返回 + SSE 异步 patch；wall 不超 300ms 预算。

**Tauri command 层**：默认 page_size=50 不动（向后兼容），实际仍受前端 `SESSION_PAGE_SIZE = 20` 控制；`EAGER_FIRST_PAGE_LIMIT` 兜底防御 HTTP 客户端 / 未来代码改动。

**测试覆盖**：perf bench `perf_eager_first_page` 加 `page_size=50` 用例，断"前 20 条已含真值 + 后 30 条仍骨架"且 wall < 300ms。

### D6：错误降级——`extract_session_metadata_cached` 失败时**保留骨架占位**

`extract_session_metadata_cached(cache, path)` 在文件打不开 / 解析失败 / IO 抖动时返回 `SessionMetadata { title: None, ... }`（已有兜底，session_metadata.rs:152-159）。

首页 eager 路径直接复用：`Option<SessionMetadata>` 为 `None` 或字段为占位时，**保留骨架字段**（与原 broadcast push 路径行为一致）。前端 sidebar 按现有 `.metadata-pending` 仍会触发 shimmer——这是首页中**部分** session 失败时的优雅降级，整页其余 session 真值仍正常渲染，不阻塞 IPC return。

**不选**：失败时整页 IPC fail / panic。代价：538 session 中 1-2 条损坏 jsonl 让整 sidebar 加载失败。

### D6b：失败 metadata 不写 cache + 失败条 spawn deferred broadcast retry — **codex design 二审 issue 6（中等）**

D6 仅说"保留骨架"。但 codex 指出：
- (a) `extract_session_metadata_cached` 内部仍可能在 stat 成功 + 行级 parse 部分成功时缓存一个**字段全占位**的 metadata —— 后续 lookup 命中"占位真值"卡住直到 `FileSignature` 变化（永远卡占位）
- (b) eager 路径不 emit broadcast，失败条永远不被 patch（用户看到一条永久 sessionId UUID 占位）

**新决策**：
- 失败 metadata SHALL NOT 写入 `MetadataCache`——`extract_session_metadata_cached` 在 `extract_session_metadata_with_ongoing` 返回字段全占位（`title=None && message_count=0 && !is_ongoing && git_branch=None`）时 SHALL 跳过 `cache.insert(...)`，让下次重新 attempt
- eager 路径若某条 metadata 解析失败（`Option<SessionMetadata>` 为占位），SHALL spawn 一次单条 `deferred retry`：500ms 后重新调 `extract_session_metadata_cached`（如再次成功 emit broadcast；再失败保持占位）

**实现位置**：`crates/cdt-api/src/ipc/local.rs::list_sessions` eager 分支结尾对失败条统一 spawn 单条 retry future（不阻塞 IPC return）。

### D7：单条 metadata 解析 timeout fallback — **codex design 二审 issue 6（严重 R1）**

D6 / R1 原说"不加 per-session timeout fallback"——但 codex 指出无 timeout 时单条 3s jsonl 会让整页 wall 3+ 秒，超出 perf gate。

**新决策**：eager 路径每条 `extract_session_metadata_cached` 调用包 `tokio::time::timeout(EAGER_PER_SESSION_TIMEOUT, ...)`，`EAGER_PER_SESSION_TIMEOUT: Duration = Duration::from_millis(500)` const。

- 超时 → 该条 SHALL 保留骨架占位 + spawn deferred 单条 retry（无 timeout，最终 emit broadcast；与 D6b 失败路径合并到同一 retry 机制）
- 整页 wall 上限：`ceil(20/8) * 500ms + ε ≈ 1500ms` 最坏；实测 corpus p95 应远低于该值

**好处**：首页 wall 有硬上限，不会被单条慢 jsonl 拖死；perf gate 可信。

**实现位置**：`list_sessions` eager 分支内部 `join_all` 前每个 future 包 timeout。

### D8：trait 文档显式刻入 cursor 分叉契约 — **codex design 二审 issue 1（中等）**

trait `DataApi::list_sessions(...)` 当前 doc comment（`crates/cdt-api/src/ipc/traits.rs:35`）描述"返回骨架 + 异步推送"。本 change 改 `LocalDataApi` 实现按 cursor 分叉但**不**改 trait 签名——风险：未来加 mock implementer 或新 backend 时按旧契约实现（cursor=None 仍返骨架）会破坏前端假设。

**新决策**：在 trait method 上加 doc comment 明确 cursor 分叉契约：
```rust
/// 返回项目下 sessions 列表，行为按 cursor 分叉：
/// - cursor == None（首页）：response items SHALL 含真值 metadata 字段
///   （title / messageCount / isOngoing / gitBranch）；不依赖 broadcast
///   `session-metadata-update` 后续推送。
/// - cursor == Some（翻页）：response items 可为骨架（占位字段），真值通过
///   `subscribe_session_metadata` broadcast 后续推送。
```

future implementer 写新 backend 时按此契约实现。trait 不加 `eager: bool` 参数（D1 已选），契约约束在 doc comment + spec 双层兜底。

### D9：HTTP `cursor=None` 路径取消 SSE-ready gate — **codex design 二审 issue 2（严重）**

现有 `ui/src/lib/transport.ts::BrowserTransport.invokeHttp` 对 `LIST_SESSIONS_LIKE_COMMANDS`（含 `list_sessions` / `list_repository_groups` / `list_worktree_sessions`）添加 `await ensureSseReady()` 前置——最多阻塞 1000ms 等 EventSource OPEN。本 change 让 `list_sessions(cursor=None)` 不再触发 SSE update event，SSE-ready gate 对首页路径**多余**——server-mode 浏览器首屏会因这个不必要 gate 拖到 1000ms 外，超 < 300ms 预算。

**新决策**：`BrowserTransport.invokeHttp` SHALL 按 `cmd + args.cursor` 双判断：
- `cmd === 'list_sessions' && (!args.cursor || args.cursor === null)`：cursor=None 首页路径，跳过 `ensureSseReady()` 直接发 fetch
- `cmd === 'list_sessions' && args.cursor`：翻页路径，仍 await ensureSseReady（broadcast push 路径仍依赖 SSE）
- `cmd === 'list_repository_groups' / 'list_worktree_sessions'`：保留 ensureSseReady（这些命令可能内部触发翻页扫描走 broadcast）
- 其它非 list_sessions 命令不变

**spec delta**：MODIFIED `http-data-api/spec.md::浏览器 client SHALL 在首次 list_sessions 前订阅 SSE` 反映该按 cursor 分叉行为。

**测试**：`ui/src/lib/transport.test.ts` 加用例：`list_sessions(cursor=null)` 不调用 ensureSseReady；`list_sessions(cursor='20')` 调用 ensureSseReady。

### D9b：首页 fire-and-forget 触发 SSE 订阅 + sse-recovered 兜底 — **codex v2 复审 issue 3（严重）**

D9 让首页跳过 `await ensureSseReady()`，但首页 path 仍可能产生：
- timeout / 失败条的 deferred retry → broadcast emit
- `page_size > EAGER_FIRST_PAGE_LIMIT` 的 remainder scan → broadcast emit

如果浏览器 EventSource 还没 OPEN（首次访问首页时极常见），这些 broadcast emit 在 SSE 订阅前发生，patch 永久丢失（且因为首页跳过 `ensureSseReady`，不会触发原有的 timeout-recovery 路径）。

**新决策**：`BrowserTransport.invokeHttp` 在 `cmd === 'list_sessions' && cursor === null` 路径下：
- **不** await ensureSseReady（D9 不变，不阻塞首屏）
- 调用时立即检查 EventSource state：**如果 `source.readyState !== EventSource.OPEN`**，**无条件**设 `sseRecoveryPending = true`（不只是 timeout 路径）。这覆盖三种场景：(a) `CONNECTING` 状态下 1000ms 内成功 OPEN（codex v3 复审 issue 1 的 fast-open 竞态）；(b) `CONNECTING` 状态下 1000ms 后 timeout（原 D9 路径）；(c) `CLOSED` 状态触发 scheduleReconnect 后 OPEN
- **fire-and-forget** 调 `void this.ensureSseReady()`（异步触发 EventSource 建立 / scheduleReconnect 在后台跑）
- 后台 EventSource 真正进入 `OPEN` 时（无论是 1000ms 内成功还是 timeout 后重连），既有 onopen handler 检查 `sseRecoveryPending`，若为 true SHALL 给所有 handler emit 一次 `sse-recovered`（既有逻辑不变）
- 这样无论 EventSource 走哪条路径达到 OPEN，**只要在 fetch 时它不是 OPEN，事后的第一次 OPEN 都会触发 sse-recovered** → UI 兜底 silent refresh

**恢复范围限定**（codex v3 复审 issue 6）：sse-recovered 触发的 silent refresh 走 `loadSessions(currentProjectId, silent=true)` → `listSessions(projectId, SESSION_PAGE_SIZE = 20)` —— 仅恢复**首页前 20 条**。已知边界：
- Sidebar 默认 SESSION_PAGE_SIZE=20，不存在 remainder 21+ 条 → 所有可见列表条目都被恢复 ✓
- HTTP CLI 客户端 / 大 page_size 调用方使用 pageSize > 20 时，21+ 条 remainder 的 broadcast emit 失败 SHALL 由客户端自负（典型 CLI 场景调用方应自己确保 SSE 已订阅再发 list_sessions）。Sidebar 不暴露大 pageSize 入口，所以这是非 sidebar 客户端的已知约束

**测试覆盖**：`ui/src/lib/transport.test.ts` 新增用例：
- 首页 `list_sessions(cursor=null)` 调用后 SHALL 异步触发 ensureSseReady（spy 看到调用但 promise 未 await）
- **fast-open 竞态**：EventSource 在 fetch 后 < 1000ms 内成功 OPEN，emit sse-recovered 给 handler（覆盖 codex v3 issue 1）
- timeout-then-open 路径：EventSource 1000ms 内未 OPEN，timeout 后 reconnect 成功 OPEN，emit sse-recovered（既有路径）

### D6c：失败 metadata 加 60s negative TTL backoff — **codex v2 复审 issue 7（中等）**

D6b 让失败 metadata 不写 cache 是为了不卡占位真值；但代价是永久损坏的 jsonl 在每次 silent refresh / 冷启动都重新尝试解析 + spawn deferred retry，造成持续 spike。

**新决策**：在 `MetadataCache` 内或独立结构维护 `negative_results: Map<sessionId, (FileSignature, Instant)>`，记录解析失败的 sessionId + 当前 `FileSignature` + 失败时刻。`extract_session_metadata_cached` 接受一个 `bypass_negative: bool` 参数（默认 `false`）控制是否跳过 negative TTL 检查：

1. 查正向 cache：命中且 `FileSignature` 等价 → 直接返真值
2. 查 negative_results（**仅当 `bypass_negative=false` 时**）：如果 sessionId 存在 + `FileSignature` 等价 + `Instant::elapsed() < NEGATIVE_TTL = 60s` → 直接返回占位，不调 `extract_session_metadata_with_ongoing`
3. 否则调 `extract_session_metadata_with_ongoing`：
   - 解析失败（字段全占位）→ 写入 negative_results（不写正向 cache）
   - 解析成功 → 移除 negative_results 中该 sessionId entry + 写正向 cache

**关键**：D6b 的 deferred retry 必须用 `bypass_negative=true` 调用——否则 retry 会命中自己刚写的 negative TTL 永远失败（codex v3 复审 issue 3）。具体路径：
- 首页 eager 第一次解析失败 / timeout → 立即写 negative_results（首次进入坏状态）+ spawn deferred retry
- deferred retry（500ms 后）调 `extract_session_metadata_cached(..., bypass_negative=true)` → 强制重新解析；如果 IO 抖动 / partial write 已恢复，retry 成功并移除 negative_results；如果仍失败，**重写** negative_results（更新 `Instant::now()` 续期 60s）

**好处**：永久损坏 jsonl 在 deferred retry 失败后才进入 60s backoff 不持续 spike；短暂故障（IO 抖动、partial write）能被 deferred retry 恢复。

**冷启动 spike 控制**：60s TTL 让首次冷启动在 60s 内不再重试同一损坏 jsonl；60s 后下次访问（自然或 file-change 触发）才重新尝试一次（仍可能再次失败 + 重置 60s）。

### D11：silent refresh + page_size>20 remainder scan dedupe — **codex v2 复审 issue 4（中等）**

D5b 让 `page_size > 20` 的 remainder（21..page_size 条）走骨架 + spawn scan + broadcast。D3b silent refresh 走 `cursor=None` 首页路径——高频 silent refresh（file-change debounce 100ms）会反复触发 remainder scan spawn，如果没有 dedupe 会反复 abort/restart 形成"永远跑不完"循环。

**新决策**：remainder scan 的 `active_scans` key 编码为 `format!("{project_id}|None|remainder")`（区分翻页路径的 `format!("{project_id}|{cursor}")`）。同 key 的新 spawn SHALL **abort 旧 entry + 启动新 entry**——与翻页路径 dedupe 模式完全相同（既有 generation token cleanup 模式 / `scan_metadata_for_page` 内部 generation 实现"同 key 后启动取代前一个"语义）。**不**做"复用 in-flight 不重启"的精细子集复用——简化逻辑，避免 sessionId 子集变化时复用判断错误；abort 旧 entry 已扫完的真值仍可通过 broadcast 推送（已经在飞的 send 不被 abort 影响），未扫完的取消后由新 spawn 接管 remainder 范围。

**前端配合**：silent refresh 入口（前端 file-change 触发）SHALL 由 `dedupeRefresh` 合并 in-flight 期间并发调用——已有机制，不需改前端。后端层 active_scans dedupe 兜底高频触发。

**Scenario 测试**：高频 silent refresh + page_size > 20 → 后端 `active_scans` 中同 `(project, None, remainder)` key 始终至多 1 个 entry；最后一次 silent refresh 的 scan 最终完成；中间被 abort 的 task SHALL NOT 永久挂起或泄漏。

### D4b 修订：abort 旧 project 的来源用 `projectId != selectedProjectId` — **codex v2 复审 issue 5（中等）**

D4b 原写 "abort all `key.starts_with("{projectA}|")`"，但实现入口 `LocalDataApi::list_sessions(projectB, ...)` 并不知道 "projectA" 具体是谁——可能有多个旧 project 翻页扫描在跑。

**新决策**：实现层从 `active_scans` key 中**解析 projectId**（key 格式 `{project_id}|{cursor}` 或 `{project_id}|None|remainder`），与当前 `projectB` 不等的 entry 全部 abort。语义：新 project 首页 eager 启动时，SHALL abort `active_scans` 中**所有** `projectId != currentProjectId` 的 entry。

实现要点：用 `key.split('|').next()` 拿出 projectId 比较即可（项目命名约定 projectId 不含 `|` 字符）。

## Risks / Trade-offs

- **R1 单条慢 metadata 拖累整页** → D7 加 `EAGER_PER_SESSION_TIMEOUT = 500ms` per-session timeout 兜底；超时条进 deferred retry。整页 wall 上限 `ceil(min(page_size, EAGER_FIRST_PAGE_LIMIT) / 8) * 500ms = ~1500ms` worst-case。p50 / p95 由 `extract_session_metadata_cached` LRU cache 让重复访问 O(1) 摊薄。`scripts/run-perf-bench.sh` + 新增 `perf_eager_first_page.rs` 监控 wall p50 / p95；超 baseline 阈值（`+20% wall / +50% user`）即触发 perf gate。perf 预算分两档：**p50 < 300ms / p95 < 500ms / worst < 1500ms**（codex v2 复审 issue 6 区分 average vs worst-case）。
- **R2 首屏感知延迟变长** → 首页 IPC return 从 < 100ms（骨架立即返）变 ≤ 300ms（首页同步等）。前端 sidebar 在 IPC pending 期间显示 SkeletonList（已有），用户感知是"加载状态条 ~200ms → 真值列表"，比当前"骨架占位 ~50ms → patch 跳变 ~200ms 后 → 真值"在视觉**连续性**上更优。`.claude/rules/perf.md` 预算 `首屏 sidebar 可见列表 < 500ms` 仍满足。
- **R3 broadcast capacity 256 vs 翻页路径仍 emit** → 翻页（cursor != None）仍 spawn scan + broadcast emit，capacity 不变。首页 eager 路径不 emit → 总 emit 量下降。**broadcast 不会被首页路径打满。**
- **R4 测试更新成本** → `http_list_sessions_skeleton_then_sse.rs` 现行测试 expectation 是首页骨架 + SSE 推送 → 改为：首页 cursor=None 直接断 items 含真值；新加 cursor=Some 翻页用例覆盖原骨架+SSE 路径。`session_metadata_stream.rs` 加新 scenario "首页 cursor=None 路径不 emit broadcast"。`ipc_contract.rs` contract 形状不变（字段都还在），但**值含义** assertion 强化（首页 title 非 null / messageCount > 0 任一）。
- **R5 前端 Sidebar `metadata-pending` 在首页路径自然降级** → 前端 markup 不需要 CSS / class 触发条件改动。`!session.title && session.messageCount === 0 && !session.isOngoing` class 触发条件在首页 items 含真值时不再 true → shimmer 不显示 → 行为自然消失。翻页路径上 metadata-pending 仍可能短暂触发（race buffer 兜底），保留兼容。**但**：silent refresh 走 eager 路径时 `mergeSilentMetadata` 行为需 D3b 决策反转——前端 `sessionMerge.ts` SHALL 改用 `mergeRecoveryResponse` 让 response 真值覆盖 prev stale。
- **R6 单条 jsonl 极慢拖累首页 wall** → D7 timeout 兜底，`EAGER_PER_SESSION_TIMEOUT = 500ms`。超时条进 deferred retry queue（无 timeout，最终 emit broadcast），前端 `.metadata-pending` shimmer 兜底视觉。整页 wall 硬上限 `ceil(page_size/8) * 500ms`。
- **R7 trait 契约面外溢** → D8 在 trait method doc comment 显式刻入 cursor 分叉契约，未来 implementer（mock / SSH 远端 backend）SHALL 按此实现；spec 与 trait doc 双层契约。

## Migration Plan

不需要数据迁移——纯实现层改动，IPC / HTTP / serde 字段不变。回滚策略：revert single commit / disable feature flag（如有）。

由于 PR #177 已合到 main，本 change 在 main 之上以独立 PR 推进。push 后走标准 N.1-N.4 流水线（push → wait-ci → codex 二审 → archive）。

## Open Questions

- **Q1**：`page_size` 默认值是否调小？已通过 D5b 解决——`EAGER_FIRST_PAGE_LIMIT = 20` 强制只前 20 条 eager，剩余降级骨架。HTTP CLI 客户端兼容。
- **Q2**：首页 eager 路径要不要 emit 一个聚合 `session-metadata-update-batch`（一条消息含整页所有真值）给前端？**决策**：不加。前端 IPC response 已含真值，listener 不需要冗余 batch event；增加 batch event 反而增加前端代码路径。
- **Q3**（codex 二审 issue 7）：spec delta 是否需要单独 Scenario 描述 cache 全命中 / 部分命中 / 全 miss / 全失败 的 eager 路径行为？已加在 ipc-data-api spec delta 内，覆盖 4 种状态。
