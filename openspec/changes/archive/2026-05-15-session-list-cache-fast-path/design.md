## Context

`LocalDataApi::list_sessions` 是 sidebar 列出会话的主入口。原行为：

1. **骨架阶段**（`list_sessions_skeleton`）：扫描 project 目录拿到 `(session_id, last_modified, jsonl_path)` 列表，分页后构造 `Vec<SessionSummary>`，所有元数据字段（`title` / `messageCount` / `isOngoing` / `gitBranch`）填占位（`None` / `0` / `false`），同时构造 `page_jobs: Vec<(String, PathBuf)>` 列表
2. **后台扫描阶段**：spawn 一个 `scan_metadata_for_page` task，内部用 `JoinSet` + `Semaphore(METADATA_SCAN_CONCURRENCY=8)` 并发扫描每条 jsonl，调 `extract_session_metadata_cached` 拿元数据，通过 `broadcast::Sender<SessionMetadataUpdate>` 发出
3. **emit 桥接**：Tauri host 在 `setup` 阶段订阅 `subscribe_session_metadata()`，把 broadcast 转 `app.emit("session-metadata-update", ...)` 推到前端
4. **前端 patch**：`Sidebar.svelte` 通过 `listen("session-metadata-update")` 按 sessionId in-place 替换 `sessions` 数组里对应项的元数据字段

PR #80 修 "sidebar 偶发显示 sessionId 8 字符前缀" bug 时发现：tauri emit 在前端 listener 未注册时 fire-and-forget 直接丢，导致 session 永久卡在 `title=null` → UI fallback `sessionId.slice(0, 8) + "…"`。前端主修是 onMount 调换顺序确保 listener 先于 emit 注册。

后端 cache fast-path 是**兜底**：`MetadataCache`（change `multi-session-cpu-cache`）已对 `get_session_detail` / 后台扫描结果做了 LRU 缓存（capacity=200，`FileSignature` mtime+size+identity 等价校验）。骨架阶段可以先查 cache，命中条直接 inline 填回真实元数据、不入 page_jobs。这样即使 emit 链路任何原因丢消息，重复打开列表也能从 cache 拿到完整元数据。

## Goals / Non-Goals

**Goals:**
- 把 cache fast-path 路径固化到 `ipc-data-api` spec 的 `Emit session metadata updates` Requirement，让后人改 `MetadataCache` 行为时能看到 sidebar 的依赖契约
- spec delta 真实反映 PR #80 已实现的代码行为，不引入任何未实现的新行为
- 描述并发限流（`Semaphore(8)`）作为 caller 防御层

**Non-Goals:**
- 改 `MetadataCache` 自身的实现（capacity / eviction / FileSignature 算法）—— 沿用 change `multi-session-cpu-cache` archive
- 改 `subscribe_session_metadata()` broadcast channel 协议 / payload 字段名 / camelCase 序列化约定
- 改前端 onMount 顺序（已在 PR #80 实现，属于前端时序 bug fix，不需要 spec）
- 修 `FileSignature` 等长改写 + mtime 不变的 cache stale 理论漏洞（codex Q2，是单独的 cache 算法改动，超出本 change 范围）
- 加新的 const 回滚开关

## Decisions

### D1：cache 命中条不入 page_jobs（不 spawn 扫描、不 emit）

**选项 A**（采用）：cache 命中条直接 inline 填骨架，不入 page_jobs；未命中条入 page_jobs 走原后台扫描路径。

**选项 B**：cache 命中条仍 inline 填骨架，但**也**入 page_jobs（重扫 + 覆盖 emit）。

**选项 C**：保持原行为不变，仅前端 onMount 顺序修复。

**为什么选 A**：cache 已有 mtime 等价校验（`FileSignature::from_metadata`），命中说明文件未变，重扫是冗余 IO + 冗余 emit。选 B 会让 broadcast 在高频列表查看场景下产生不必要事件流，前端 patch 也是 no-op。选 C 不能解决 "emit 丢失 → title 永久 null" 兜底问题（前端主修虽然消除了 onMount 时序，但其它隐性丢消息路径仍可能存在，如 webview 重 mount / 切 worktree）。

**风险**：cache stale 时（`FileSignature` 等长改写 + mtime 不变的罕见 case）会显示旧 title。但这是 cache 算法层固有缺陷（codex Q2），与本 change 无关；mtime 不变意味着用户视角文件"没变"，旧 title 是可接受的。

### D2：lookup 并发执行 + Semaphore 限流

**选项 A**（采用）：用 `futures::future::join_all` 并发调度 N 条 lookup（每条内含 `tokio::fs::metadata` await），并发上限 `Semaphore(METADATA_SCAN_CONCURRENCY=8)`。

**选项 B**：串行 `for s in page_sessions { try_lookup(...).await }`。

**选项 C**：无限制并发（不加 Semaphore）。

**为什么选 A**：caller（如 `list_all_sessions` 路径）可传 `page_size=50` 甚至更大，串行 stat 累计 ms 数随 page 线性放大（macOS NVMe ≈ 0.5ms/次，50 条 ≈ 25ms）；并发后理论 ≈ 5ms。codex 二审 Q3 明确指出"caller 可传任意大 page_size" → 选 C 会一次性把 tokio blocking pool（`spawn_blocking` 池容量 512）占满影响别处 IO。选 A 用同一上限 `METADATA_SCAN_CONCURRENCY=8` 与后台扫描对齐，保持单一可调参数。

**风险**：[Semaphore 排队阻塞骨架返回] → cache 全命中时 stat 仅 0.5ms/次，8 并发足以打满。cache 全 miss 时骨架返回 SHALL NOT 被 JSONL full scan 阻塞（miss 条入 page_jobs 走后台扫描），但仍 SHALL 等待受限并发（`Semaphore` ≤ 8）的 stat + cache lookup 全部完成才返回——即骨架返回延迟与 page_size 弱相关、与 IO 延迟强相关，page=20 时实测 ≈ 5-10 ms。

### D3：lookup-only 函数与 `extract_session_metadata_cached` 解耦

**选项 A**（采用）：新增 `try_lookup_cached_metadata(cache, path) -> Option<SessionMetadata>`，cache miss 或 stat 失败返回 `None`，由调用方决定 fallback；**不**触发扫描。

**选项 B**：复用 `extract_session_metadata_cached`，但在骨架阶段调用时如果 cache miss 会触发同步扫描（async fs read 整个 jsonl）。

**为什么选 A**：选 B 会让骨架阶段的 cache miss 退化成"同步全扫"，违反 spec "骨架快速加载" 不变量（page=20 时最坏 20 个文件全扫，秒级延迟）。选 A 让 cache miss 严格 fallback 到后台扫描路径，骨架不被阻塞；同时 lookup-only 函数语义更清晰（其名表达"只查不扫"），后人维护时不会误用。

### D4：与现有 SHALL / Scenario 的兼容性

主 spec line 23 已写：`title` / `messageCount` / `isOngoing` **SHALL 允许**为占位值——"允许"不是"必须"。fast-path 路径下骨架带真实值仍符合"允许占位"语义（允许范围 ⊇ {占位, 真实值}）。

主 spec 现有 Scenario "receiver SHALL 在扫描完成后**最多**收到 N 条 SessionMetadataUpdate" 用"最多"作为上界，cache 全命中时 0 条仍满足 0 ≤ N。

因此本 change 的 spec delta 是**扩展**而非冲突：原有 SHALL 全部保留，新增的 Scenario 描述 cache 命中条的 invariant + 并发限流的 invariant。

### D5：事后补 spec 的元决策（meta）

按 CLAUDE.md `feedback_sync_spec_after_code.md`，行为契约改动应"先 propose 再 apply"，事后补是已被否决的下策。本 change 是 PR #80 已 push 后才补 spec 的事后场景。元决策：

**接受事后补**（vs. 撤回 PR #80、删 cache fast-path 部分、重新走 propose → apply → archive 流程）：

理由：
1. cache fast-path 实现已通过 5/5 测试 + just preflight + codex 2 轮异构二审，**代码层质量已验证**
2. 撤回 + 重做的成本高，且 spec delta 本身是真实反映已实现行为，reviewer 看 spec 与代码不会脱节
3. 用户已显式同意走"补 spec"路径（"补充一下 openspec 吧"）

代价：
1. 违反 CLAUDE.md "先 propose 再 apply" 硬约束，在 user memory 已记入 `feedback_identify_behavior_extension`（识别 bug fix 里混进的新行为路径）作为反思
2. design.md 在 apply **之后**写，缺失"propose 阶段的设计取舍机会"——本 design.md 是 reconstruction，未来 reviewer 无从评估"如果先 propose 是否会选别的方案"

下次出现"修 bug 时引入新行为路径"的混合改动，先按 `feedback_identify_behavior_extension` 拆分识别，新行为路径**那部分**走 openspec。

## Risks / Trade-offs

- [cache stale（D1 风险延展）] → mtime 不变 + 等长改写的罕见 case 显示旧 title。修复路径属于 `multi-session-cpu-cache` cache 算法层，单独跟进。
- [并发 Semaphore 等待时间放大骨架返回] → cache 全命中时实测 < 10ms（page=20，8 并发，0.5ms/stat），用户感知阈值之下。
- [active_scans 与 cache 全命中的交互] → cache 全命中时 `page_jobs.is_empty()` 让 `list_sessions` 跳过 spawn 分支，**不**触碰 `active_scans`。原有 race-free 抢占逻辑（abort + generation + insert 在同一 sync lock 临界区）由"cache miss 时进入 spawn 分支"路径自然继承，没有引入新 race（codex 二轮二审 Q5 已确认）。
- [事后补 spec 的流程债（D5）] → 已记入 user memory feedback，下次避免。
