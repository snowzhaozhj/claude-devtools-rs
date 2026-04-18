## Context

现状：前端 Sidebar 切项目时调用 `listSessions(projectId)`；Tauri IPC 进入 `crates/cdt-api/src/ipc/local.rs::list_sessions`，内部对当前页（默认 50）每个 session 顺序 `extract_session_metadata`，其实现对 JSONL 全文件扫描（算 `message_count` 配对计数 + `is_ongoing`）。claude-devtools-rs 项目有 50 个会话，单文件平均 1 MB、最大 4.6 MB，总耗时肉眼可感（数秒级 spinner）。

前端 `{#each}` 渲染 50 条 session item（含 pin 图标、ongoing 圆点、消息计数、相对时间、右键菜单），DOM 已经接近可感知拐点。

反闪烁三原则（CLAUDE.md 已明示）必须保持：稳定 key、`silent=true` 保留旧列表、不经"加载中..."中间态。

## Goals / Non-Goals

**Goals：**
- 首屏骨架列表 < 200ms 出现（切项目 / 初次加载）。
- 元数据（title / messageCount / isOngoing）异步扫描，逐条 patch 到视图。
- 并发度可控（避免 50 个文件并发撑爆 SSD I/O）。
- 虚拟滚动承载未来几百会话的扩容。
- 保持反闪烁三原则：key 稳定、silent 刷新、无中间态。
- HTTP API 行为兼容（HTTP 无 push 通道，保持同步完整返回）。

**Non-Goals：**
- SessionDetail 页面性能优化。
- 跨项目预取。
- 持久化元数据缓存（硬盘缓存：下一轮再说）。
- Notifications / Tabs 列表的虚拟滚动。
- 其他 list API（`list_projects` 等）的骨架化。

## Decisions

### 1. 元数据递送方式：`list_sessions` 骨架 + broadcast event

**选：** `list_sessions` 返回的 `SessionSummary` 的 `title` / `messageCount` / `isOngoing` 为占位值（`null` / `0` / `false`）立即返回；后台 `JoinSet` 扫描，每扫完一个向 `tokio::sync::broadcast::Sender<SessionMetadataUpdate>` 推送一条；Tauri host 在 `setup` 订阅后以 `app.emit("session-metadata-update", payload)` 转发给前端；前端在 Sidebar 内 `listen("session-metadata-update", ...)` 按 `sessionId` 定位 in-place patch。

**替代：** (a) 新增独立命令 `get_session_metadata_batch(ids)` 前端轮询——多一次 IPC 往返，且前端需自己管状态机；(b) 保持同步但后端并发 JoinSet 等所有结果再返回——首屏仍要等最慢的文件扫完，峰值时间与当前一致。

**理由：** broadcast 方案可用既有 `FileWatcher` bridge 模式（见 CLAUDE.md"订阅后端 `broadcast::Receiver` 转 `emit(...)` 是典型模式"），零基础设施成本；前端拿到骨架即可立即渲染。

### 2. 并发度 = 8 + `tokio::task::JoinSet`

**选：** `JoinSet` 内 spawn 当前页所有扫描任务，但用 `tokio::sync::Semaphore::new(8)` 限流。

**替代：** (a) 全并发 50——会同时开 50 个文件 read handle，macOS 默认 ulimit 256，但 SSD 小文件随机读性能会因队列过深衰减；(b) 串行（当前）——慢；(c) 并发度 = CPU 核数——I/O 密集型不匹配。

**理由：** JSONL 扫描是 I/O 密集（buffered line read），8 路并发足够打满 NVMe 顺序读，且不抢 tokio runtime。常量写死在 `session_metadata.rs`，后续可提为 config。

### 3. broadcast 订阅 API 位置：`LocalDataApi` 非 trait 方法

**选：** 类比既有 `subscribe_files()` / `subscribe_detected_errors()`，新增 `LocalDataApi::subscribe_session_metadata() -> broadcast::Receiver<SessionMetadataUpdate>`；不加入 `DataApi` trait（HTTP 侧不需要）。

**替代：** 加到 trait——HTTP 侧要么 no-op 要么走 SSE，复杂度不值。

**理由：** 遵循 CLAUDE.md "Trigger CRUD 走独立方法" 模式，避免破坏现有 trait 实现与集成测试。

### 4. 扫描触发时机：`list_sessions` 调用链内

**选：** 每次 `list_sessions(projectId)` 被调用，后端：
1. 同步完成目录 scan → 返回骨架 `SessionSummary` 列表。
2. 返回前用 `tokio::spawn` 启动一个扫描任务（持有 broadcast `Sender` 克隆），异步并发扫当前页。
3. 若同一 `projectId` 在扫描进行中被再次调用，**取消前一次** 并重启（用 `tokio::sync::Mutex<Option<AbortHandle>>` 按 projectId 存）——避免切项目来回导致的事件风暴。

**替代：** 全局 LRU 缓存 + TTL——引入缓存一致性问题（file-change 得失效）；本次不做，留给下一轮。

**理由：** 每次 list 都重扫能天然对齐 file-change 刷新（`silent=true` 自动 refetch 会复用该路径）；取消前一次扫描避免切项目时事件串扰。

### 5. HTTP API 保持同步完整返回

**选：** HTTP `GET /projects/:id/sessions` 路径保留 serial 扫描（同当前）。

**替代：** HTTP 也骨架化 + 单独的 metadata endpoint——增加 API 复杂度；HTTP 当前无实际用户（UI 走 IPC）。

**理由：** HTTP 是 future-proofing，不是当前用户路径；保持简单。后续如需优化可单开 change。

### 6. 虚拟滚动方案：自写固定高度 windowing

**选：** 自写 Svelte 5 `$effect` + `scrollTop` 监听 + `itemHeight * totalCount` 占位高度。每个 session item 高度固定（当前 ~44px，pin/ongoing 不影响行高）。overscan = 5。日期分组头独立渲染在 pinned / date group 容器内（不参与 windowing），分组内部 items 走 windowing。

**替代：** (a) `@tanstack/svelte-virtual`——额外依赖，兼容 Svelte 5 runes 情况未验证；(b) `svelte-virtual-list`——Svelte 4 只，不兼容 runes；(c) IntersectionObserver 分批渲染——实现复杂、滚动条长度不准。

**理由：** 项目已有自写 Svelte 组件习惯（BaseItem、OutputBlock 等），轻量实现 ~80 行代码；零依赖风险。固定高度在当前 design token 下稳定（`session-item` padding 8px + title line-height 1.4 × 13px + meta line-height × 11px ≈ 44px）。

**边界处理：**
- PINNED 分区和日期分组各自是一个 windowing 容器。
- 分组数据变化（file-change 刷新）时 recompute `totalCount` 和 `scrollTop`。
- 活动高亮（`activeSessionId`）出现在当前视口外时，**不**自动滚动（保持用户当前位置），仅在用户点击导航时由 Tab 系统保证显式滚动可见（future）。
- 右键菜单坐标按 clientX/Y 直接用（与虚拟滚动无关）。

### 7. 前端状态机：骨架 + patch

**`SessionSummary` 运行时可能三态：**
- **skeleton**：title=null, messageCount=0, isOngoing=false（刚从 `listSessions` 返回）。
- **partial**：已收到 metadata update 事件，字段已填。
- **stale**：file-change 触发 `silent=true` refetch 后，旧 `SessionSummary` 被新骨架替换前，保留当前值；新骨架到达后**保留旧元数据字段值**直到新 metadata event 到达再覆盖——避免"已完成会话的元数据瞬间闪回骨架态"。

**实现细节：** `loadSessions(silent=true)` 替换 `sessions` 时按 `sessionId` 对齐旧数组，用旧元数据填充新骨架（一次性 merge，不改 UI 逻辑）。

### 8. 事件 payload 格式（camelCase）

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadataUpdate {
    pub project_id: String,
    pub session_id: String,
    pub title: Option<String>,
    pub message_count: usize,
    pub is_ongoing: bool,
}
```

前端类型对应 `{ projectId, sessionId, title, messageCount, isOngoing }`。

## Risks / Trade-offs

- **[风险] 事件风暴**：file-change 密集触发 `silent=true` refetch，每次都会重跑扫描并产出 50 条 metadata event → Mitigation：decision 4 的 AbortHandle 取消机制；前端 `listen` handler 幂等（按 sessionId 覆盖），乱序到达无副作用。
- **[风险] 虚拟滚动与动画冲突**：OngoingIndicator 的 `animate-ping` 在滚动复用 DOM 时可能抖动 → Mitigation：windowing 按 `sessionId` 做 key，Svelte 会复用同 key 节点；OngoingIndicator 的 keyed subtree 不会重建。
- **[风险] 固定行高假设破裂**：若未来 session item 增加第二行（长 title 折行等）行高会变 → Mitigation：当前设计 title 强制 `white-space: nowrap + ellipsis`，不会折行；若将来改就重新评估（可转 dynamic height windowing）。
- **[风险] HTTP API 行为分叉**：IPC 骨架化，HTTP 仍同步 → 两者语义不一致。未来若 HTTP 成为主路径需统一 → Mitigation：design.md / spec 明文写分叉理由；下一轮 change 可对齐。
- **[权衡] 无持久化缓存**：每次切项目都重扫，虽然是并发，但 50 个大文件仍耗 CPU → 后续可加硬盘元数据缓存（文件 mtime → metadata）；本次不做，避免过度设计。
- **[权衡] 骨架态 UI 信息缺失**：首 200ms 内 title 显示 sessionId 前缀、messageCount 显 `C`、无 ongoing 圆点——信息密度下降 → 这是"快速出现 vs 完整信息"的自然权衡；符合"瞬时反馈 > 完整渲染"的通行 UX。

## Migration Plan

1. 后端先实现骨架返回 + broadcast，但保留旧同步完整返回路径作为 fallback（feature flag：`CDT_SESSIONS_FAST_LOAD=1`）——可快速回滚。
2. Tauri bridge 上线后，前端切换订阅，验证无闪烁。
3. 稳定 1 周后移除 fallback 和 feature flag。
4. HTTP API 路径不变（见 decision 5）。

无数据迁移。回滚只需设置 `CDT_SESSIONS_FAST_LOAD=0` 重启。
