## Context

`cdt-watch::FileWatcher` 把 inotify / FSEvents 事件 normalize 成 `cdt_core::FileChangeEvent` 后通过 `broadcast::channel<FileChangeEvent>(256)` 广播（`watcher.file_tx`）。`LocalDataApi` 启动时（`local.rs:2240` 附近）spawn 4 个独立 task：

1. `start_task` —— 启动 watcher
2. `bridge_task`（`local.rs:2253`）—— 订阅 `watcher.subscribe_files()` 转发到 `LocalDataApi.file_tx`（`channels.files`），让 Tauri host / SSE endpoint 等下游消费
3. `notifier_task`（`NotificationPipeline`）—— 直接订阅 watcher，**不**走 `LocalDataApi.file_tx`
4. `unified_invalidator_task`（`spawn_unified_cache_invalidator`）—— issue #261 把原本两个独立 invalidator（`ProjectScanCache` + `ParsedMessageCache`）合并到一个，减少 watcher subscriber 数 4 → 3

invalidator 内部有 sync 三档判定（`apply_file_event_to_project_scan_cache`），结果当前**沉默不暴露**给消费者。

**SSH 路径绕过**：`LocalDataApi::attach_remote_watcher`（`local.rs:1760`）走的是 `RemotePollingWatcher::spawn(..., self.file_tx.clone(), ...)` ——SSH 事件**直接生产到 `LocalDataApi.file_tx`**，绕过 `watcher.file_tx` 与 unified invalidator。`cdt-watch::FileWatcher::attach_remote`（`watcher.rs:101`）已经实现了"接入 watcher.file_tx"的正确路径但**未被使用**（codex 二审 round 1 发现）。

**HTTP/SSE 路径**：除 Tauri host emit 外，`crates/cdt-api/src/http/bridge.rs:49` 通过 `PushEvent::FileChange` 把 `LocalDataApi.file_tx` 的事件序列化喂给 HTTP `?http=1` 浏览器调试入口；`PushEvent::FileChange`（`events.rs:10`）当前只含 4 个字段。

**lag 兜底路径**：前端 `transport.ts:172/448` 已有 `sse-recovered` / `sse-lagged` pseudo-event；spec `sidebar-navigation` `Sidebar SHALL 订阅 sse-recovered / sse-lagged 触发 silent refresh` Requirement 已规定 lag 时前端走 silent refresh 兜底。

`SidebarNavigation` spec §"会话总数显示口径" 要求 silent 刷新时 `selectedGroup.totalSessions` 同步上升 / 下降，前端必须有触发 IPC revalidate 的条件；但当前条件粗到把所有事件都触发。telemetry（uptime 7831s）：`content_append_skipped: 1598` / `structural: 109` / IPC 1437 次。

## Goals / Non-Goals

**Goals:**

- 把后端 unified invalidator 已经在算的 structural 信号通过 `FileChangeEvent` payload 暴露给前端，让前端能精准过滤
- 让 unified invalidator 成为 `LocalDataApi.file_tx` **唯一生产者**（涵盖 Local + SSH 两条路径）
- HTTP/SSE 路径（`PushEvent::FileChange` + `transport.ts` normalize + 前端 store 类型）同步加 `sessionListChanged` 字段
- 维持 cache 失效语义不变（invalidator 内部三档判定逻辑 0 改动）
- 维持 `Sidebar` 三档触发的功能正确性：`projectListChanged || sessionListChanged || deleted` 仍能覆盖所有改 `total_sessions` 的场景
- broadcast lag 时前端走已有的 `sse-lagged` 兜底路径触发 silent refresh
- IPC 频率 1437 → ~109；p95 268ms → < 100ms

**Non-Goals:**

- **不**做 in-flight scan coalesce（留 P2，落地后看真实 telemetry 决定；burst 场景见 R9）
- **不**改 `refreshAfterInflight` 节流策略（叠加放大效应被源头治住）
- **不**改 `sig_mismatch` 高频判定（`O_APPEND` 设计内）
- **不**改 watcher 层判定逻辑（`cdt-watch::FileWatcher::parse_project_event` 与 `mark_project_seen` 不动；只改 `attach_remote_watcher` 调用方式让 SSH 走 attach_remote）
- **不**改 IPC 协议命令名 / 任何 `LocalDataApi` 公开方法签名

## Decisions

### D1: unified invalidator 是 `LocalDataApi.file_tx` 唯一生产者（含 SSH 路径）

**选择**：删除 `local.rs:2253` 的 `bridge_task`；`spawn_unified_cache_invalidator` 新增 `file_tx: broadcast::Sender<FileChangeEvent>` 参数（即 `LocalDataApi.file_tx` 的 sender），loop 内 sync invalidate 后立即 `file_tx.send(enriched_event)`，再异步 `apply_file_event_to_parsed_cache`。SSH 路径见 D5 改造。

**Why over alternatives:**

- **替代 A**：保留 bridge_task，让 invalidator 把 enriched event 旁路写到 file_tx —— **拒绝**：双 producer 引发事件顺序与重复问题
- **替代 B**：bridge_task 持有 invalidator 的判定函数引用，自己跑 sync 判定后 emit —— **拒绝**：把 invalidator 状态机暴露给 bridge，破坏封装；且 invalidator 自己也要跑同一判定，产生双跑
- **采纳**：唯一生产者保证 emit 顺序（sync 判定 → emit → async parsed invalidate）+ 单一数据流，分层最清晰

时延影响：`apply_file_event_to_project_scan_cache` 是 sync HashMap lookup（无 fs op），实测 < 10μs。emit 在锁释放后调用（`apply_file_event_to_*_cache` 函数返回后），不持锁。

### D2: SSH 路径通过 `FileWatcher::attach_remote_with_cancel` 接入 unified invalidator

**选择**：

1. 扩展 `cdt-watch::FileWatcher::attach_remote`（`watcher.rs:101`）签名加 `cancel: cdt_ssh::CancelToken` 参数（替代当前内部 `CancelToken::new()`），让调用方注入自己持有的 token；现有 1 个 test 调用方（`watcher.rs:862` `attach_remote_broadcasts_schema_compatible_file_event`）同步更新即可——这是 API breaking change 但 cdt-watch 不在 public 包发布范围
2. 改 `LocalDataApi::attach_remote_watcher`（`local.rs:1760-1771`）的 `RemotePollingWatcher::spawn(..., self.file_tx.clone(), cancel_token.clone())` 为调用 `self.watcher.attach_remote(sftp, projects_dir, cancel_token.clone())`；让 SSH 事件喂入 `watcher.file_tx`，与 Local 共用同一 unified invalidator 入口
3. `LocalDataApi` 加 `watcher: Option<Arc<FileWatcher>>` 字段（构造期注入），现有 `new()` / `new_with_watcher()` 两处 `Self { ... }` 初始化同步加默认 `None` / `Some(...)`
4. `cdt-ssh::polling_watcher::build_change_event` 仍构造 `session_list_changed: false`（由 invalidator 后续 enrich）
5. dead-signal monitor 路径不变——`RemoteWatcherHandle` 仍由 `attach_remote` 返回给 `LocalDataApi`，monitor task 仍持有 `cancel_token` clone，外部 disconnect 时仍能 cancel

**Why over alternatives:**

- **替代 A**：SSH polling 直接生产到 `LocalDataApi.file_tx` 但绕开 enrich（恒 `session_list_changed: false`）—— **拒绝**：SSH 用户在远端会话增 / 删时 sidebar `totalSessions` 滞后，与 Local 行为不一致；且违反 D1 唯一生产者契约
- **替代 B**：在 `polling_watcher` 层独立计算 `session_list_changed` —— **拒绝**：判定逻辑分叉两份；polling_watcher 没有 `ProjectScanCache` 上下文反查不到 `contains_session_id`
- **采纳**：用已存在的 `FileWatcher::attach_remote` API（`watcher.rs:101`），SSH 与 Local 共用同一 enrichment gateway

**已知限制**：`ProjectScanCache` 注释明写 "SSH entry 不被 watcher 触发，仅靠 TTL + generation 校验回收"。`apply_file_event_to_project_scan_cache` 内部以 `ContextId::local(projects_dir)` 决定失效作用域，仅清 Local entry；SSH 事件进入后规则 1 / 规则 3 都基于 Local cache snapshot 判定（与 Local 一致），不会清 SSH entry。规则 2 的 `contains_session_id` 在 SSH context 下查的是 Local cache（可能为空 / 不含 SSH session），自然不命中——SSH event 退化为只看 `project_list_changed || deleted`，与现状一致。

### D3: 字段命名 `session_list_changed`（camelCase `sessionListChanged`）

（保持原决策不变）

**选择**：在 `FileChangeEvent` 上加 `pub session_list_changed: bool`，`#[serde(default, skip_serializing_if = "std::ops::Not::not")]`，对应前端 `payload.sessionListChanged`。

**Why over alternatives:**

- `structural_change`：抽象，前端开发者不易理解
- `affects_total_sessions`：把字段绑死到一个 UI 用例，未来有其它消费者会语义不匹配
- 拆 `session_added` + `session_removed`：消费者只关心"是否变了"，拆开浪费 payload + 强迫消费者写 `(added || removed)`
- **采纳**：`session_list_changed` 既精确又中立，与已有 `project_list_changed` 命名风格统一

### D4: enrich 时机契约 —— sync invalidate 后、async parsed invalidate 前

**选择**：`spawn_unified_cache_invalidator` loop 体顺序：

```
loop {
    let event = rx.recv().await;
    let session_list_changed = apply_file_event_to_project_scan_cache(...);  // sync, lock 在函数内部释放后返回 bool
    let enriched = FileChangeEvent { session_list_changed, ..event };
    let _ = file_tx.send(enriched);                                            // emit (锁已释放)
    apply_file_event_to_parsed_cache(...).await;                                // async, 不阻塞 emit
}
```

**Why**：emit 必须在 step 4（sync 判定后、async stat 前）。前端拿到 file-change 决定是否拉 list_repository_groups，与 parsed_cache 的 stat I/O 是独立路径，不能让前端等磁盘 stat。issue #261 已经定下"scan-first"顺序契约，本变更只是把 emit 插在 scan 之后、parsed 之前，不破坏既有顺序约束。

**反压**：`broadcast::Sender::send` 满时丢旧元素不阻塞，invalidator 自身永远不会被慢 subscriber 阻塞（与现状 bridge_task 行为一致）。subscriber 慢造成的 lag 通过 D7 兜底。

### D5: HTTP/SSE 路径同步携带 `session_list_changed` 字段

**选择**：`crates/cdt-api/src/ipc/events.rs::PushEvent::FileChange` 加 `session_list_changed: bool` 字段；`crates/cdt-api/src/http/bridge.rs::spawn_file_bridge`（`bridge.rs:49`）转发字段；前端 `ui/src/lib/transport.ts` normalize 路径加映射；`ui/src/lib/fileChangeStore.svelte.ts::FileChangePayload` 类型定义加可选 `sessionListChanged?: boolean`。

**Why over alternatives:**

- 不动 SSE 路径，只动 Tauri 直 emit —— **拒绝**：HTTP `?http=1` 浏览器调试模式（spec `server-mode`）下前端拿不到字段，调试场景与桌面端语义分叉
- **采纳**：单一字段贯穿 IPC + HTTP/SSE 两路径，contract test 一处覆盖

### D6: broadcast lag 兜底走 `sse-lagged` 路径，三处 lag 来源同形态

**当前现状（codex round 2 核实）**：
- `crates/cdt-api/src/http/sse.rs:23` 已有 `SSE_LAGGED_SENTINEL = r#"{"type":"sse_lagged"}"#`，覆盖 `events_tx → SSE client` 一跳的 broadcast Lagged
- `crates/cdt-api/src/http/bridge.rs:56` 的 `spawn_file_bridge` 在 `file_rx Lagged` 路径**当前直接吞掉**——这是 `LocalDataApi.file_tx → events_tx` 一跳的 lag，下游 SSE stream 不会感知
- `src-tauri/src/lib.rs:1126` Tauri host file-change bridge 在 Lagged 路径**当前 continue 不通知前端**
- 前端 `transport.ts::TauriTransport`（`transport.ts:39` 附近）当前**不 listen** `sse-lagged` 事件名
- 前端 `Sidebar.svelte:364` 的 sse-lagged 订阅被包在 `if (!isTauriRuntime())` 门禁内，Tauri 下不订阅

**选择**：

1. **后端三处 lag 兜底统一形态**：
   - unified invalidator 在 `RecvError::Lagged` 路径调 `invalidate_local()` + counter `lag_conservative`，**不**通过 `file_tx.send` emit synthetic enriched event（lag 期间事件已丢，合成 event 字段语义不明）
   - HTTP `spawn_file_bridge`（`bridge.rs:56`）在 `file_rx.recv() Lagged` 路径 SHALL `events_tx.send(PushEvent::SseLagged { source: "file-change", missed: n })` 让下游 SSE 客户端走已有 sentinel 路径
   - Tauri host file-change bridge（`src-tauri/src/lib.rs:1126`）在 `file_rx.recv() Lagged` 路径 SHALL `app_handle.emit("sse-lagged", { source: "file-change", missed: n })`
2. **新增 `PushEvent::SseLagged` variant**（`crates/cdt-api/src/ipc/events.rs`）：携带 `source: String` + `missed: u64`，序列化为 `{ "type": "sse_lagged", "source": "...", "missed": ... }`；HTTP `convert_broadcast_result`（`sse.rs:35`）已有的 `Lagged` 路径与新 variant 形态对齐（消费者拿到都是 `type: "sse_lagged"`）
3. **前端两路径同步打通**：
   - `TauriTransport`（`transport.ts:39` 附近）SHALL 显式 `listen("sse-lagged", payload)` 后通过 `dispatch("sse-lagged", payload)` fanout 给 handler 列表，与 `BrowserTransport` 内部 synthesize 路径形态一致
   - 移除 `Sidebar.svelte:364` 的 `if (!isTauriRuntime())` 门禁——sse-lagged / sse-recovered handler 在两 runtime 下都注册（`sse-recovered` 在 Tauri 下不会被触发但订阅 noop，无副作用；`sse-lagged` 现在两 runtime 都会触发）

**Why over alternatives:**

- 让 invalidator 在 lag 时 emit synthetic enriched event 触发前端兜底 —— **拒绝**：synthetic event 的 `project_id` / `session_id` 字段语义不明
- 让 Tauri host bridge 自己拦 `Lagged` 不通知前端 —— **拒绝**：前端在 lag 期间错过 structural 信号 → totalSessions 滞后 5min（LOCAL_CACHE_TTL）才恢复
- HTTP `spawn_file_bridge` lag 时直接 drop event 不喂 SseLagged —— **拒绝**（codex round 2 阻塞 3）：现有 SSE sentinel 只覆盖 `events_tx → client` 一跳的 BroadcastStream Lagged；`file_rx → events_tx` 一跳的 lag 当前被吞，前端永远拿不到信号
- **采纳**：三处 lag 来源（invalidator、HTTP bridge、Tauri bridge）形态对齐，前端两 transport 打通同一 handler 路径；语义清晰

### D7: 拆 sole producer 契约为独立 Requirement

**选择**：在 spec delta 中把"unified invalidator 作为 `LocalDataApi.file_tx` 唯一生产者"独立成 ADDED Requirement `Unified invalidator 作为 LocalDataApi.file_tx 唯一生产者`，不埋在 `ProjectScanCache 按事件语义分级失效` Requirement 里。

**Why**：sole producer 约束的是 `LocalDataApi.file_tx` 事件拓扑，影响 Tauri / SSE / SSH 所有消费者，不只是 cache 语义。独立 Requirement 让 reviewer 单独可见 + 独立 SHALL 句 fidelity 易于审计。

## Risks / Trade-offs

- **[R1] `bridge_task` 移除让现有外部消费者拿到的 event 形态变化** → 对外 IPC payload **完全兼容**（新增字段 + `#[serde(default)]`），旧客户端反序列化拿到 `sessionListChanged=false`，行为退化为不触发 loadProjects；SSE 历史回放可能看到字段缺失，可接受
- **[R2] enriched event 时延比 raw event 多 ~10μs（sync HashMap lookup）** → 实测可忽略；issue #261 注释证实大多数事件走 `content_append_skipped` 快路径
- **[R3] invalidator panic 时 `file_tx` 也停产** → 现状 invalidator 已有 `into_inner` poison 兜底（issue #261 hardening），单 task panic 不会一刀切；本改动不引入新 panic 点
- **[R4] 11 处 `FileChangeEvent { ... }` 构造点编译失败** → 按 `crates/CLAUDE.md` 硬约束流程，一轮 Edit 全部补齐再 `cargo check`；编译失败是显式破坏而非沉默 bug，CI 拦
- **[R5] 前端 sessionListChanged=false 但实际改了 total_sessions 的盲点** → 唯一可能盲点是 SSH 路径下 cache 为空时三档判定退化（D2 已知限制）；现状用户在 SSH context 下侧边栏行为已经依赖 TTL + 手动刷新，本变更不引入新退化
- **[R6] IPC contract test 字段名拼写错配** → `crates/CLAUDE.md::Serde / IPC 契约` 硬约束 `xx_session_yy → xxSessionYy`，加 round-trip test 拦截
- **[R7] HTTP/SSE 字段同步漏遗导致桌面端与浏览器调试模式行为分叉** → D5 显式覆盖 `PushEvent` + `bridge.rs` + `transport.ts` + store 类型四处；contract test 同时覆盖 IPC 与 HTTP 形态
- **[R8] `LocalDataApi.file_tx` capacity=256 在极端 burst 下被打满引发 `Lagged`** → D6 走已有 `sse-lagged` 兜底；但若 lag 频繁说明实际负载超 broadcast 设计假设——后续观察 telemetry 决定是否调 capacity 或改 mpsc + per-subscriber buffer
- **[R9] 109 次 structural 在 git checkout / mass delete 等 burst 场景仍可能短时聚集触发并发 scan miss** → 留 P2 scan coalesce 在后续 PR 处理；本 PR 落地后看真实 telemetry，若 bucket 28 反弹未自然降到 < 10 再加 coalesce
- **[R10] SSH 路径改用 `FileWatcher::attach_remote` 后，dead-signal monitor cancel 链路需要保留** → codex round 2 核实：当前 `attach_remote_watcher` 自建 `CancelToken` clone 给 monitor task；扩展 `attach_remote` 签名加 `cancel: CancelToken` 参数让调用方注入，`RemoteWatcherHandle` 仍由 watcher 返回给 LocalDataApi，monitor task 持有 token clone 继续工作。一处 test 调用方（`watcher.rs:862`）同步更新签名
- **[R11] HTTP 路径 `spawn_file_bridge` 的 `file_rx Lagged` 当前被吞** → codex round 2 阻塞 3：现有 SSE sentinel 只覆盖 BroadcastStream → SSE client 一跳；`file_rx → events_tx` 一跳的 lag 不可见。本变更通过加 `PushEvent::SseLagged` variant + bridge 显式 emit 把这条 lag 路径打通，前端拿到与现有 BroadcastStream lag sentinel 形态一致的信号
- **[R12] Tauri runtime sse-lagged 前端订阅当前断链** → codex round 2 阻塞 2：`TauriTransport` 不 listen `sse-lagged` + Sidebar 把订阅包在 `!isTauriRuntime()` 门禁内。本变更两侧打通，前端 handler 在两 runtime 下都注册

## Migration Plan

无需迁移。本改动是 IPC 字段**新增**（不删 / 不改语义），旧客户端反序列化拿到 `sessionListChanged=false`，行为等价于"老版前端不识别新信号"——退化为不触发额外 `loadProjects`，但通过 `projectListChanged || deleted` 仍能覆盖大部分 structural 事件。

发布后回滚：`git revert` 单 PR 即可；下一个 release 即恢复旧行为，无需 schema 迁移 / 数据修复。

## Open Questions

无。D1-D7 已覆盖所有设计层决策，剩余实现细节走 tasks.md 实现层落地。
