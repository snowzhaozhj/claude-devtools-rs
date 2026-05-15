## Context

`LocalDataApi` 把每次 `list_sessions(projectId, pagination)` 的后台元数据扫描句柄存到 `active_scans: HashMap<String, ScanEntry>`，key 当前是 `metadata_scan_key(project_id) = project_id`。spec `ipc-data-api/spec.md::Scenario "同 projectId 新扫描取消旧扫描"` 描述：cursor=null 调用扫描中、cursor="next" 第二次调用 → 旧扫描被 abort。

Sidebar 首次加载链路里 `loadSessions(project, false)` 完成后 `queueMicrotask(() => maybeLoadMoreSessions(true))`（`Sidebar.svelte:251`）会自动判断视口剩余空间触发 `loadMoreSessions` → `list_sessions(project, cursor=20)` —— 此时 page 1 的后台扫描还在 in-flight，按 spec 行为 page 1 直接被 abort。结果：page 1 里 cache miss 的 session 永远不会被扫描，前端 `title=null`，UI 退化到 `sessionId.slice(0, 8) + "…"` 占位。

PR #80（`fix(ui): sidebar 偶发显示 session id 前缀根因修复`）误判为前端 listener 注册时序问题；PR #82 修的是 silent 刷新计数跳变；PR #70 修的是分页对齐。三 PR 都没碰到核心：**spec 的 abort 维度本身错误**。

`PaginatedResponse<T>` 已经携带 `total: number`，但 spec `ipc-data-api/spec.md::List sessions uses project-scoped light pagination` 明确 callers 可以把 `total` 当 informational。当前前端 `Sidebar.svelte:419` 的 `totalSessions = sessions.length` 显示已加载条目数，所以翻页时 20 → 40 → 60 跳变。

## Goals / Non-Goals

**Goals:**

- 修正 `active_scans` abort 维度，让首次加载 + 自动补满视口的 page 1/page 2 扫描互不干扰。
- 修正 Sidebar 总数显示口径，避免翻页时数字跳变；保持 silent 刷新拿到的 result.total 也能更新本地 `sessionsTotal`。
- 保持现有"同 cursor 重复请求抢占"行为（典型场景：silent 刷新 page 1 重复触发）不变。

**Non-Goals:**

- 不优化 `scanner.scan()` 的 `extract_session_cwd` IO 成本（这是另一条性能线，独立 change）。
- 不改 `broadcast::Sender` 通道、capacity、emit 协议。
- 不改前端 listener 过滤 `payload.projectId !== selectedProjectId` 的语义（切 project 仍按 projectId 过滤）。
- 不动 cache 全命中 fast-path 的"不触碰 active_scans"语义（`page_jobs.is_empty()` 直接跳过仍成立）。

## Decisions

### D1: `active_scans` key 改为 `(projectId, cursor)` 编码字符串

**选项 A**（采用）：把 `metadata_scan_key(project_id, cursor)` 改写为 `format!("{project_id}|{cursor}")`，cursor=`None` 编码为 `"{project_id}|"`，**保持 `HashMap<String, ScanEntry>` 类型不变**。`scan_metadata_for_page` cleanup 时按完整 key 比较 generation。

**选项 B**：把 `active_scans` 类型改成 `HashMap<String, HashMap<String, ScanEntry>>` 双层。

**取舍**：A 改动量最小（只动两处 key 构造）、与既有 `generation` race-free 设计无缝兼容。B 嵌套结构带来更多 lock 临界区复杂度，没有额外收益。

**风险**：cursor 字符串理论上可能含 `|` 字符。当前 cursor 由 `list_sessions_skeleton` 用 `(offset).to_string()` 生成（纯 ASCII 数字 / `None`），不会有歧义；如果未来 cursor schema 变化要在 doc-comment 注明 reserved 分隔符。

### D2: 切 project 时**不**主动 abort 旧 project 的扫描

**选项 A**（采用）：保留 broadcast emit 不变，前端 listener 已经按 `payload.projectId !== selectedProjectId` 过滤旧 project 的事件，旧 project scan 跑完自己结束、不影响 UI；**不**新增"切 project 即 abort 该 project 全部 cursor 扫描"逻辑。

**选项 B**：在 `list_sessions` 入口检测到 project 切换时遍历 `active_scans` abort 该 project 全部 cursor 扫描。

**取舍**：A 实现简单，符合现有 channel + listener 过滤的双保险设计；B 引入 "project switch detection" 状态，但旧扫描的 broadcast 也只是几十条 fire-and-forget 消息，capacity 256 完全够用，CPU 消耗也只是后台 task 的 stat / parse。**用户实际不可见**。除非未来发现旧 project 扫描真的成为瓶颈，再走单独 change 加。

### D3: 前端 `sessionsTotal` 派生从后端 `result.total` 取值

**选项 A**（采用）：新增 `let sessionsTotal = $state<number>(0);`，`loadSessions` 非 silent 路径设 `sessionsTotal = result.total`、silent 路径合并完后也设 `sessionsTotal = result.total`（silent 拿到的 total 也是后端最新全量计数，覆盖一次没问题）；`loadMoreSessions` 翻页**不**更新 `sessionsTotal`（页内 total 不会变）。`totalSessions` 派生从 `sessions.length` 改为 `sessionsTotal`。

**选项 B**：保留 `sessions.length`，要求后端"骨架阶段一次返回全部 session id"。

**取舍**：A 与 spec `ipc-data-api/spec.md::List sessions uses project-scoped light pagination` 的 "informational total" 定义对齐，零后端改动。B 破坏 light pagination 设计，骨架阶段就要 read_dir 完整目录（其实当前实现也已经这么做，但 spec 不强制，未来可能改），违背 spec 意图。

### D4: spec scenario "同 projectId 新扫描取消旧扫描" 改写

把 ipc-data-api 的 Scenario 文本由 "**WHEN** ... **AND** 调用方再次调用 `list_sessions("projectA", { pageSize: 20, cursor: "next" })`" 改写为 "**WHEN** ... **AND** 调用方再次调用 `list_sessions("projectA", { pageSize: 20, cursor: null })`（**同 cursor**）"——保留同 cursor 抢占语义；新增 Scenario "不同 cursor 的并发扫描互不 abort" 覆盖 page 1 + page 2 并存。

## Risks / Trade-offs

- **[Risk] cursor 字符串编码冲突** → cursor 当前是纯数字 offset 字符串，无 `|` 字符；在 `metadata_scan_key` doc-comment 显式声明 `|` 为 reserved，未来 cursor schema 变化时同步检查。
- **[Risk] 旧 project 扫描的 broadcast 噪声** → 前端 listener 已按 `payload.projectId !== selectedProjectId` 过滤，capacity 256 远大于单 project 单 page 的 update 数（≤ pageSize=20）。同时切多个 project 扫描的 update 同时挤进 channel 时——一次切换最多 20 条 + 下一次切换 20 条，远不会 lag。
- **[Trade-off] 总扫描任务数轻微增加** → 用户首次加载 + 自动补满视口的场景下，page 1 / page 2 扫描并存而非互斥，CPU 峰值瞬时增加（依然受 `METADATA_SCAN_CONCURRENCY=8` semaphore 限流）。换来的是 page 1 title 不再卡住。是赚的。
- **[Trade-off] silent 刷新与 loadMore 的 cursor 不同**：silent 刷新走 cursor=`null`，loadMore 走 cursor="20" 等；二者不会互相 abort（设计如此）。silent 刷新自身重复触发会在 cursor=null 上抢占（同 D1 设计）。这是预期行为。
