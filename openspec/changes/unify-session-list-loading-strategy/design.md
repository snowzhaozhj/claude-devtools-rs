## Context

`cdt-api` 已经在 `add-server-mode` change 中完成 `/api/events` SSE bridge + 浏览器 `transport.ts` 归一化，**基础设施已就位**但 HTTP `list_sessions` 路由仍调 `LocalDataApi::list_sessions_sync()`——一个为"HTTP 无 push 通道"假设设计的完整同步全扫方法（实测 100–500 ms 阻塞 fetch）。这造成两条加载链路在 handler 层就分叉：

- **IPC 路径**：`list_sessions` 骨架快返（~5–20 ms）+ `try_lookup_cached_metadata` lookup-fast-path + 后台 `JoinSet + Semaphore(8)` 扫描通过 `broadcast::Sender<SessionMetadataUpdate>` push 增量；前端 Sidebar 监听 Tauri event `session-metadata-update` in-place patch。
- **HTTP 路径**：`list_sessions_sync` 同步全扫元数据，single response 含完整字段；浏览器订阅 `/api/events` 但 metadata producer 接的是同一 broadcast，**仅** file-change 与其他 IPC 调用触发的扫描产物才会被推到 SSE——HTTP `list_sessions` 自己不再 spawn 扫描，等于完全绕过自家 SSE 通道。

冷启动场景下，`MetadataCache`（内存 LRU capacity=200，by `FileSignature`）随进程退出全清；下次启动首次 `list_sessions` 所有条目 cache miss，骨架字段 `title=null / messageCount=0 / isOngoing=false` 必须等后台扫描 emit 才能填上，视觉上"字段瞬变"明显。

桌面侧 Sidebar.svelte 已经在 PR #122 之类做过反闪烁优化（不清空旧列表 / pendingMetadataUpdates buffer race fix / in-place patch 不替换实例引用），但 store 级没有 sessions cache（每次切 project 都重拉骨架）、无 in-flight cancel（旧 project 还在后台扫描时新 project 已 spawn）、loadMore 无 debounce（快速滚动可触发多次串行 IPC）。

## Goals / Non-Goals

**Goals**

- 桌面 IPC 与 server mode HTTP 走**同一个** `list_sessions` 后端实现，杜绝 handler 层协议分叉
- server mode 翻页耗时从 100–500 ms → ~10–20 ms（与 IPC 同档）
- 切 project 时 0 视觉断层：保留旧 project 列表至新 project 骨架到达（已实现）+ store 级 sessions cache 让回头切 project 时直接 stale-first hydrate
- 快速翻页 / 快速切 project 不排队：in-flight cancel + 100 ms trailing debounce
- 冷启动 `list_sessions` 骨架阶段对**文件未变化**的 session SHALL 直接命中 cache 带真值（zero emit），消除"字段跳变"的冷启视觉断层
- metadata 字段从占位到真值用 CSS `transition: opacity` ≤ 150 ms 渐显，骨架行显示统一 shimmer placeholder

**Non-Goals**

- **不**做后端 metadata cache 的"hover prefetch / sibling prewarm"等主动预热（方案 C，风险面大、与"先做对再做快"冲突）
- **不**做前端 sessions store 的磁盘持久化（仅内存 LRU；磁盘持久化交给后端 `MetadataCache`——前端 cache 是 staleness 容忍区，进程重启重新走骨架 + 后端 cache 命中即可）
- **不**改变 `cdt-cli` / 非浏览器 HTTP client 的行为契约：`LocalDataApi::list_sessions_sync()` trait method 仍保留作 trait 默认 fallback，但 axum HTTP route 不再调用
- **不**改 `SessionMetadataUpdate` broadcast schema 或 SSE event 字段命名

## Decisions

### D1 — HTTP `list_sessions` 路由切到骨架版

**决策**：`cdt-api::http::routes::list_sessions` 把 `s.api.list_sessions_sync(...)` 改成 `s.api.list_sessions(...)`，复用 IPC 的骨架 + spawn 扫描 + broadcast emit 实现。SSE bridge 已订阅 `LocalDataApi::subscribe_session_metadata()`（`add-server-mode` change `http-data-api` spec §"session_metadata_update producer"），扫描产物自动经 bridge 推到所有 `/api/events` 客户端。

**Alt 1**（rejected）：保留 `list_sessions_sync` 但在内部也 `broadcast::send(...)` 一份 metadata。

理由：双写两条路径同步/异步语义错位，HTTP client 同时收完整 metadata 还收 SSE patch；语义重复 + 排查复杂。

**Alt 2**（rejected）：HTTP 走骨架 + 自己起个 long-poll 端点而非复用 SSE。

理由：`add-server-mode` 已把 SSE 定为唯一 push 通道，再加 long-poll 是协议分叉相反方向。

### D2 — 浏览器 `EventSource` SHALL 早于 `list_sessions` 首次调用

**决策**：`ui/src/lib/transport.ts::BrowserTransport.invokeHttp('list_sessions', ...)` 入口 SHALL `await this.ensureSseReady()` 才发首次 GET。`ensureSseReady()` 内部按"重新读 source 引用"循环（不是绑定到一个特定 EventSource 实例的 onopen）：

```ts
async ensureSseReady(): Promise<void> {
  const deadline = performance.now() + 1000;
  while (performance.now() < deadline) {
    const src = this.source;                 // 每次循环重读 this.source
    if (src && src.readyState === EventSource.OPEN) return;
    if (!src || src.readyState === EventSource.CLOSED) this.scheduleReconnect(src);
    await new Promise(r => setTimeout(r, 50));   // 50ms 轮询
  }
  console.warn("[transport] SSE not OPEN within 1000ms, proceeding without subscription");
}
```

这样可以正确处理"重连退避中（current source 是 CLOSED，新 source 还没创建）"窗口——固定到 onopen 监听器会绑死在旧 source 上，永远等不到新 source 的 OPEN。50 ms 轮询粒度对 1 s 超时窗口足够（理论上 20 次检查），CPU 开销可忽略。

**多 tab 场景规约**：浏览器多 tab 同打开 server-mode 时，每个 tab 是独立 JS context、独立 `BrowserTransport` 单例、独立 `EventSource` —— **不共享**。HTTP/1.1 同源 6 连接限制意味着同时打开 ≥ 6 个 tab 时第 7 个起的 EventSource 会进入 pending 等可用连接；本 change **不**引入 BroadcastChannel / SharedWorker 跨 tab 共享 SSE（实现复杂、跨浏览器兼容差），而是规约：用户同源同时 ≥ 6 tab 是非主流场景；超出时第 7+ tab 的列表加载 SHALL 表现为 `ensureSseReady` 1000 ms 超时放行 → 骨架返回但 metadata patch 走 file-change silent refresh 兜底（功能不丢，仅冷启字段跳变）。该限制 SHALL 在 user-facing docs 中提示（README server-mode 段落标注"建议 ≤ 5 个 tab 同时打开"）。

**Alt**（rejected）：用 HTTP response header 嵌入一个"scan 已开始" tag，前端轮询 patch endpoint。

理由：和 SSE 重复，浪费已有基础设施。

### D3 — sessions store 数据结构

**决策**：新增 `ui/src/lib/sessionListStore.svelte.ts`，单例 module-level `$state`：

```ts
type SessionListEntry = {
  sessions: SessionSummary[];      // 含骨架与已 patch 的真值
  nextCursor: string | null;
  total: number;
  lastFetchedAt: number;           // ms epoch，用于 stale 判定
  generation: number;              // 每次 fetch 递增；fetch 返回时校验
};
const cache = new Map<string, SessionListEntry>();
// LRU 按访问顺序保留 max 16 个 project，超出 evict 最久未访问条
```

API：
- `read(projectId)`：纯读，立返（或 undefined）
- `loadFirstPage(projectId, opts: { silent: boolean, mode: 'replace' | 'merge' })`：从 transport 拉首页，返回 `Promise<SessionListEntry>`；内部维护 `inflight: Map<string, Promise>` 防同 project 同 cursor 重发
- `loadMore(projectId)`：100 ms trailing debounce，cursor 拼接拉下一页
- `applyMetadata(update)`：listener 收到 `session-metadata-update` 时调用，按 sessionId in-place patch（不替换 entry 引用，仅 mutate `sessions[i]` 的字段）
- `invalidate(projectId)`：file-change 触发的 silent refresh 用，不删 entry（保留 stale），下次 read 仍返还，但 marker `lastFetchedAt` 倒回，触发 SWR

**Why module-level Map 不用 Svelte `$state(rune)` deep proxy**：sessions 列表 in-place mutation + Map 操作不需要 reactive 全树追踪；Sidebar 通过显式订阅或 polling-read pattern 消费，避免 Svelte 5 deep proxy 对 200×16 = 3200 个 SessionSummary 都建 proxy 的开销。

### D4 — In-flight cancel：generation token + AbortController 双轨

**决策**：每次 `loadFirstPage` / `loadMore` 启动时 `++entry.generation`，记录 `my = entry.generation`；fetch 返回时若 `entry.generation !== my` 直接 return 丢弃响应。

- **HTTP 路径**：同时创建 `AbortController`，挂到 `fetch(..., { signal })`；新 generation 启动时 abort 旧 controller，让网络层立即释放。
- **IPC 路径**：Tauri `invoke` 不支持 abort，generation 是唯一手段；后端 `list_sessions` 已有 `active_scans` per-`(projectId, cursor)` abort 机制，前端不需要主动通知后端。

**Why 不让后端也按 projectId cancel**：当前 spec 明确"不同 cursor 的扫描 SHALL 并存而互不 abort"（spec ipc-data-api §"Session list pagination avoids duplicate full scans"），切 project 后旧 project 的后台扫描产物会因 `pendingMetadataUpdates` 已 clear + sessions 已 replace 而被前端 buffer/list 双层 drop，无需后端介入。改后端语义会引入新 race window，得不偿失。

**已知 stale-update race（接受作为最佳努力）**：用户在 A → B → A 的快速来回切换路径下，若期间 A 项目的某 session 文件发生变更（mtime/size 改变），可能出现以下竞争：

1. 第一次 A 访问触发的旧扫描在 abort 之前已对该 sessionId emit 出旧值的 `SessionMetadataUpdate`，但消息还在 Tauri event queue / SSE wire 上未被前端 listener 处理；
2. 用户切到 B 期间，listener 收到这条 update：`update.projectId === A !== selectedProjectId === B`，handler 第一行 drop（无 race，spec sidebar-navigation §"会话元数据增量 patch" 已规约）；
3. 用户切回 A，listener 在 store mutation 路径上重新激活并应用 update.projectId === A === selectedProjectId → 写入 store。

如果在步骤 2-3 之间文件已经被新扫描重新读出真值（更晚的 update 已经 emit 并被处理过），步骤 3 的旧 update 会**短暂覆盖**新 update 的字段值。该 race 的触发窗口窄（要求 200–500 ms 内 A→B→A + 期间文件变更），且 file-change watcher debounce 100 ms 后会再次触发 silent refresh 拉回真值——**接受**作为最佳努力，不引入 IPC schema 改动（payload 加 `scanToken` 等）。后端 `SessionMetadataUpdate` 序列化字段保持现状。spec sidebar-navigation 显式 acknowledge 该 race 的存在与 file-change 兜底路径，避免实现者误以为有遗漏。

### D4b — Sidebar 不强制通过 store API 调 loadFirstPage / loadMore（apply 阶段反转）

**反转**：原 D4 假设 Sidebar 重构为完全通过 `store.loadFirstPage` / `loadMore` 调用，让 cancel 机制对 sidebar 路径生效。apply 阶段评估发现：

1. Sidebar 现有 `loadSessions` / `loadMoreSessions` 已经实现 `selectedProjectId / sessionsNextCursor` 校验 + `sessionsLoadingMore` flag 等价的 leading-fire 短路保护
2. 切换 Sidebar 完全通过 store 需要新建 subscribe/notify 机制让 sidebar.sessions 与 store.entry.sessions 双向同步，并把 sidebar 的 reconcile pinned/hidden + `pendingMetadataUpdates` race buffer 重写嵌入 store——改动 surface area > 200 行且涉及 race bug 重 testing 风险
3. cancel 机制的真实收益（"避免快速切 project 时旧 IPC response 覆盖新列表"）已被 sidebar 既有 `if (projectId !== selectedProjectId) return` 路径覆盖；store cancel 仅在 sidebar 通过 store 自身做 SWR refresh 时实际生效

**修订**：
- Sidebar 保留原 IPC 直调路径（`listSessions(...)`）；新增 store 集成仅做三件事：(a) 切 project 时 `store.read(projectId)` 命中则立即 hydrate；(b) IPC resolve 后调 `store.setSessions(...)` 写回 store 缓存；(c) metadata listener 调 `store.applyMetadata(...)` 与 store 保持同步
- store API `loadFirstPage` / `loadMore` 的 generation token + AbortController + debounce 实现**保留**作为内部 SWR refresh 路径 + 未来 sidebar 完全切换到 store 时的契约
- 见 spec sidebar-navigation 修订后的"Store `loadFirstPage` / `loadMore` 内部 generation token 取消机制" Requirement 与"Sidebar 集成边界"段落

**Why 保留 store cancel 机制**：sidebar 后续通过 store 触发 silent refresh 时（如 file-change 路径调 store 内部 refresh），cancel 防止并发 SWR 互相覆盖；这部分行为不依赖 sidebar 是否完全使用 store。

### D5 — `loadMoreSessions` leading + trailing debounce 100 ms

**决策**：store 内部 `loadMore(projectId)` 加 **leading + trailing** 组合 debounce 100 ms：

- **入口**：先检查 inflight short-circuit（`entry.inflightCursor === currentCursor` 时直接 return），inflight 期间任何调用都被丢弃
- **Leading**：当前不在 100 ms cooldown 窗口内 → 立即触发 fetch，记录 `lastFiredAt = now`
- **Trailing**：当前在 cooldown 窗口内 → 重置 trailing timer 到 `lastFiredAt + 100 ms`；timer 触发时若仍在 cooldown 内、且**仍不在 inflight 状态**，发起一次 fetch；timer 已 pending 则不重复 schedule

**Why 既要 leading 又要 trailing**：

- 纯 trailing：用户**慢速**滚动每 110 ms 触发一次 maybeLoadMoreSessions，每次都越过 100 ms trailing 边界 → 每次都发 fetch；inflight 期间 fetch 会被 `sessionsLoadingMore` short-circuit，但**两次 fetch 之间**仍可能产生新 generation 浪费后端 active_scans 抢占工作
- 纯 leading：首次进入 threshold 立即发，期间 100 ms 内的滚动信号都丢弃；用户**停在 threshold 边缘** 99 ms 后又继续滚动时，trailing fetch 不会触发 → 用户感受到"翻页迟钝"
- **leading + trailing**：首次立即发（用户无感知延迟），cooldown 内合并所有触发为 1 次 trailing fetch（节省冗余 work），inflight short-circuit 防同 cursor 重复

inflight short-circuit 必须在 debounce 触发前判定（即 leading 检查前的第一道闸门）——不能让 leading fire 后再发现已在 inflight 而 abort，那会让后端 active_scans 多走一次 spawn/abort 循环浪费。

100 ms 是人类感知滚动停顿阈值（< 100 ms 视为连续滚动；> 100 ms 视为停顿）。

**Alt 1**（rejected）：仅 leading-edge debounce。理由：见上"纯 leading"分析。

**Alt 2**（rejected）：仅 trailing-edge debounce。理由：见上"纯 trailing"分析（codex 二审 Q4 指出）。

### D5b — Sidebar 的 `loadMoreSessions` 不直接调 store.loadMore（apply 阶段反转）

**反转**：与 D4b 同源——sidebar 完全使用 `store.loadMore` 需要 reactive 同步机制，复杂度不值。Sidebar `loadMoreSessions` 继续走原 `listSessions(projectId, pageSize, cursor)` IPC 直调 + `sessionsLoadingMore` flag 保护。

**Sidebar 路径与 store API 的关系**：
- sidebar 的 `sessionsLoadingMore` flag 是 leading-fire + inflight short-circuit 等价行为（首次 maybeLoadMoreSessions 触发时 flag=true 立即 fire；后续高频触发被 flag 短路）
- scroll 事件在用户停下后自然停止，sidebar 不需要 trailing-fire（停顿 100 ms 后用户继续滚动会自然 re-trigger 新 scroll 事件）
- store.loadMore 的 leading+trailing+inflight short-circuit 实现保留供未来 sidebar 完全切换到 store 时使用

**为什么 store 仍保留 trailing-fire**：store API 的设计目标是"通用列表 + debounce loadMore"，调用方可能不是 scroll 事件驱动（如 keypress / button click 按节奏触发），trailing-fire 在那些场景才有显著价值。

### D6 — metadata 字段视觉渐显

**决策**：骨架行 metadata 字段（`title` / `messageCount` / `isOngoing` 圆点 / `gitBranch`）SHALL 用条件 CSS class `.metadata-pending` 标识占位状态，class 上挂 shimmer 动画（`linear-gradient` 移动）；元数据 patch 到达后移除 class，触发 `transition: opacity 150ms ease-out` 让真值 fade-in。

**Why 不用 Svelte `transition:fade`**：transition 指令绑定 mount/unmount，元数据 patch 不重建 DOM 节点（in-place patch 是字段 mutate），用 transition 不触发；CSS transition + class toggle 更直接。

**Why 150 ms**：>200 ms 会让用户感到"卡顿等待"，<100 ms 等同瞬变无渐显感；150 ms 是 Material Motion / iOS HIG 共同推荐区间。

### D7 — 冷启动闪烁靠 UI 视觉策略，**不**持久化 cache 到磁盘

**决策**：冷启动 `MetadataCache` 为空，所有 session 字段都要等后台扫描 emit 才能填上真值——这个"骨架到真值"的内容变化**不可消除**，但能在视觉层让它**看起来像"加载完成"而不是"内容跳变"**。所有冷启视觉过渡靠：

- 骨架行用 `.metadata-pending` class + shimmer 动画铺底，表达"加载中"语义
- metadata patch 到达后移除 class，触发 CSS `transition: opacity 150ms ease-out` 让真值从透明渐变到不透明

总过渡 200–370 ms（骨架渲染 5–20 ms + 后台扫描 50–200 ms + 渐显 150 ms），用户感知是"sidebar 在加载 → 加载完成"。这是 design D6 已规约的内容。

**Alt 1**（rejected）：`MetadataCache` 持久化到磁盘（启动 hydrate / 退出 dump）。

理由（与用户原始 B 范围约束一致）：

1. **长时间不打开 app 后磁盘 snapshot 全 stale**——所有 session mtime 可能已变，`FileSignature` 校验命中率跌到接近零，持久化等于白做却留下复杂度
2. **临时数据上磁盘违反"内存级状态不持久化"原则**——引入跨 process / 跨平台 / 版本演进 / 退出 dump 竞争 / 磁盘满静默失败等多维度风险面（codex Q5c 已指出跨 process dump 互覆盖）
3. 视觉收益**仅在剩余 20%**（首帧完全无占位）；前述 UI 视觉策略已覆盖 80% 体验，性价比不值
4. 即使加 `FileSignature` 校验真相源也只能保证"显示不会错"，不能阻止"用户看到的 session 后端已删除"的短暂 ghost

**Alt 2**（rejected）：冷启 `list_sessions` 同步全扫前 20 条（不 spawn 后台）。

理由：等于回到 `list_sessions_sync` 性能档（50–200 ms 阻塞），与 D1 "HTTP 与 IPC 共用骨架 + push" 统一方向冲突；冷启首屏阻塞感知比"加载中过渡"更糟。

### D8 — 与 `add-server-mode` change 的归档顺序

**决策**：本 change 的 spec delta 基于 `add-server-mode` archive **之后**的 `http-data-api` / `ipc-data-api` 主 spec 状态写。本 change 在 `add-server-mode` 合并 + archive 后再 push PR；本地 worktree 可并行落地代码，但 push 前 SHALL rebase main 重写 spec delta。

**Why 不等**：本 change 代码改动核心独立于 `add-server-mode`（一个改后端 handler 选择、一个改前端 store / 视觉），并行省 wall time；spec delta 风险窗口仅"`add-server-mode` 中途撤回 SSE bridge"——可能性极低。

### D9 — `list_sessions_sync` trait method 保留

**决策**：`DataApi::list_sessions_sync` trait 默认方法 + `LocalDataApi` override 实现**保留**，但 `cdt-api::http::routes::list_sessions` 不再调用。

**Why 不删**：
- `cdt-cli` 可能直接调 `LocalDataApi::list_sessions_sync`（非走 HTTP 路由）
- 第三方 HTTP client（如不订阅 SSE 的脚本）仍可通过另一个 endpoint 或直接 trait 调用拿完整数据——本 change 仅改 in-tree 浏览器 client 的体验路径，不削减能力
- 删 trait method 是 cross-cutting 改动，需要单独 change 处理 cli 兼容

## Risks / Trade-offs

- **R1：SSE 订阅与首次 fetch race 窗口** → D2 mitigation：`ensureSseReady` await OPEN；1 s 超时放行避免阻塞冷启（极端情况 SSE 永远拿不到 OPEN 但 HTTP 正常时，骨架仍能展示，metadata 由后续 file-change silent refresh 兜底）。
- **R2：In-flight cancel 时 SSE 已 emit 旧 project metadata** → 前端 listener 仍会收到 update，但 `pendingMetadataUpdates` clear + `sessions` 已替换为新 project → buffer 中没有 sessionId 匹配 → drop。无害。
- **R3：sessions store LRU evict 导致用户回头切 project 体感倒退** → capacity 16 远大于日常多 project 场景（用户单次会话通常 3-6 个 project），评估为低风险；可在 settings 暴露 capacity（v2，本 change 不做）。
- **R4（撤回，本 change 不引入持久化）**
- **R5（撤回，本 change 不引入持久化）**
- **R6：debounce 100 ms 引入翻页延迟感** → 100 ms 在人类感知阈值下沿；如有用户报告"翻页迟钝"，可调到 50 ms 或改成 leading + trailing 组合。
- **R7：metadata 渐显 150 ms 累计在长列表上视觉过载** → CSS animation 浏览器层 GPU 合成，200 条同时渐显成本 ms 级；如出现性能问题用 `content-visibility: auto` + `will-change: opacity` 优化（先观察后再决定，避免过度优化）。
- **R8：与 `add-server-mode` archive 中途撤回 SSE bridge** → 本 change spec delta 失去前提依赖；mitigation：PR push 前重新 grep 主 spec 验证 SSE 桥接相关 Requirement 仍在；缺失则补一个 ADDED Requirement"`add-server-mode` 撤回部分的 SSE producer" 进本 change。
- **R9：`list_sessions_sync` 保留为 dead code** → 加 `#[allow(dead_code)]` 或 `#[cfg(any(test, feature = "legacy-sync-api"))]`；未来 cli 不再使用时可在独立 change 删除。

## Migration Plan

1. **Phase A（后端独立可发）**
   - `cdt-api/src/http/routes.rs::list_sessions` 切骨架版
   - 测试：`crates/cdt-api/tests/http_list_sessions_skeleton_then_sse.rs`

2. **Phase B（前端 transport 与 store）**
   - `ui/src/lib/transport.ts::BrowserTransport.ensureSseReady`
   - 新建 `ui/src/lib/sessionListStore.svelte.ts`
   - `ui/src/lib/sessionListStore.test.ts` 单测

3. **Phase C（Sidebar 接线 + 视觉）**
   - `Sidebar.svelte::loadSessions` / `loadMoreSessions` 改走 store
   - metadata-pending CSS class + shimmer + fade-in
   - 验证 Tauri runtime + 浏览器 runtime + 冷启动三个场景

4. **Phase D（e2e + 性能 baseline）**
   - Playwright spec：快速翻页 / 切 project / 冷启动首帧
   - `scripts/run-perf-bench.sh` 跑 `perf_cold_scan` + `perf_get_session_detail`，确认 user/real / RSS 无回归

5. **Phase E（PR push 时机）**
   - 等 `add-server-mode` archive merge 进 main
   - 本 worktree rebase main，spec delta 基于最新主 spec 校验
   - push → wait-ci → codex 二审 → archive

**Rollback**：`cdt-api/src/http/routes.rs::list_sessions` 改回 `list_sessions_sync` 即恢复 server mode 旧行为；前端 store / 视觉渐显独立，不引入新 spec 契约的话也可逐项 revert，互不依赖。

## Open Questions

- **Q1（撤回，本 change 不引入持久化）**
- **Q2**：浏览器 server mode 多 tab 同开时，前端 sessions store 各自独立（每个 tab JS 进程）；后端 `MetadataCache` 共享；SSE bridge 每 tab 独立 EventSource。当前无观察到问题，但快速翻页 + 多 tab 时 SSE 带宽叠加；如有报告再加 dedupe。
